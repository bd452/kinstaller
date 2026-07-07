//! The libkpm compatibility gate: identity check + compiled-in whitelist.
//!
//! Before Kinstaller calls into libkpm it must know the library speaks the
//! exact ABI our `types` module was written against. `dlsym` cannot detect
//! struct-layout changes, so the policy is default-deny: we hash the installed
//! `libkpm.so` and only proceed if the hash appears in the compiled-in table
//! of verified releases ([`crate::compat_table::COMPAT_TABLE`]).

use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

pub use crate::compat_table::COMPAT_TABLE;

/// One verified libkpm build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompatEntry {
    /// Upstream KPM release version (informational).
    pub kpm_version: &'static str,
    /// KPM platform name (`kindlehf` / `kindlepw2`).
    pub platform: &'static str,
    /// Lower-hex SHA-256 of `libkpm.so`.
    pub sha256: &'static str,
}

/// The KPM platform this Kinstaller binary was built for.
///
/// Determined at compile time: the hard-float target is `kindlehf`, the
/// soft-float target is `kindlepw2`. (KPM installs both platform trees on
/// every device; the correct one is the one matching our own ABI.)
pub const KPM_PLATFORM: &str = if cfg!(target_abi = "eabihf") {
    "kindlehf"
} else {
    "kindlepw2"
};

/// Root of the on-device KMC install.
pub const KMC_ROOT: &str = "/var/local/kmc";

/// Identity of an installed libkpm, as measured on this device.
#[derive(Debug, Clone)]
pub struct KpmIdentity {
    /// Path of the library that was measured.
    pub lib_path: PathBuf,
    /// Lower-hex SHA-256 of the library file.
    pub sha256: String,
    /// Version reported by the `kpm` CLI (ships in lockstep with the `.so`),
    /// e.g. `0.2.1`. `None` if the CLI could not be run or parsed.
    pub cli_version: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CompatError {
    #[error("libkpm not found at {0} — is KPM installed?")]
    LibNotFound(PathBuf),
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "installed libkpm is not a verified build (version: {}, sha256: {sha256}); \
         this Kinstaller supports KPM {supported}",
        .cli_version.as_deref().unwrap_or("unknown")
    )]
    Unverified {
        cli_version: Option<String>,
        sha256: String,
        supported: String,
    },
}

/// Default path of the device-installed libkpm for this binary's platform.
pub fn default_lib_path() -> PathBuf {
    Path::new(KMC_ROOT)
        .join(KPM_PLATFORM)
        .join("lib")
        .join("libkpm.so")
}

/// Default path of the device-installed kpm CLI for this binary's platform.
pub fn default_cli_path() -> PathBuf {
    Path::new(KMC_ROOT)
        .join(KPM_PLATFORM)
        .join("bin")
        .join("kpm")
}

fn sha256_file(path: &Path) -> Result<String, CompatError> {
    let bytes = std::fs::read(path).map_err(|source| CompatError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Ask the co-shipped CLI for its libkpm version ("libkpm vX.Y.Z" line).
fn query_cli_version(cli_path: &Path) -> Option<String> {
    let output = Command::new(cli_path).arg("version").output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("libkpm v") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Measure the identity of the libkpm at `lib_path`.
pub fn measure(lib_path: &Path, cli_path: Option<&Path>) -> Result<KpmIdentity, CompatError> {
    if !lib_path.exists() {
        return Err(CompatError::LibNotFound(lib_path.to_path_buf()));
    }
    Ok(KpmIdentity {
        lib_path: lib_path.to_path_buf(),
        sha256: sha256_file(lib_path)?,
        cli_version: cli_path.and_then(query_cli_version),
    })
}

/// A human-readable list of supported KPM versions for this platform.
pub fn supported_versions() -> String {
    let mut versions: Vec<&str> = COMPAT_TABLE
        .iter()
        .filter(|e| e.platform == KPM_PLATFORM)
        .map(|e| e.kpm_version)
        .collect();
    versions.dedup();
    if versions.is_empty() {
        "(none)".to_string()
    } else {
        versions.join(", ")
    }
}

/// Verify `identity` against the compiled-in whitelist (default-deny).
pub fn verify(identity: &KpmIdentity) -> Result<CompatEntry, CompatError> {
    COMPAT_TABLE
        .iter()
        .find(|e| e.sha256 == identity.sha256)
        .copied()
        .ok_or_else(|| CompatError::Unverified {
            cli_version: identity.cli_version.clone(),
            sha256: identity.sha256.clone(),
            supported: supported_versions(),
        })
}
