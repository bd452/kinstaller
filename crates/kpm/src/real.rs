//! The real backend: drives the device-installed `libkpm.so` through
//! `kpm-sys`. Only compiled with the `libkpm` feature.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;
use std::sync::{Arc, Mutex};

use kpm_sys::api::KpmApi;
use kpm_sys::io::{IoEvent, IoHandler};
use kpm_sys::types as sys;

use crate::backend::Backend;
use crate::{
    check_kpm_result, Artifact, BackendInfo, Error, EventSink, InstallTarget, InstalledPackage,
    Package, Repository, Result, SemVer, Verbosity,
};

/// Routes global KPMIO callbacks to the sink of the currently-running job.
/// libkpm's IO callbacks carry no user data, so this indirection is global;
/// the worker thread swaps the active sink in and out around each job.
#[derive(Clone, Default)]
struct SinkRouter {
    current: Arc<Mutex<Option<EventSink>>>,
}

impl IoHandler for SinkRouter {
    fn on_event(&self, event: IoEvent) {
        let guard = self.current.lock().unwrap();
        let Some(sink) = guard.as_ref() else { return };
        match event {
            IoEvent::Log { verbosity, message } => {
                let level = match verbosity {
                    sys::KPM_VERBOSITY_DEBUG => Verbosity::Debug,
                    sys::KPM_VERBOSITY_WARN => Verbosity::Warn,
                    sys::KPM_VERBOSITY_ERROR => Verbosity::Error,
                    _ => Verbosity::Info,
                };
                sink.log(level, message);
            }
            IoEvent::Stream(c) => {
                // Forward streamed subprocess output as info-level fragments.
                if c != '\r' {
                    sink.log(Verbosity::Info, c.to_string());
                }
            }
            IoEvent::Progress { percent, message } => sink.progress(percent, message),
        }
    }

    fn on_input(&self, prompt: &str) -> bool {
        let guard = self.current.lock().unwrap();
        match guard.as_ref() {
            Some(sink) => sink.confirm(prompt),
            None => false,
        }
    }
}

pub struct RealBackend {
    api: KpmApi,
    kpm: sys::KPM,
    // Own the path buffers referenced by `kpm` for the backend's lifetime.
    _db_path: CString,
    _pkg_path: CString,
    router: SinkRouter,
    info: BackendInfo,
}

// The raw pointers inside `sys::KPM` are only touched from the worker thread.
unsafe impl Send for RealBackend {}

impl RealBackend {
    /// Load + verify libkpm and initialise a KPM session.
    pub fn new() -> Result<Self> {
        let (api, loaded) = unsafe { kpm_sys::load_verified() }.map_err(|e| match e {
            kpm_sys::LoadError::Compat(c) => Error::Incompatible(c.to_string()),
            other => Error::Unavailable(other.to_string()),
        })?;

        let db_path = CString::new(sys::KPM_DB_PATH).unwrap();
        let pkg_path = CString::new(sys::KPM_PKG_PATH).unwrap();

        // Mirror the CLI: make sure the package/db directories exist.
        let _ = std::fs::create_dir_all(sys::KPM_PKG_PATH);
        if let Some(parent) = Path::new(sys::KPM_DB_PATH).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut kpm = sys::KPM {
            db: ptr::null_mut(),
            dbPath: db_path.as_ptr() as *mut c_char,
            pkgPath: pkg_path.as_ptr() as *mut c_char,
            maxConnections: 5,
        };
        check_kpm_result(unsafe { (api.KPM_Initialise)(&mut kpm) })?;

        let router = SinkRouter::default();
        kpm_sys::io::set_io_handler(Box::new(router.clone()));

        let kpm_version = loaded
            .identity
            .cli_version
            .clone()
            .or_else(|| loaded.entry.map(|e| e.kpm_version.to_string()));
        let info = BackendInfo {
            description: format!(
                "libkpm {} ({})",
                kpm_version.as_deref().unwrap_or("unknown"),
                kpm_sys::compat::KPM_PLATFORM
            ),
            kpm_version,
            lib_sha256: Some(loaded.identity.sha256.clone()),
        };

        Ok(Self {
            api,
            kpm,
            _db_path: db_path,
            _pkg_path: pkg_path,
            router,
            info,
        })
    }

