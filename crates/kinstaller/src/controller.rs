//! View-model: bridges [`KpmClient`] and Slint [`AppState`].

use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use kpm::client::KpmClient;
use kpm::{Event, InstallTarget, InstalledPackage, Package, Repository, Upgrade, Verbosity};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::{AppState, AppWindow, LogLine, PackageRow, RepoRow, VersionRow};

const RUN_LOG_CAP: usize = 200;
const COMPAT_GUIDANCE: &str = "\n\nUpdate Kinstaller or KPM to continue.";

enum PendingOp {
    Install(Vec<InstallTarget>),
    Remove(Vec<String>),
    UpdateIndex,
    AddRepo(String),
}

pub struct Controller {
    client: KpmClient,
    window: slint::Weak<AppWindow>,
    pending: Arc<Mutex<Option<PendingOp>>>,
}

impl Controller {
    pub fn new(client: KpmClient, window: slint::Weak<AppWindow>) -> Rc<Self> {
        Rc::new(Self {
            client,
            window,
            pending: Arc::new(Mutex::new(None)),
        })
    }

    pub fn wire(self: &Rc<Self>, app: &AppWindow) {
        let state = app.global::<AppState>();

        let c = Rc::clone(self);
        state.on_tab_changed(move |tab| c.on_tab_changed(tab));

        let c = Rc::clone(self);
        state.on_refresh_current_tab(move || c.refresh_current_tab());

        let c = Rc::clone(self);
        state.on_update_index(move || c.update_index());

        let c = Rc::clone(self);
        state.on_upgrade_all(move || c.upgrade_all());

        let c = Rc::clone(self);
        state.on_add_repo(move |url| c.add_repo(url.into()));

        let c = Rc::clone(self);
        state.on_remove_repo(move |id| c.remove_repo(id.into()));

        let c = Rc::clone(self);
        state.on_open_repo(move |id| c.open_repo(id.into()));

        let c = Rc::clone(self);
        state.on_close_repo(move || c.close_repo());

        let c = Rc::clone(self);
        state.on_search_packages(move |query| c.search(query.into()));

        let c = Rc::clone(self);
        state.on_open_package(move |repo, id| {
            c.open_package(repo.into(), id.into());
        });

        let c = Rc::clone(self);
        state.on_close_detail(move || c.close_detail());

        let c = Rc::clone(self);
        state.on_request_install(move || c.request_install());

        let c = Rc::clone(self);
        state.on_request_remove(move || c.request_remove());

        let c = Rc::clone(self);
        state.on_request_launch(move || c.request_launch());

        let c = Rc::clone(self);
        state.on_confirm_accepted(move || c.confirm_accepted());

        let c = Rc::clone(self);
        state.on_confirm_cancelled(move || c.confirm_cancelled());

        let c = Rc::clone(self);
        state.on_run_dismissed(move || c.run_dismissed());

        let weak = self.window.clone();
        state.on_keyboard_append(move |ch| {
            if let Some(w) = weak.upgrade() {
                keyboard_edit_target(w.global::<AppState>(), Some(ch.as_str()), false);
            }
        });

        let weak = self.window.clone();
        state.on_keyboard_backspace(move || {
            if let Some(w) = weak.upgrade() {
                keyboard_edit_target(w.global::<AppState>(), None, true);
            }
        });
    }

