//! Integration tests for [`kpm::mock::MockBackend`] through [`kpm::client::KpmClient`].

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use kpm::client::KpmClient;
use kpm::mock::MockBackend;
use kpm::{Error, Event, InstallTarget, SemVer};

fn mock_client() -> KpmClient {
    KpmClient::start(Box::new(MockBackend::new()))
}

#[test]
fn search_finds_koreader_case_insensitive() {
    let client = mock_client();

    let results = client.search("ko").unwrap();
    assert!(
        results.iter().any(|p| p.id == "koreader"),
        "search('ko') should find koreader"
    );

    let upper = client.search("KO").unwrap();
    assert!(
        upper.iter().any(|p| p.id == "koreader"),
        "search should be case-insensitive"
    );
    assert_eq!(upper, results, "case should not change results");
}

#[test]
fn search_empty_query_returns_all_packages() {
    let client = mock_client();

    let all = client.search("").unwrap();
    assert_eq!(
        all.len(),
        9,
        "empty query should match every indexed package"
    );

    let by_repo: usize = ["kindlemodding", "community"]
        .iter()
        .map(|repo| client.list_repository_packages(repo).unwrap().len())
        .sum();
    assert_eq!(all.len(), by_repo);
}

#[test]
fn list_upgrades_reports_only_koreader() {
    let client = mock_client();

    let upgrades = client.list_upgrades().unwrap();
    assert_eq!(upgrades.len(), 1);
    assert_eq!(upgrades[0].installed.id, "koreader");
    assert!(
        !upgrades.iter().any(|u| u.installed.id == "kterm"),
        "local kterm must never appear as an upgrade"
    );
}

#[test]
fn install_records_max_version_and_clears_upgrade() {
    let client = mock_client();
    let (events_tx, _events_rx) = channel();

    client
        .install(vec![InstallTarget::by_id("kindlefetch")], events_tx)
        .unwrap();

    let installed = client.get_installed("kindlefetch").unwrap().unwrap();
    assert_eq!(
        installed.version,
        SemVer {
            major: 1,
            minor: 3,
            patch: 1,
        },
        "install should pick the newest indexed version"
    );

    let upgrades = client.list_upgrades().unwrap();
    assert!(
        !upgrades.iter().any(|u| u.installed.id == "kindlefetch"),
        "freshly installed package at max version should not be listed"
    );
}

#[test]
fn uninstall_unknown_id_returns_kpm_error() {
    let client = mock_client();
    let (events_tx, _events_rx) = channel();

    let err = client
        .uninstall(vec!["no-such-package".into()], events_tx)
        .unwrap_err();

    match err {
        Error::Kpm { code, name } => {
            assert_eq!(code, 2);
            assert_eq!(name, "KPM_GENERIC_ERROR");
        }
        other => panic!("expected Kpm error code 2, got {other:?}"),
    }
}

#[test]
fn install_streams_progress_and_log_while_in_flight() {
    let client = mock_client();
    let (events_tx, events_rx) = channel::<Event>();

    let install_client = client.clone();
    let install_handle = thread::spawn(move || {
        install_client.install(vec![InstallTarget::by_id("blockamazon")], events_tx)
    });

    let mut saw_progress_100 = false;
    let mut saw_log = false;
    let mut events_before_done = false;

    while !install_handle.is_finished() {
        while let Ok(event) = events_rx.try_recv() {
            events_before_done = true;
            match event {
                Event::Progress { percent, .. } if percent == 100 => saw_progress_100 = true,
                Event::Log { .. } => saw_log = true,
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    install_handle.join().unwrap().unwrap();

    while let Ok(event) = events_rx.try_recv() {
        match event {
            Event::Progress { percent, .. } if percent == 100 => saw_progress_100 = true,
            Event::Log { .. } => saw_log = true,
            _ => {}
        }
    }

    assert!(
        events_before_done,
        "events should arrive while install is still in flight"
    );
    assert!(
        saw_progress_100,
        "install should emit Progress reaching 100"
    );
    assert!(saw_log, "install should emit at least one Log event");
}

#[test]
fn add_repository_validates_url_and_appends() {
    let client = mock_client();
    let (events_tx, _events_rx) = channel();

    let err = client
        .add_repository("not-a-url", events_tx.clone())
        .unwrap_err();
    assert!(
        matches!(err, Error::Other(_)),
        "invalid URL should error, got {err:?}"
    );

    let before = client.list_repositories().unwrap().len();
    let repo = client
        .add_repository("https://example.org/new-repo/manifest.json", events_tx)
        .unwrap();

    let repos = client.list_repositories().unwrap();
    assert_eq!(repos.len(), before + 1);
    assert!(repos.iter().any(|r| r.url == repo.url));
}
