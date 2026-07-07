//! [`KpmClient`]: a cloneable, thread-safe handle that serialises all backend
//! work onto one worker thread.
//!
//! Query methods block until the worker answers (index queries are local
//! sqlite reads, so this is fast). Job methods (`update_index`, `install`,
//! `uninstall`, `add_repository`) also block until the job finishes, but
//! stream [`Event`]s to a caller-supplied channel while running — the caller
//! is expected to invoke them from its own background task, never from a UI
//! event loop thread.

use std::sync::mpsc::{channel, Sender};
use std::thread;

use crate::backend::Backend;
use crate::{
    Artifact, BackendInfo, Event, EventSink, InstallTarget, InstalledPackage, Package, Repository,
    Result, Upgrade,
};

enum Command {
    Info(Sender<BackendInfo>),
    ListRepositories(Sender<Result<Vec<Repository>>>),
    AddRepository(String, EventSink, Sender<Result<Repository>>),
    RemoveRepository(String, Sender<Result<()>>),
    ListRepositoryPackages(String, Sender<Result<Vec<Package>>>),
    Search(String, Sender<Result<Vec<Package>>>),
    GetPackage(Option<String>, String, Sender<Result<Option<Package>>>),
    ListArtifacts(String, String, Sender<Result<Vec<Artifact>>>),
    ListInstalled(Sender<Result<Vec<InstalledPackage>>>),
    GetInstalled(String, Sender<Result<Option<InstalledPackage>>>),
    ListUpgrades(Sender<Result<Vec<Upgrade>>>),
    UpdateIndex(EventSink, Sender<Result<()>>),
    Install(Vec<InstallTarget>, EventSink, Sender<Result<()>>),
    Uninstall(Vec<String>, EventSink, Sender<Result<()>>),
    Launch(String, Sender<Result<()>>),
}

/// Cloneable handle to the KPM worker thread.
#[derive(Clone)]
pub struct KpmClient {
    tx: Sender<Command>,
}