    fn with_state<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(AppState<'_>) -> R,
    {
        self.window.upgrade().map(|w| f(w.global::<AppState>()))
    }

    fn on_tab_changed(&self, tab: i32) {
        match tab {
            0 => self.refresh_home(),
            1 => self.refresh_sources(),
            2 => self.refresh_changes(),
            3 => self.refresh_installed(),
            _ => {}
        }
    }

    pub fn refresh_current_tab(&self) {
        if let Some(tab) = self.with_state(|s| s.get_active_tab()) {
            self.on_tab_changed(tab);
        }
    }

    pub fn refresh_home(&self) {
        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let repos = client.list_repositories();
            let installed = client.list_installed();
            let upgrades = client.list_upgrades();
            let info = client.info();

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match (&repos, &installed, &upgrades) {
                        (Ok(repos), Ok(installed), Ok(upgrades)) => {
                            state.set_repo_count(repos.len() as i32);
                            state.set_installed_count(installed.len() as i32);
                            state.set_upgrade_count(upgrades.len() as i32);
                            state.set_backend_description(info.description.into());
                            state.set_status_text("Ready".into());
                        }
                        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                            state.set_status_text(format!("Error: {e}").into());
                        }
                    }
                }
            });
        });
    }

    pub fn refresh_sources(&self) {
        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let result = client.list_repositories();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match result {
                        Ok(repos) => {
                            let rows: Vec<RepoRow> = repos.iter().map(repo_row).collect();
                            state.set_repos(ModelRc::new(VecModel::from(rows)));
                            state.set_status_text("Ready".into());
                        }
                        Err(e) => state.set_status_text(format!("Error: {e}").into()),
                    }
                }
            });
        });
    }

    pub fn open_repo(&self, id: String) {
        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let repos = client.list_repositories();
            let packages = client.list_repository_packages(&id);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match (&repos, &packages) {
                        (Ok(repos), Ok(pkgs)) => {
                            let name = repos
                                .iter()
                                .find(|r| r.id == id)
                                .map(|r| r.name.clone())
                                .unwrap_or_else(|| id.clone());
                            let rows: Vec<PackageRow> = pkgs.iter().map(package_row).collect();
                            state.set_browsing_repo_id(id.into());
                            state.set_browsing_repo_name(name.into());
                            state.set_repo_packages(ModelRc::new(VecModel::from(rows)));
                            state.set_show_repo_packages(true);
                            state.set_status_text("Ready".into());
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            state.set_status_text(format!("Error: {e}").into());
                        }
                    }
                }
            });
        });
    }

    pub fn close_repo(&self) {
        self.with_state(|state| {
            state.set_show_repo_packages(false);
            state.set_browsing_repo_id("".into());
            state.set_browsing_repo_name("".into());
            state.set_repo_packages(ModelRc::new(VecModel::from(Vec::<PackageRow>::new())));
        });
    }

    pub fn refresh_changes(&self) {
        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let result = client.list_upgrades();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match result {
                        Ok(upgrades) => {
                            let rows: Vec<PackageRow> = upgrades.iter().map(upgrade_row).collect();
                            state.set_changes(ModelRc::new(VecModel::from(rows)));
                            state.set_status_text("Ready".into());
                        }
                        Err(e) => state.set_status_text(format!("Error: {e}").into()),
                    }
                }
            });
        });
    }

    pub fn refresh_installed(&self) {
        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let result = client.list_installed();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match result {
                        Ok(installed) => {
                            let rows: Vec<PackageRow> =
                                installed.iter().map(installed_row).collect();
                            state.set_installed(ModelRc::new(VecModel::from(rows)));
                            state.set_status_text("Ready".into());
                        }
                        Err(e) => state.set_status_text(format!("Error: {e}").into()),
                    }
                }
            });
        });
    }

    pub fn search(&self, query: String) {
        if query.trim().is_empty() {
            self.with_state(|state| {
                state.set_search_results(ModelRc::new(VecModel::from(Vec::<PackageRow>::new())));
            });
            return;
        }

        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let result = client.search(&query);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match result {
                        Ok(packages) => {
                            let rows: Vec<PackageRow> = packages.iter().map(package_row).collect();
                            state.set_search_results(ModelRc::new(VecModel::from(rows)));
                            state.set_status_text("Ready".into());
                        }
                        Err(e) => state.set_status_text(format!("Error: {e}").into()),
                    }
                }
            });
        });
    }

    pub fn open_package(&self, repo: String, id: String) {
        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let pkg = client.get_package(Some(&repo), &id);
            let installed = client.get_installed(&id);
            let artifacts = client.list_artifacts(&repo, &id);

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match (&pkg, &installed, &artifacts) {
                        (Ok(Some(package)), Ok(installed), Ok(artifacts)) => {
                            let mut artifacts = artifacts.clone();
                            artifacts.sort_by_key(|a| std::cmp::Reverse(a.version));

                            let installed_ver = installed.as_ref().map(|p| p.version);
                            let newest = artifacts.first().map(|a| a.version);

                            let primary_action = match (installed_ver, newest) {
                                (None, _) => "Install",
                                (Some(inst), Some(new)) if new > inst => "Upgrade",
                                _ => "Reinstall",
                            };

                            let version_rows: Vec<VersionRow> = artifacts
                                .iter()
                                .map(|a| VersionRow {
                                    version: a.version.to_string().into(),
                                    url: a.url.clone().into(),
                                })
                                .collect();

                            state.set_detail_id(id.into());
                            state.set_detail_repo(repo.into());
                            state.set_detail_title(package.name.clone().into());
                            state.set_detail_author(package.author.clone().into());
                            state.set_detail_description(package.description.clone().into());
                            state.set_detail_versions(ModelRc::new(VecModel::from(version_rows)));
                            state.set_detail_installed_version(
                                installed
                                    .as_ref()
                                    .map(|p| p.version.to_string())
                                    .unwrap_or_default()
                                    .into(),
                            );
                            state.set_detail_primary_action(primary_action.into());
                            state.set_detail_can_remove(installed.is_some());
                            state.set_detail_can_launch(installed.is_some());
                            state.set_screen(1);
                            state.set_status_text("Ready".into());
                        }
                        (Ok(None), _, _) => {
                            state.set_status_text("Error: package not found".into());
                        }
                        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                            state.set_status_text(format!("Error: {e}").into());
                        }
                    }
                }
            });
        });
    }

    pub fn close_detail(&self) {
        self.with_state(|state| {
            state.set_screen(0);
            state.set_detail_id("".into());
        });
    }

    fn show_confirm(
        &self,
        title: impl Into<SharedString>,
        lines: Vec<SharedString>,
        op: PendingOp,
    ) {
        *self.pending.lock().unwrap() = Some(op);
        self.with_state(|state| {
            state.set_confirm_title(title.into());
            state.set_confirm_lines(ModelRc::new(VecModel::from(lines)));
            state.set_screen(2);
        });
    }

    pub fn request_install(&self) {
        let Some((repo, id, title, action)) = self.with_state(|state| {
            (
                state.get_detail_repo().to_string(),
                state.get_detail_id().to_string(),
                state.get_detail_title().to_string(),
                state.get_detail_primary_action().to_string(),
            )
        }) else {
            return;
        };

        let target = InstallTarget {
            repository: Some(repo),
            id: id.clone(),
            version: None,
        };

        let run_title = match action.as_str() {
            "Upgrade" => "Upgrading",
            "Reinstall" => "Reinstalling",
            _ => "Installing",
        };
        let seed = format!("{action} {title} ({id})…");
        self.begin_run(
            run_title,
            PendingOp::Install(vec![target]),
            Some(seed.into()),
        );
    }

    pub fn request_remove(&self) {
        let Some((id, title)) = self.with_state(|state| {
            (
                state.get_detail_id().to_string(),
                state.get_detail_title().to_string(),
            )
        }) else {
            return;
        };

        let lines = vec![
            format!("Remove {title}").into(),
            format!("Package: {id}").into(),
        ];

        self.show_confirm("Remove Package?", lines, PendingOp::Remove(vec![id]));
    }

    pub fn update_index(&self) {
        self.begin_run("Updating Index", PendingOp::UpdateIndex, None);
    }

    pub fn upgrade_all(&self) {
        let client = self.client.clone();
        let pending = Arc::clone(&self.pending);
        let weak = self.window.clone();
        thread::spawn(move || {
            let result = client.list_upgrades();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match result {
                        Ok(upgrades) if upgrades.is_empty() => {
                            state.set_status_text("No upgrades available".into());
                        }
                        Ok(upgrades) => {
                            let lines: Vec<SharedString> = upgrades
                                .iter()
                                .map(|u| {
                                    format!(
                                        "{} ({} → {})",
                                        u.installed.name, u.installed.version, u.available.version
                                    )
                                    .into()
                                })
                                .collect();
                            let targets: Vec<InstallTarget> = upgrades
                                .iter()
                                .map(|u| InstallTarget {
                                    repository: Some(u.available.repository.clone()),
                                    id: u.installed.id.clone(),
                                    version: Some(u.available.version),
                                })
                                .collect();

                            *pending.lock().unwrap() = Some(PendingOp::Install(targets));
                            state.set_confirm_title("Upgrade All Packages?".into());
                            state.set_confirm_lines(ModelRc::new(VecModel::from(lines)));
                            state.set_screen(2);
                        }
                        Err(e) => state.set_status_text(format!("Error: {e}").into()),
                    }
                }
            });
        });
    }

    pub fn add_repo(&self, url: String) {
        self.begin_run(
            "Adding Repository",
            PendingOp::AddRepo(url),
            Some("Fetching repository manifest…".into()),
        );
    }

    pub fn confirm_accepted(&self) {
        let op = self.pending.lock().unwrap().take();
        if let Some(op) = op {
            self.begin_run_from_op(&op);
        }
    }

    pub fn confirm_cancelled(&self) {
        self.pending.lock().unwrap().take();
        self.with_state(|state| {
            if state.get_detail_id().is_empty() {
                state.set_screen(0);
            } else {
                state.set_screen(1);
            }
        });
    }

    fn begin_run_from_op(&self, op: &PendingOp) {
        let (title, seed) = match op {
            PendingOp::Install(_) => ("Installing", None),
            PendingOp::Remove(_) => ("Removing", None),
            PendingOp::UpdateIndex => ("Updating Index", None),
            PendingOp::AddRepo(_) => ("Adding Repository", None),
        };
        self.begin_run(title, op.clone_op(), seed);
    }

    fn begin_run(&self, title: &str, op: PendingOp, seed_log: Option<SharedString>) {
        if self.with_state(|s| s.get_busy()).unwrap_or(false) {
            return;
        }

        *self.pending.lock().unwrap() = Some(op);
        let weak = self.window.clone();
        self.with_state(|state| {
            state.set_screen(3);
            state.set_run_progress(0);
            state.set_run_status("Starting…".into());
            let initial_log = seed_log
                .map(|text| vec![LogLine { text, is_error: false }])
                .unwrap_or_default();
            state.set_run_log(ModelRc::new(VecModel::from(initial_log)));
            state.set_run_done(false);
            state.set_run_failed(false);
            state.set_run_title(title.into());
            state.set_busy(true);
        });
        request_redraw(&weak);
        self.run_pending();
    }

    pub fn run_pending(&self) {
        let op = self.pending.lock().unwrap().take();
        let Some(op) = op else {
            return;
        };

        let client = self.client.clone();
        let weak = self.window.clone();
        let (event_tx, event_rx) = mpsc::channel::<Event>();

        thread::spawn(move || {
            while let Ok(event) = event_rx.recv() {
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak.upgrade() {
                        let state = w.global::<AppState>();
                        match event {
                            Event::Log { level, message } => {
                                append_run_log(&state, message, level == Verbosity::Error);
                            }
                            Event::Progress { percent, message } => {
                                state.set_run_progress(percent as i32);
                                state.set_run_status(message.into());
                            }
                            Event::Confirm { reply, .. } => {
                                let _ = reply.send(true);
                            }
                        }
                        request_redraw(&weak);
                    }
                });
            }
        });

        let weak = self.window.clone();
        thread::spawn(move || {
            let result = match &op {
                PendingOp::Install(targets) => client.install(targets.clone(), event_tx.clone()),
                PendingOp::Remove(ids) => client.uninstall(ids.clone(), event_tx.clone()),
                PendingOp::UpdateIndex => client.update_index(event_tx.clone()),
                PendingOp::AddRepo(url) => {
                    let r = client.add_repository(url, event_tx.clone());
                    match &r {
                        Ok(repo) => {
                            let _ = event_tx.send(Event::Log {
                                level: Verbosity::Info,
                                message: format!("Added repository '{}'", repo.id),
                            });
                        }
                        Err(e) => {
                            let _ = event_tx.send(Event::Log {
                                level: Verbosity::Error,
                                message: e.to_string(),
                            });
                        }
                    }
                    r.map(|_| ())
                }
            };
            drop(event_tx);

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    state.set_run_done(true);
                    state.set_run_failed(result.is_err());
                    state.set_busy(false);
                    if result.is_err() {
                        let err = result.as_ref().map_err(|e| e.to_string()).unwrap_err();
                        append_run_log(&state, err.clone(), true);
                        state.set_run_status(err.into());
                    } else {
                        state.set_run_status("Finished".into());
                    }
                    request_redraw(&w.as_weak());
                }
            });
        });
    }

    pub fn run_dismissed(&self) {
        let done = self.with_state(|s| s.get_run_done()).unwrap_or(false);
        if !done {
            return;
        }
        let back_to_detail = self
            .with_state(|s| !s.get_detail_id().is_empty())
            .unwrap_or(false);
        self.with_state(|state| {
            state.set_screen(if back_to_detail { 1 } else { 0 });
        });
        self.refresh_home();
        self.refresh_current_tab();
        if back_to_detail {
            let Some((repo, id)) = self.with_state(|s| {
                (s.get_detail_repo().to_string(), s.get_detail_id().to_string())
            }) else {
                return;
            };
            self.open_package(repo, id);
        }
    }

    pub fn remove_repo(&self, id: String) {
        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let result = client.remove_repository(&id);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match result {
                        Ok(()) => {
                            state.set_show_repo_packages(false);
                            state.set_browsing_repo_id("".into());
                            state.set_browsing_repo_name("".into());
                            state.set_status_text("Repository removed".into());
                        }
                        Err(e) => state.set_status_text(format!("Error: {e}").into()),
                    }
                }
            });
        });

        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let result = client.list_repositories();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    if let Ok(repos) = result {
                        let rows: Vec<RepoRow> = repos.iter().map(repo_row).collect();
                        state.set_repos(ModelRc::new(VecModel::from(rows)));
                    }
                }
            });
        });
    }

    pub fn request_launch(&self) {
        let Some((id, title)) = self.with_state(|state| {
            (
                state.get_detail_id().to_string(),
                state.get_detail_title().to_string(),
            )
        }) else {
            return;
        };

        let client = self.client.clone();
        let weak = self.window.clone();
        thread::spawn(move || {
            let result = client.launch(&id);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let state = w.global::<AppState>();
                    match result {
                        Ok(()) => state.set_status_text(format!("Launched {title}").into()),
                        Err(e) => state.set_status_text(format!("Error: {e}").into()),
                    }
                }
            });
        });
    }
}

