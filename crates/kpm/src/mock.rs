//! An in-memory fake backend with simulated progress, enabling native UI
//! development on macOS/Windows/Linux without a Kindle or libkpm.

use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::Duration;

use crate::backend::Backend;
use crate::{
    Artifact, BackendInfo, Error, EventSink, InstallTarget, InstalledPackage, Package, Repository,
    Result, SemVer, Verbosity,
};

const STEP: Duration = Duration::from_millis(120);

struct MockPackage {
    package: Package,
    versions: Vec<SemVer>,
}

pub struct MockBackend {
    repositories: Vec<Repository>,
    /// repo id -> package id -> package
    index: BTreeMap<String, BTreeMap<String, MockPackage>>,
    installed: BTreeMap<String, InstalledPackage>,
}

fn v(major: u32, minor: u32, patch: u32) -> SemVer {
    SemVer {
        major,
        minor,
        patch,
    }
}

impl MockBackend {
    pub fn new() -> Self {
        let kmc = Repository {
            id: "kindlemodding".into(),
            url: "https://repo.kindlemodding.org/manifest.json".into(),
            name: "Official KMC Repo".into(),
            description: "The official KMC repo".into(),
        };
        let community = Repository {
            id: "community".into(),
            url: "https://example.org/kindle-repo/manifest.json".into(),
            name: "Community Repo".into(),
            description: "Community-maintained Kindle homebrew".into(),
        };

        let mut index: BTreeMap<String, BTreeMap<String, MockPackage>> = BTreeMap::new();

        let kmc_packages = [
            ("kpm", "KPM", "Hackerdude", "The Kindle Package Manager allows you to install and uninstall apps and programs easily on your Kindle", vec![v(0, 2, 0), v(0, 2, 1)]),
            ("koreader", "KOReader", "KOReader team", "An ebook reader application supporting PDF, DjVu, EPUB, FB2 and many more formats", vec![v(2025, 4, 0), v(2026, 1, 0)]),
            ("kterm", "kterm", "bfabiszewski", "Terminal emulator for the Kindle with an on-screen keyboard", vec![v(3, 0, 0)]),
            ("kindlefetch", "KindleFetch", "justrals", "System information display for your Kindle, neofetch style", vec![v(1, 2, 0), v(1, 3, 1)]),
            ("blockamazon", "BlockAmazon", "gingrspacecadet", "Block Amazon services on your Kindle", vec![v(0, 1, 0)]),
        ];
        let community_packages = [
            (
                "kwordle",
                "KWordle",
                "somedev",
                "Wordle game for Kindle e-ink screens",
                vec![v(1, 0, 0), v(1, 1, 0)],
            ),
            (
                "hyprpad",
                "Hyprpad",
                "gingrspacecadet",
                "Text editor for Kindle",
                vec![v(0, 9, 2)],
            ),
            (
                "kanki",
                "Kanki",
                "flashdev",
                "Flashcard app for language learning",
                vec![v(2, 0, 1)],
            ),
            (
                "gnomegames",
                "GNOME Games",
                "porters",
                "Classic GNOME games collection for Kindle",
                vec![v(1, 0, 0)],
            ),
        ];

        for (repo, packages) in [
            ("kindlemodding", &kmc_packages[..]),
            ("community", &community_packages[..]),
        ] {
            let entry = index.entry(repo.to_string()).or_default();
            for (id, name, author, description, versions) in packages {
                entry.insert(
                    id.to_string(),
                    MockPackage {
                        package: Package {
                            repository: repo.to_string(),
                            id: id.to_string(),
                            name: name.to_string(),
                            author: author.to_string(),
                            description: description.to_string(),
                        },
                        versions: versions.clone(),
                    },
                );
            }
        }

        // Pre-installed state: kpm current, koreader out of date, kterm local.
        let mut installed = BTreeMap::new();
        installed.insert(
            "kpm".to_string(),
            InstalledPackage {
                id: "kpm".into(),
                repository: Some("kindlemodding".into()),
                name: "KPM".into(),
                author: "Hackerdude".into(),
                description: "The Kindle Package Manager".into(),
                version: v(0, 2, 1),
                installed_as_dependency: false,
            },
        );
        installed.insert(
            "koreader".to_string(),
            InstalledPackage {
                id: "koreader".into(),
                repository: Some("kindlemodding".into()),
                name: "KOReader".into(),
                author: "KOReader team".into(),
                description: "An ebook reader application".into(),
                version: v(2025, 4, 0),
                installed_as_dependency: false,
            },
        );
        installed.insert(
            "kterm".to_string(),
            InstalledPackage {
                id: "kterm".into(),
                repository: None, // local install
                name: "kterm".into(),
                author: "bfabiszewski".into(),
                description: "Terminal emulator (installed from local file)".into(),
                version: v(3, 0, 0),
                installed_as_dependency: true,
            },
        );

        Self {
            repositories: vec![kmc, community],
            index,
            installed,
        }
    }