    /// Run `f` with `sink` receiving all KPMIO callbacks fired during it.
    fn with_sink<T>(&mut self, sink: &EventSink, f: impl FnOnce(&mut Self) -> T) -> T {
        *self.router.current.lock().unwrap() = Some(sink.clone());
        let out = f(self);
        *self.router.current.lock().unwrap() = None;
        out
    }
}

impl Drop for RealBackend {
    fn drop(&mut self) {
        kpm_sys::io::clear_io_handler();
        unsafe { (self.api.KPM_Cleanup)(&mut self.kpm) };
    }
}

fn owned_str(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

fn opt_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(owned_str(ptr))
    }
}

fn semver_from(v: sys::SemVer) -> SemVer {
    SemVer {
        major: v.major,
        minor: v.minor,
        patch: v.patch,
    }
}

fn repo_from(r: &sys::Repository) -> Repository {
    Repository {
        id: owned_str(r.id),
        url: owned_str(r.url),
        name: owned_str(r.name),
        description: owned_str(r.description),
    }
}

fn package_from(p: &sys::IndexedPackage) -> Package {
    Package {
        repository: owned_str(p.repository),
        id: owned_str(p.id),
        name: owned_str(p.name),
        author: owned_str(p.author),
        description: owned_str(p.description),
    }
}

fn installed_from(p: &sys::InstalledPackage) -> InstalledPackage {
    InstalledPackage {
        id: owned_str(p.id),
        repository: opt_str(p.repository),
        name: owned_str(p.name),
        author: owned_str(p.author),
        description: owned_str(p.description),
        version: semver_from(p.version),
        installed_as_dependency: p.installed_as_dependency,
    }
}

fn artifact_from(a: &sys::IndexedArtifact) -> Artifact {
    Artifact {
        repository: owned_str(a.repository),
        package_id: owned_str(a.id),
        url: owned_str(a.url),
        version: semver_from(a.version),
    }
}