impl PendingOp {
    fn clone_op(&self) -> Self {
        match self {
            Self::Install(t) => Self::Install(t.clone()),
            Self::Remove(i) => Self::Remove(i.clone()),
            Self::UpdateIndex => Self::UpdateIndex,
            Self::AddRepo(u) => Self::AddRepo(u.clone()),
        }
    }
}

fn package_row(pkg: &Package) -> PackageRow {
    PackageRow {
        id: pkg.id.clone().into(),
        repo: pkg.repository.clone().into(),
        name: pkg.name.clone().into(),
        subtitle: pkg.author.clone().into(),
        badge: "".into(),
    }
}

fn installed_row(pkg: &InstalledPackage) -> PackageRow {
    let mut badge = pkg.version.to_string();
    if pkg.installed_as_dependency {
        badge.push_str(" · dep");
    }
    PackageRow {
        id: pkg.id.clone().into(),
        repo: pkg.repository.clone().unwrap_or_default().into(),
        name: pkg.name.clone().into(),
        subtitle: pkg.author.clone().into(),
        badge: badge.into(),
    }
}

fn upgrade_row(up: &Upgrade) -> PackageRow {
    PackageRow {
        id: up.installed.id.clone().into(),
        repo: up.available.repository.clone().into(),
        name: up.installed.name.clone().into(),
        subtitle: up.installed.author.clone().into(),
        badge: format!("{} → {}", up.installed.version, up.available.version).into(),
    }
}

