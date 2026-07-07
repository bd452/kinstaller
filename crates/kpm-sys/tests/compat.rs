//! Integration tests for the libkpm compatibility gate.

use std::io::Write;
use std::path::PathBuf;

use kpm_sys::compat::{self, CompatError, KpmIdentity, COMPAT_TABLE, KPM_PLATFORM};

/// FIPS 180-2 SHA-256 test vector for the byte string `abc`.
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn platform_entry() -> &'static compat::CompatEntry {
    COMPAT_TABLE
        .iter()
        .find(|e| e.platform == KPM_PLATFORM)
        .expect("compat table should contain an entry for this platform")
}

#[test]
fn verify_accepts_known_hash() {
    let entry = platform_entry();
    let identity = KpmIdentity {
        lib_path: compat::default_lib_path(),
        sha256: entry.sha256.to_string(),
        cli_version: Some(entry.kpm_version.to_string()),
    };

    let verified = compat::verify(&identity).unwrap();
    assert_eq!(verified, *entry);
}

#[test]
fn verify_rejects_unknown_hash_with_supported_list() {
    let identity = KpmIdentity {
        lib_path: PathBuf::from("/tmp/not-a-real-libkpm.so"),
        sha256: "0".repeat(64),
        cli_version: None,
    };

    let err = compat::verify(&identity).unwrap_err();
    let supported = compat::supported_versions();
    let display = err.to_string();

    match err {
        CompatError::Unverified {
            supported: listed, ..
        } => {
            assert_eq!(listed, supported);
            assert!(display.contains(&listed));
            assert!(display.contains("0.2.1"));
        }
        other => panic!("expected Unverified, got {other:?}"),
    }
}

#[test]
fn measure_hashes_temp_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("libkpm.so");
    let mut file = std::fs::File::create(&path).expect("create temp file");
    file.write_all(b"abc").expect("write temp file");
    drop(file);

    let identity = compat::measure(&path, None).unwrap();
    assert_eq!(identity.sha256, ABC_SHA256);
    assert_eq!(identity.lib_path, path);
}

#[test]
fn measure_missing_path_returns_lib_not_found() {
    let missing = PathBuf::from("/no/such/libkpm.so");
    let err = compat::measure(&missing, None).unwrap_err();

    match err {
        CompatError::LibNotFound(path) => assert_eq!(path, missing),
        other => panic!("expected LibNotFound, got {other:?}"),
    }
}

#[test]
fn supported_versions_lists_platform_release_once() {
    let versions = compat::supported_versions();
    assert_eq!(
        versions.matches("0.2.1").count(),
        1,
        "supported_versions should list 0.2.1 exactly once, got '{versions}'"
    );
}