/// Treat "row not found" (`KPM_SQLITE_ERROR` from the Get* lookups) as None.
fn ok_or_none<T>(result: Result<T>) -> Result<Option<T>> {
    match result {
        Ok(v) => Ok(Some(v)),
        Err(Error::Kpm { code: 3, .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

impl Backend for RealBackend {
    fn info(&self) -> BackendInfo {
        self.info.clone()
    }

    fn list_repositories(&mut self) -> Result<Vec<Repository>> {
        let mut count: usize = 0;
        let mut list: *mut sys::Repository = ptr::null_mut();
        check_kpm_result(unsafe {
            (self.api.KPM_ListRepositories)(&mut self.kpm, &mut count, &mut list)
        })?;
        let repos = unsafe { std::slice::from_raw_parts(list, count) }
            .iter()
            .map(repo_from)
            .collect();
        unsafe { (self.api.KPM_FreeRepositoryList)(count, list) };
        Ok(repos)
    }

    fn add_repository(&mut self, url: &str, events: &EventSink) -> Result<Repository> {
        let c_url = CString::new(url).map_err(|_| Error::Other("invalid URL".into()))?;
        let mut raw = sys::Repository {
            id: ptr::null_mut(),
            url: ptr::null_mut(),
            name: ptr::null_mut(),
            description: ptr::null_mut(),
        };
        let mut io = kpm_sys::io::kpm_io();
        let result = self.with_sink(events, |this| unsafe {
            (this.api.KPM_AddRepository)(&mut this.kpm, c_url.as_ptr(), &mut raw, &mut io)
        });
        check_kpm_result(result)?;
        let repo = repo_from(&raw);
        unsafe { (self.api.KPM_FreeRepository)(&mut raw) };
        Ok(repo)
    }

    fn remove_repository(&mut self, id: &str) -> Result<()> {
        let c_id = CString::new(id).map_err(|_| Error::Other("invalid id".into()))?;
        check_kpm_result(unsafe {
            (self.api.KPM_GetRepository)(&mut self.kpm, c_id.as_ptr(), ptr::null_mut())
        })?;
        check_kpm_result(unsafe { (self.api.KPM_RemoveRepository)(&mut self.kpm, c_id.as_ptr()) })
    }

    fn list_repository_packages(&mut self, repository_id: &str) -> Result<Vec<Package>> {
        let c_id = CString::new(repository_id).map_err(|_| Error::Other("invalid id".into()))?;
        let mut count: usize = 0;
        let mut list: *mut sys::IndexedPackage = ptr::null_mut();
        check_kpm_result(unsafe {
            (self.api.KPM_ListRepositoryPackages)(
                &mut self.kpm,
                c_id.as_ptr(),
                &mut count,
                &mut list,
            )
        })?;
        let packages = unsafe { std::slice::from_raw_parts(list, count) }
            .iter()
            .map(package_from)
            .collect();
        unsafe { (self.api.KPM_FreeIndexedPackageList)(count, list) };
        Ok(packages)
    }

    fn search(&mut self, query: &str) -> Result<Vec<Package>> {
        let c_query = CString::new(query).map_err(|_| Error::Other("invalid query".into()))?;
        let mut count: usize = 0;
        let mut list: *mut sys::IndexedPackage = ptr::null_mut();
        check_kpm_result(unsafe {
            (self.api.KPM_SearchPackages)(&mut self.kpm, c_query.as_ptr(), &mut count, &mut list)
        })?;
        let packages = unsafe { std::slice::from_raw_parts(list, count) }
            .iter()
            .map(package_from)
            .collect();
        unsafe { (self.api.KPM_FreeIndexedPackageList)(count, list) };
        Ok(packages)
    }

    fn get_package(&mut self, repository: Option<&str>, id: &str) -> Result<Option<Package>> {
        let c_repo = repository
            .map(CString::new)
            .transpose()
            .map_err(|_| Error::Other("invalid repository".into()))?;
        let c_id = CString::new(id).map_err(|_| Error::Other("invalid id".into()))?;
        let mut raw = sys::IndexedPackage {
            repository: ptr::null_mut(),
            id: ptr::null_mut(),
            name: ptr::null_mut(),
            author: ptr::null_mut(),
            description: ptr::null_mut(),
        };
        let code = unsafe {
            (self.api.KPM_GetPackage)(
                &mut self.kpm,
                c_repo.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
                c_id.as_ptr(),
                &mut raw,
            )
        };
        ok_or_none(check_kpm_result(code).map(|_| {
            let pkg = package_from(&raw);
            unsafe { (self.api.KPM_FreeIndexedPackage)(&mut raw) };
            pkg
        }))
    }

    fn list_artifacts(&mut self, repository: &str, package_id: &str) -> Result<Vec<Artifact>> {
        let c_repo =
            CString::new(repository).map_err(|_| Error::Other("invalid repository".into()))?;
        let c_id = CString::new(package_id).map_err(|_| Error::Other("invalid id".into()))?;
        let mut count: usize = 0;
        let mut list: *mut sys::IndexedArtifact = ptr::null_mut();
        check_kpm_result(unsafe {
            (self.api.KPM_ListPackageArtifacts)(
                &mut self.kpm,
                c_repo.as_ptr(),
                c_id.as_ptr(),
                &mut count,
                &mut list,
            )
        })?;
        let artifacts = unsafe { std::slice::from_raw_parts(list, count) }
            .iter()
            .map(artifact_from)
            .collect();
        unsafe { (self.api.KPM_FreeIndexedArtifactList)(count, list) };
        Ok(artifacts)
    }

    fn list_installed(&mut self) -> Result<Vec<InstalledPackage>> {
        let mut count: usize = 0;
        let mut list: *mut sys::InstalledPackage = ptr::null_mut();
        check_kpm_result(unsafe {
            (self.api.KPM_ListInstalledPackages)(&mut self.kpm, &mut count, &mut list)
        })?;
        let installed = unsafe { std::slice::from_raw_parts(list, count) }
            .iter()
            .map(installed_from)
            .collect();
        unsafe { (self.api.KPM_FreeInstalledPackageList)(count, list) };
        Ok(installed)
    }

    fn get_installed(&mut self, id: &str) -> Result<Option<InstalledPackage>> {
        let c_id = CString::new(id).map_err(|_| Error::Other("invalid id".into()))?;
        let mut raw = sys::InstalledPackage {
            id: ptr::null_mut(),
            repository: ptr::null_mut(),
            name: ptr::null_mut(),
            author: ptr::null_mut(),
            description: ptr::null_mut(),
            version: sys::SemVer {
                major: 0,
                minor: 0,
                patch: 0,
            },
            installed_as_dependency: false,
        };
        let code =
            unsafe { (self.api.KPM_GetInstalledPackage)(&mut self.kpm, c_id.as_ptr(), &mut raw) };
        ok_or_none(check_kpm_result(code).map(|_| {
            let pkg = installed_from(&raw);
            unsafe { (self.api.KPM_FreeInstalledPackage)(&mut raw) };
            pkg
        }))
    }

    fn update_index(&mut self, events: &EventSink) -> Result<()> {
        let mut io = kpm_sys::io::kpm_io();
        let result = self.with_sink(events, |this| unsafe {
            (this.api.KPM_UpdateIndex)(&mut this.kpm, &mut io)
        });
        check_kpm_result(result)
    }

    fn install(&mut self, targets: &[InstallTarget], events: &EventSink) -> Result<()> {
        // Own all the C strings/versions for the duration of the call; libkpm
        // borrows InstallTargets (the CLI passes argv pointers directly).
        let c_strings: Vec<(Option<CString>, CString, Option<Box<sys::SemVer>>)> = targets
            .iter()
            .map(|t| {
                let repo = t
                    .repository
                    .as_deref()
                    .map(CString::new)
                    .transpose()
                    .map_err(|_| Error::Other("invalid repository".into()))?;
                let id =
                    CString::new(t.id.as_str()).map_err(|_| Error::Other("invalid id".into()))?;
                let version = t.version.map(|v| {
                    Box::new(sys::SemVer {
                        major: v.major,
                        minor: v.minor,
                        patch: v.patch,
                    })
                });
                Ok((repo, id, version))
            })
            .collect::<Result<_>>()?;

        let mut raw_targets: Vec<sys::InstallTarget> = c_strings
            .iter()
            .map(|(repo, id, version)| sys::InstallTarget {
                repository: repo
                    .as_ref()
                    .map_or(ptr::null_mut(), |s| s.as_ptr() as *mut c_char),
                id: id.as_ptr() as *mut c_char,
                version: version
                    .as_ref()
                    .map_or(ptr::null_mut(), |v| v.as_ref() as *const _ as *mut _),
            })
            .collect();

        let mut io = kpm_sys::io::kpm_io();
        let result = self.with_sink(events, |this| unsafe {
            (this.api.KPM_InstallPackages)(
                &mut this.kpm,
                raw_targets.len(),
                raw_targets.as_mut_ptr(),
                &mut io,
            )
        });
        check_kpm_result(result)
    }

    fn uninstall(&mut self, ids: &[String], events: &EventSink) -> Result<()> {
        let c_ids: Vec<CString> = ids
            .iter()
            .map(|id| CString::new(id.as_str()).map_err(|_| Error::Other("invalid id".into())))
            .collect::<Result<_>>()?;
        let mut ptrs: Vec<*const c_char> = c_ids.iter().map(|s| s.as_ptr()).collect();

        let mut io = kpm_sys::io::kpm_io();
        let result = self.with_sink(events, |this| unsafe {
            (this.api.KPM_UninstallPackages)(&mut this.kpm, ptrs.len(), ptrs.as_mut_ptr(), &mut io)
        });
        check_kpm_result(result)
    }

    fn launch(&mut self, id: &str) -> Result<()> {
        // Delegate to the co-shipped CLI, detached, exactly like a KUAL
        // scriptlet would: `kpm launch <id>` runs the package's launch.sh.
        std::process::Command::new(kpm_sys::compat::default_cli_path())
            .arg("launch")
            .arg(id)
            .spawn()
            .map_err(|e| Error::Other(format!("could not launch {id}: {e}")))?;
        Ok(())
    }
}