fn repo_row(repo: &Repository) -> RepoRow {
    RepoRow {
        id: repo.id.clone().into(),
        name: repo.name.clone().into(),
        url: repo.url.clone().into(),
        description: repo.description.clone().into(),
    }
}

fn request_redraw(weak: &slint::Weak<AppWindow>) {
    if let Some(w) = weak.upgrade() {
        w.window().request_redraw();
    }
}

fn append_run_log(state: &AppState<'_>, message: String, is_error: bool) {
    let log = state.get_run_log();
    let mut lines: Vec<LogLine> = (0..log.row_count())
        .filter_map(|i| log.row_data(i))
        .collect();

    // libkpm streams subprocess output one character at a time; fold fragments
    // into the current line until we see a newline.
    if !is_error
        && !message.contains('\n')
        && message.len() <= 4
        && lines.last().is_some_and(|line| !line.is_error)
    {
        let last = lines.pop().expect("checked non-empty");
        let mut text = last.text.to_string();
        text.push_str(&message);
        lines.push(LogLine {
            text: text.into(),
            is_error: false,
        });
    } else {
        for (i, part) in message.split('\n').enumerate() {
            if i == 0 {
                if !is_error && lines.last().is_some_and(|line| !line.is_error) && !part.is_empty()
                {
                    let last = lines.pop().expect("checked non-empty");
                    let mut text = last.text.to_string();
                    text.push_str(part);
                    lines.push(LogLine {
                        text: text.into(),
                        is_error: false,
                    });
                } else if !part.is_empty() || is_error {
                    lines.push(LogLine {
                        text: part.into(),
                        is_error,
                    });
                }
            } else if !part.is_empty() || is_error {
                lines.push(LogLine {
                    text: part.into(),
                    is_error,
                });
            }
        }
    }

    if lines.len() > RUN_LOG_CAP {
        lines.drain(0..lines.len() - RUN_LOG_CAP);
    }
    state.set_run_log(ModelRc::new(VecModel::from(lines)));
}

pub fn set_compat_error(app: &AppWindow, title: &str, body: &str) {
    let state = app.global::<AppState>();
    state.set_compatible(false);
    state.set_compat_title(title.into());
    state.set_compat_body(format!("{body}{COMPAT_GUIDANCE}").into());
}

fn strip_last_grapheme(text: &str) -> String {
    let count = text.chars().count();
    if count == 0 {
        return String::new();
    }
    text.chars().take(count - 1).collect()
}

fn keyboard_edit_target(state: AppState<'_>, append: Option<&str>, backspace: bool) {
    let target = state.get_keyboard_target();
    let current = match target {
        1 => state.get_add_repo_url().to_string(),
        2 => state.get_search_query().to_string(),
        _ => return,
    };
    let updated = if backspace {
        strip_last_grapheme(&current)
    } else if let Some(text) = append {
        format!("{current}{text}")
    } else {
        return;
    };
    match target {
        1 => state.set_add_repo_url(updated.into()),
        2 => state.set_search_query(updated.into()),
        _ => {}
    }
}