    fn find_package(&self, id: &str) -> Option<&MockPackage> {
        self.index.values().find_map(|packages| packages.get(id))
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for MockBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            description: "mock backend (desktop preview)".into(),
            kpm_version: Some("0.2.1-mock".into()),
            lib_sha256: None,
        }
    }

    fn list_repositories(&mut self) -> Result<Vec<Repository>> {
        Ok(self.repositories.clone())
    }

    fn add_repository(&mut self, url: &str, events: &EventSink) -> Result<Repository> {
        events.log(Verbosity::Info, format!("Fetching manifest from {url}"));
        sleep(STEP * 3);
        if !url.starts_with("http") {
            return Err(Error::Other(format!("invalid repository URL: {url}")));
        }
        let id = format!("repo{}", self.repositories.len() + 1);
        let repo = Repository {
            id: id.clone(),
            url: url.to_string(),
            name: format!("Repository {id}"),
            description: "Added at runtime (mock)".into(),
        };
        self.repositories.push(repo.clone());
        self.index.entry(id).or_default();
        events.log(Verbosity::Info, format!("Added repository '{}'", repo.id));
        Ok(repo)
    }

    fn remove_repository(&mut self, id: &str) -> Result<()> {
        let before = self.repositories.len();
        self.repositories.retain(|r| r.id != id);
        if self.repositories.len() == before {
            return Err(Error::Other(format!("no such repository: {id}")));
        }
        self.index.remove(id);
        Ok(())
    }

    fn list_repository_packages(&mut self, repository_id: &str) -> Result<Vec<Package>> {
        Ok(self
            .index
            .get(repository_id)
            .map(|packages| packages.values().map(|p| p.package.clone()).collect())
            .unwrap_or_default())
    }

    fn search(&mut self, query: &str) -> Result<Vec<Package>> {
        let needle = query.to_lowercase();
        Ok(self
            .index
            .values()
            .flat_map(|packages| packages.values())
            .filter(|p| {
                p.package.id.to_lowercase().contains(&needle)
                    || p.package.name.to_lowercase().contains(&needle)
            })
            .map(|p| p.package.clone())
            .collect())
    }

    fn get_package(&mut self, repository: Option<&str>, id: &str) -> Result<Option<Package>> {
        match repository {
            Some(repo) => Ok(self
                .index
                .get(repo)
                .and_then(|packages| packages.get(id))
                .map(|p| p.package.clone())),
            None => Ok(self.find_package(id).map(|p| p.package.clone())),
        }
    }

    fn list_artifacts(&mut self, repository: &str, package_id: &str) -> Result<Vec<Artifact>> {
        Ok(self
            .index
            .get(repository)
            .and_then(|packages| packages.get(package_id))
            .map(|p| {
                p.versions
                    .iter()
                    .map(|&version| Artifact {
                        repository: repository.to_string(),
                        package_id: package_id.to_string(),
                        url: format!("packages/{package_id}/artifacts/{package_id}_{version}.kpkg"),
                        version,
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn list_installed(&mut self) -> Result<Vec<InstalledPackage>> {
        Ok(self.installed.values().cloned().collect())
    }

    fn get_installed(&mut self, id: &str) -> Result<Option<InstalledPackage>> {
        Ok(self.installed.get(id).cloned())
    }

    fn update_index(&mut self, events: &EventSink) -> Result<()> {
        for (i, repo) in self.repositories.clone().iter().enumerate() {
            let base = (i as u32) * 100 / self.repositories.len().max(1) as u32;
            events.progress(base, format!("Updating index for {}", repo.name));
            events.log(Verbosity::Info, format!("Downloading {}", repo.url));
            sleep(STEP * 4);
            events.log(Verbosity::Info, format!("Indexed repository '{}'", repo.id));
        }
        events.progress(100, "Index up to date");
        Ok(())
    }

    fn install(&mut self, targets: &[InstallTarget], events: &EventSink) -> Result<()> {
        for (i, target) in targets.iter().enumerate() {
            let Some(found) = self.find_package(&target.id) else {
                events.log(
                    Verbosity::Error,
                    format!("Could not find package '{}'", target.id),
                );
                return Err(Error::Kpm {
                    code: 2,
                    name: "KPM_GENERIC_ERROR",
                });
            };
            let package = found.package.clone();
            let version = target
                .version
                .or_else(|| found.versions.iter().copied().max())
                .unwrap_or_default();

            let base = (i as u32) * 100 / targets.len() as u32;
            let span = 100 / targets.len() as u32;
            events.progress(base, format!("Installing {} v{version}", package.name));
            events.log(
                Verbosity::Info,
                format!("Downloading {}_{version}.kpkg", package.id),
            );
            sleep(STEP * 5);
            events.progress(base + span / 3, format!("Extracting {}", package.id));
            sleep(STEP * 3);
            events.progress(
                base + 2 * span / 3,
                format!("Running install.sh for {}", package.id),
            );
            events.log(Verbosity::Info, "install.sh: setting up scriptlet");
            sleep(STEP * 3);

            self.installed.insert(
                package.id.clone(),
                InstalledPackage {
                    id: package.id.clone(),
                    repository: Some(package.repository.clone()),
                    name: package.name.clone(),
                    author: package.author.clone(),
                    description: package.description.clone(),
                    version,
                    installed_as_dependency: false,
                },
            );
            events.log(
                Verbosity::Info,
                format!("Installed {} v{version}", package.name),
            );
        }
        events.progress(100, "Done");
        Ok(())
    }

    fn uninstall(&mut self, ids: &[String], events: &EventSink) -> Result<()> {
        for (i, id) in ids.iter().enumerate() {
            let Some(pkg) = self.installed.get(id).cloned() else {
                events.log(Verbosity::Error, format!("'{id}' is not installed"));
                return Err(Error::Kpm {
                    code: 2,
                    name: "KPM_GENERIC_ERROR",
                });
            };
            let base = (i as u32) * 100 / ids.len() as u32;
            events.progress(base, format!("Removing {}", pkg.name));
            events.log(Verbosity::Info, format!("Running uninstall.sh for {id}"));
            sleep(STEP * 4);
            self.installed.remove(id);
            events.log(Verbosity::Info, format!("Removed {}", pkg.name));
        }
        events.progress(100, "Done");
        Ok(())
    }

    fn launch(&mut self, id: &str) -> Result<()> {
        if self.installed.contains_key(id) {
            Ok(())
        } else {
            Err(Error::Other(format!("'{id}' is not installed")))
        }
    }
}
