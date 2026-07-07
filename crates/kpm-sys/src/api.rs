//! Runtime loader for `libkpm.so`.
//!
//! All `KPM_*` entry points are resolved with `dlsym` at load time and stored
//! as plain function pointers. The [`KpmApi`] struct owns the underlying
//! [`libloading::Library`], so the mapping stays valid for the lifetime of the
//! struct (even if the file is replaced on disk by a KPM self-upgrade).

#![allow(non_snake_case)]

use std::os::raw::{c_char, c_int};
use std::path::Path;

use crate::types::*;

macro_rules! kpm_api {
    ($( $name:ident : fn( $($arg:ty),* ) $(-> $ret:ty)? ; )*) => {
        /// Resolved libkpm entry points. Field names match the C symbols.
        #[allow(non_snake_case)]
        pub struct KpmApi {
            // Keep the library mapped for as long as the symbols are usable.
            _lib: libloading::Library,
            $( pub $name: unsafe extern "C" fn( $($arg),* ) $(-> $ret)?, )*
        }

        impl KpmApi {
            /// Load `libkpm.so` from `path` and resolve every symbol.
            ///
            /// # Safety
            /// Loading a shared library runs its initialisers, and the
            /// resolved functions are only sound to call if the library's ABI
            /// matches the `types` module (enforced by the compatibility gate
            /// in `compat`).
            pub unsafe fn load(path: &Path) -> Result<Self, crate::LoadError> {
                let lib = libloading::Library::new(path)
                    .map_err(|e| crate::LoadError::Open(path.display().to_string(), e.to_string()))?;
                $(
                    let $name = *lib
                        .get::<unsafe extern "C" fn( $($arg),* ) $(-> $ret)?>(
                            concat!(stringify!($name), "\0").as_bytes(),
                        )
                        .map_err(|e| crate::LoadError::Symbol(stringify!($name), e.to_string()))?;
                )*
                Ok(Self { _lib: lib, $( $name, )* })
            }
        }
    };
}

kpm_api! {
    // Lifecycle
    KPM_Initialise: fn(*mut KPM) -> c_int;
    KPM_Cleanup: fn(*mut KPM);

    // Repositories
    KPM_FreeRepository: fn(*mut Repository);
    KPM_FreeRepositoryList: fn(usize, *mut Repository);
    KPM_ListRepositories: fn(*mut KPM, *mut usize, *mut *mut Repository) -> c_int;
    KPM_GetRepository: fn(*mut KPM, *const c_char, *mut Repository) -> c_int;
    KPM_AddRepository: fn(*mut KPM, *const c_char, *mut Repository, *mut KPMIO) -> c_int;
    KPM_RemoveRepository: fn(*mut KPM, *const c_char) -> c_int;
    KPM_ListRepositoryPackages: fn(*mut KPM, *const c_char, *mut usize, *mut *mut IndexedPackage) -> c_int;

    // Indexed packages
    KPM_FreeIndexedPackage: fn(*mut IndexedPackage);
    KPM_FreeIndexedPackageList: fn(usize, *mut IndexedPackage);
    KPM_GetPackage: fn(*mut KPM, *const c_char, *const c_char, *mut IndexedPackage) -> c_int;
    KPM_GetPackages: fn(*mut KPM, *const c_char, *mut usize, *mut *mut IndexedPackage) -> c_int;
    KPM_SearchPackages: fn(*mut KPM, *const c_char, *mut usize, *mut *mut IndexedPackage) -> c_int;

    // Installed packages
    KPM_FreeInstalledPackage: fn(*mut InstalledPackage);
    KPM_FreeInstalledPackageList: fn(usize, *mut InstalledPackage);
    KPM_GetInstalledPackage: fn(*mut KPM, *const c_char, *mut InstalledPackage) -> c_int;
    KPM_ListInstalledPackages: fn(*mut KPM, *mut usize, *mut *mut InstalledPackage) -> c_int;

    // Installed dependencies
    KPM_FreeInstalledPackageDependency: fn(*mut InstalledDependency);
    KPM_FreeInstalledPackageDependencyList: fn(usize, *mut InstalledDependency);
    KPM_ListInstalledPackageDependencies: fn(*mut KPM, *const c_char, *mut usize, *mut *mut InstalledDependency) -> c_int;
    KPM_ListInstalledPackageDependents: fn(*mut KPM, *const c_char, *mut usize, *mut *mut InstalledDependency) -> c_int;

    // Artifacts
    KPM_FreeIndexedArtifact: fn(*mut IndexedArtifact);
    KPM_FreeIndexedArtifactList: fn(usize, *mut IndexedArtifact);
    KPM_GetArtifact: fn(*mut KPM, *const c_char, *const c_char, SemVer, *mut IndexedArtifact) -> c_int;
    KPM_ListPackageArtifacts: fn(*mut KPM, *const c_char, *const c_char, *mut usize, *mut *mut IndexedArtifact) -> c_int;

    // Artifact dependencies
    KPM_FreeArtifactDependency: fn(*mut ArtifactDependency);
    KPM_FreeArtifactDependencyList: fn(usize, *mut ArtifactDependency);
    KPM_ListArtifactDependencies: fn(*mut KPM, *const c_char, *const c_char, *const c_char, *mut usize, *mut *mut ArtifactDependency) -> c_int;

    // Index + install/uninstall
    KPM_UpdateIndex: fn(*mut KPM, *mut KPMIO) -> c_int;
    KPM_FreeInstallTarget: fn(*mut InstallTarget);
    KPM_FreeInstallTargetList: fn(usize, *mut InstallTarget);
    KPM_InstallPackages: fn(*mut KPM, usize, *mut InstallTarget, *mut KPMIO) -> c_int;
    KPM_UninstallPackages: fn(*mut KPM, usize, *mut *const c_char, *mut KPMIO) -> c_int;
}
