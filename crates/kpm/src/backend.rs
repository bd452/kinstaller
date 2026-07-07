//! The backend trait implemented by the real (`libkpm`) and `mock` backends.

use crate::{
    Artifact, BackendInfo, EventSink, InstallTarget, InstalledPackage, Package, Repository, Result,
    Upgrade,
};

/// Package-management operations. Implementations are driven from a single
/// worker thread, so `&mut self` methods never race.
pub trait Backend: Send {
    fn info(&self) -> BackendInfo;

    // Repositories
    fn list_repositories(&mut self) -> Result<Vec<Repository>>;
    fn add_repository(&mut self, url: &str, events: &EventSink) -> Result<Repository>;
    fn remove_repository(&mut self, id: &str) -> Result<()>;
    fn list_repository_packages(&mut self, repository_id: &str) -> Result<Vec<Package>>;

    // Index queries
    fn search(&mut self, query: &str) -> Result<Vec<Package>>;
    fn get_package(&mut self, repository: Option<&str>, id: &str) -> Result<Option<Package>>;
    fn list_artifacts(&mut self, repository: &str, package_id: &str) -> Result<Vec<Artifact>>;

    // Installed state
    fn list_installed(&mut self) -> Result<Vec<InstalledPackage>>;
    fn get_installed(&mut self, id: &str) -> Result<Option<InstalledPackage>>;

    // Long-running jobs
    fn update_index(&mut self, events: &EventSink) -> Result<()>;
    fn install(&mut self, targets: &[InstallTarget], events: &EventSink) -> Result<()>;
    fn uninstall(&mut self, ids: &[String], events: &EventSink) -> Result<()>;

    /// Launch an installed package (its `launch.sh`).
    fn launch(&mut self, id: &str) -> Result<()>;

    /// Compute available upgrades: installed packages whose repository has a
    /// newer artifact (mirrors the upstream CLI `upgrade` selection logic).
    fn list_upgrades(&mut self) -> Result<Vec<Upgrade>> {
        let installed = self.list_installed()?;
        let mut upgrades = Vec::new();
        for pkg in installed {
            let Some(repo) = pkg.repository.clone() else {
                continue; // local install, nothing to compare against
            };
            let Ok(artifacts) = self.list_artifacts(&repo, &pkg.id) else {
                continue;
            };
            // Artifacts are returned newest-first by KPM; take the max anyway.
            let Some(best) = artifacts.into_iter().max_by_key(|a| a.version) else {
                continue;
            };
            if best.version > pkg.version {
                upgrades.push(Upgrade {
                    installed: pkg,
                    available: best,
                });
            }
        }
        Ok(upgrades)
    }
}
