//! Raw `#[repr(C)]` types mirroring `vendor/KPM/src/include/kpm/kpm.h` and
//! `semver.h` at the pinned submodule commit.
//!
//! These layouts are hand-maintained (rather than bindgen-at-build-time) so
//! that building Kinstaller never requires libclang; they MUST be updated in
//! lockstep with the `vendor/KPM` submodule pin, and the compatibility table
//! gate guarantees they are never used against an unverified libkpm build.

#![allow(non_snake_case, non_camel_case_types)]

use std::os::raw::{c_char, c_int, c_uint, c_void};

/// `enum KPMResult`
pub const KPM_OK: c_int = 0;
pub const KPM_ABORTED: c_int = 1;
pub const KPM_GENERIC_ERROR: c_int = 2;
pub const KPM_SQLITE_ERROR: c_int = 3;
pub const KPM_CURL_ERROR: c_int = 4;
pub const KPM_INVALID_RESPONSE_CODE: c_int = 5;
pub const KPM_INVALID_RESPONSE_CONTENT: c_int = 6;
pub const KPM_FILE_SYSTEM_ERROR: c_int = 7;
pub const KPM_LIBARCHIVE_ERROR: c_int = 8;
pub const KPM_PARSE_ERROR: c_int = 9;

/// `enum KPMVerbosity`
pub const KPM_VERBOSITY_DEBUG: c_int = 0;
pub const KPM_VERBOSITY_INFO: c_int = 1;
pub const KPM_VERBOSITY_WARN: c_int = 2;
pub const KPM_VERBOSITY_ERROR: c_int = 3;

/// `struct SemVer` (semver.h)
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: c_uint,
    pub minor: c_uint,
    pub patch: c_uint,
}

/// `struct Repository`
#[repr(C)]
#[derive(Debug)]
pub struct Repository {
    pub id: *mut c_char,
    pub url: *mut c_char,
    pub name: *mut c_char,
    pub description: *mut c_char,
}

/// `struct IndexedPackage`
#[repr(C)]
#[derive(Debug)]
pub struct IndexedPackage {
    pub repository: *mut c_char,
    pub id: *mut c_char,
    pub name: *mut c_char,
    pub author: *mut c_char,
    pub description: *mut c_char,
}

/// `struct IndexedArtifact`
#[repr(C)]
#[derive(Debug)]
pub struct IndexedArtifact {
    pub repository: *mut c_char,
    pub id: *mut c_char,
    pub url: *mut c_char,
    pub version: SemVer,
}

/// `struct ArtifactDependency`
#[repr(C)]
#[derive(Debug)]
pub struct ArtifactDependency {
    pub artifact_repository: *mut c_char,
    pub artifact_id: *mut c_char,
    pub artifact_url: *mut c_char,
    pub id: *mut c_char,
    pub min_version: SemVer,
    pub max_version: SemVer,
}

/// `struct InstalledPackage`
#[repr(C)]
#[derive(Debug)]
pub struct InstalledPackage {
    pub id: *mut c_char,
    pub repository: *mut c_char,
    pub name: *mut c_char,
    pub author: *mut c_char,
    pub description: *mut c_char,
    pub version: SemVer,
    pub installed_as_dependency: bool,
}

/// `struct InstalledDependency`
#[repr(C)]
#[derive(Debug)]
pub struct InstalledDependency {
    pub dependent: *mut c_char,
    pub dependency_id: *mut c_char,
    pub min_version: SemVer,
    pub max_version: SemVer,
}

/// `struct InstallTarget`
#[repr(C)]
#[derive(Debug)]
pub struct InstallTarget {
    pub repository: *mut c_char,
    pub id: *mut c_char,
    pub version: *mut SemVer,
}

/// `struct KPM`
#[repr(C)]
#[derive(Debug)]
pub struct KPM {
    pub db: *mut c_void, // sqlite3*
    pub dbPath: *mut c_char,
    pub pkgPath: *mut c_char,
    pub maxConnections: c_int,
}

/// `struct KPMIO` — function pointers to *variadic* C callbacks. The
/// trampolines are provided by the C shim (see `shim/kpmio_shim.c`); Rust
/// never defines these directly.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KPMIO {
    pub log: *mut c_void,         // KPMLog*
    pub stream: *mut c_void,      // KPMStream*
    pub logProgress: *mut c_void, // KPMLogProgress*
    pub getInput: *mut c_void,    // KPMGetInput*
}

/// Default KPM paths (matching `meson.options` / `paths.md` upstream).
pub const KPM_DB_PATH: &str = "/mnt/us/kmc/kpm/kpm.db";
pub const KPM_PKG_PATH: &str = "/mnt/us/kmc/kpm/packages";