impl KpmClient {
    /// Start the worker thread with the given backend.
    pub fn start(mut backend: Box<dyn Backend>) -> Self {
        let (tx, rx) = channel::<Command>();
        thread::Builder::new()
            .name("kpm-worker".into())
            .spawn(move || {
                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        Command::Info(reply) => {
                            let _ = reply.send(backend.info());
                        }
                        Command::ListRepositories(reply) => {
                            let _ = reply.send(backend.list_repositories());
                        }
                        Command::AddRepository(url, events, reply) => {
                            let _ = reply.send(backend.add_repository(&url, &events));
                        }
                        Command::RemoveRepository(id, reply) => {
                            let _ = reply.send(backend.remove_repository(&id));
                        }
                        Command::ListRepositoryPackages(id, reply) => {
                            let _ = reply.send(backend.list_repository_packages(&id));
                        }
                        Command::Search(query, reply) => {
                            let _ = reply.send(backend.search(&query));
                        }
                        Command::GetPackage(repo, id, reply) => {
                            let _ = reply.send(backend.get_package(repo.as_deref(), &id));
                        }
                        Command::ListArtifacts(repo, id, reply) => {
                            let _ = reply.send(backend.list_artifacts(&repo, &id));
                        }
                        Command::ListInstalled(reply) => {
                            let _ = reply.send(backend.list_installed());
                        }
                        Command::GetInstalled(id, reply) => {
                            let _ = reply.send(backend.get_installed(&id));
                        }
                        Command::ListUpgrades(reply) => {
                            let _ = reply.send(backend.list_upgrades());
                        }
                        Command::UpdateIndex(events, reply) => {
                            let _ = reply.send(backend.update_index(&events));
                        }
                        Command::Install(targets, events, reply) => {
                            let _ = reply.send(backend.install(&targets, &events));
                        }
                        Command::Uninstall(ids, events, reply) => {
                            let _ = reply.send(backend.uninstall(&ids, &events));
                        }
                        Command::Launch(id, reply) => {
                            let _ = reply.send(backend.launch(&id));
                        }
                    }
                }
            })
            .expect("failed to spawn kpm worker thread");
        Self { tx }
    }

    fn request<T>(&self, make: impl FnOnce(Sender<T>) -> Command) -> T
    where
        T: Send + 'static,
    {
        let (reply_tx, reply_rx) = channel();
        self.tx
            .send(make(reply_tx))
            .expect("kpm worker thread is gone");
        reply_rx.recv().expect("kpm worker thread dropped reply")
    }

    pub fn info(&self) -> BackendInfo {
        self.request(Command::Info)
    }

    pub fn list_repositories(&self) -> Result<Vec<Repository>> {
        self.request(Command::ListRepositories)
    }

    pub fn add_repository(&self, url: &str, events: Sender<Event>) -> Result<Repository> {
        let sink = EventSink::new(events, true);
        self.request(|reply| Command::AddRepository(url.to_string(), sink, reply))
    }

    pub fn remove_repository(&self, id: &str) -> Result<()> {
        self.request(|reply| Command::RemoveRepository(id.to_string(), reply))
    }

    pub fn list_repository_packages(&self, repository_id: &str) -> Result<Vec<Package>> {
        self.request(|reply| Command::ListRepositoryPackages(repository_id.to_string(), reply))
    }

    pub fn search(&self, query: &str) -> Result<Vec<Package>> {
        self.request(|reply| Command::Search(query.to_string(), reply))
    }

    pub fn get_package(&self, repository: Option<&str>, id: &str) -> Result<Option<Package>> {
        self.request(|reply| {
            Command::GetPackage(repository.map(str::to_string), id.to_string(), reply)
        })
    }

    pub fn list_artifacts(&self, repository: &str, package_id: &str) -> Result<Vec<Artifact>> {
        self.request(|reply| {
            Command::ListArtifacts(repository.to_string(), package_id.to_string(), reply)
        })
    }

    pub fn list_installed(&self) -> Result<Vec<InstalledPackage>> {
        self.request(Command::ListInstalled)
    }

    pub fn get_installed(&self, id: &str) -> Result<Option<InstalledPackage>> {
        self.request(|reply| Command::GetInstalled(id.to_string(), reply))
    }

    pub fn list_upgrades(&self) -> Result<Vec<Upgrade>> {
        self.request(Command::ListUpgrades)
    }

    /// Update the package index, streaming progress to `events`.
    pub fn update_index(&self, events: Sender<Event>) -> Result<()> {
        let sink = EventSink::new(events, true);
        self.request(|reply| Command::UpdateIndex(sink, reply))
    }

    /// Install/upgrade packages, streaming progress to `events`. The UI shows
    /// its own confirmation before queueing, so backend prompts auto-accept.
    pub fn install(&self, targets: Vec<InstallTarget>, events: Sender<Event>) -> Result<()> {
        let sink = EventSink::new(events, true);
        self.request(|reply| Command::Install(targets, sink, reply))
    }

    /// Uninstall packages, streaming progress to `events`.
    pub fn uninstall(&self, ids: Vec<String>, events: Sender<Event>) -> Result<()> {
        let sink = EventSink::new(events, true);
        self.request(|reply| Command::Uninstall(ids, sink, reply))
    }

    pub fn launch(&self, id: &str) -> Result<()> {
        self.request(|reply| Command::Launch(id.to_string(), reply))
    }
}

/// Construct the default backend for this build and start a client on it.
///
/// Returns `Err` with a user-presentable message when the real backend's
/// compatibility gate rejects the installed KPM (the soft-fail path).
pub fn start_default_client() -> Result<KpmClient> {
    #[cfg(feature = "libkpm")]
    {
        let backend = crate::real::RealBackend::new()?;
        return Ok(KpmClient::start(Box::new(backend)));
    }
    #[cfg(all(feature = "mock", not(feature = "libkpm")))]
    {
        Ok(KpmClient::start(Box::new(crate::mock::MockBackend::new())))
    }
    #[cfg(all(not(feature = "mock"), not(feature = "libkpm")))]
    {
        Err(Error::Unavailable(
            "kpm built without a backend: enable the `libkpm` or `mock` feature".into(),
        ))
    }
}
