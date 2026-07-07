//! Safe, backend-agnostic interface to the Kindle Package Manager.
//!
//! The UI talks to [`client::KpmClient`], which runs all package operations on
//! a dedicated worker thread and streams [`Event`]s back while long jobs
//! (index update, install, uninstall) run.
//!
//! Two backends implement [`backend::Backend`]:
//! - `libkpm` feature: the real thing — `dlopen`s the device-installed
//!   `libkpm.so` through `kpm-sys` (after its compatibility gate passes).
//! - `mock` feature: an in-memory fake with simulated progress, so the UI can
//!   be developed natively on any OS.

pub mod backend;
pub mod client;
#[cfg(feature = "mock")]
pub mod mock;
#[cfg(feature = "libkpm")]
pub mod real;

use std::fmt;
use std::sync::mpsc::Sender;

/// A semantic version, mirroring KPM's `struct SemVer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A package repository KPM has indexed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    pub id: String,
    pub url: String,
    pub name: String,
    pub description: String,
}

/// A package available in a repository index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub repository: String,
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
}

/// A concrete downloadable version of a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub repository: String,
    pub package_id: String,
    pub url: String,
    pub version: SemVer,
}

/// A package installed on the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPackage {
    pub id: String,
    /// Repository it was installed from; `None` for local installs.
    pub repository: Option<String>,
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: SemVer,
    pub installed_as_dependency: bool,
}

/// An available upgrade (installed version -> newest indexed artifact).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upgrade {
    pub installed: InstalledPackage,
    pub available: Artifact,
}

/// What to install: a package id, optionally pinned to a repository/version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallTarget {
    pub repository: Option<String>,
    pub id: String,
    pub version: Option<SemVer>,
}

impl InstallTarget {
    pub fn by_id(id: impl Into<String>) -> Self {
        Self {
            repository: None,
            id: id.into(),
            version: None,
        }
    }
}

/// Log level, mirroring `enum KPMVerbosity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    Debug,
    Info,
    Warn,
    Error,
}

/// Events streamed from the backend while a job runs.
#[derive(Debug, Clone)]
pub enum Event {
    Log {
        level: Verbosity,
        message: String,
    },
    Progress {
        percent: u32,
        message: String,
    },
    /// The backend asks a yes/no question; send the answer on `reply`.
    /// If the receiver is dropped without replying, the answer is "no".
    Confirm {
        prompt: String,
        reply: Sender<bool>,
    },
}

/// A sink for job events plus the confirm-prompt channel.
#[derive(Clone)]
pub struct EventSink {
    tx: Sender<Event>,
    /// When `true`, confirmation prompts are auto-accepted (the UI has
    /// already shown its own confirm screen before queueing the job).
    pub auto_confirm: bool,
}

impl EventSink {
    pub fn new(tx: Sender<Event>, auto_confirm: bool) -> Self {
        Self { tx, auto_confirm }
    }

    pub fn log(&self, level: Verbosity, message: impl Into<String>) {
        let _ = self.tx.send(Event::Log {
            level,
            message: message.into(),
        });
    }

    pub fn progress(&self, percent: u32, message: impl Into<String>) {
        let _ = self.tx.send(Event::Progress {
            percent,
            message: message.into(),
        });
    }

    /// Ask the user a yes/no question, blocking until they answer.
    pub fn confirm(&self, prompt: impl Into<String>) -> bool {
        if self.auto_confirm {
            return true;
        }
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        if self
            .tx
            .send(Event::Confirm {
                prompt: prompt.into(),
                reply: reply_tx,
            })
            .is_err()
        {
            return false;
        }
        reply_rx.recv().unwrap_or(false)
    }
}

/// Information about the loaded backend, shown on the Home tab.
#[derive(Debug, Clone)]
pub struct BackendInfo {
    /// Human-readable backend description ("libkpm 0.2.1" / "mock").
    pub description: String,
    /// KPM version string, if known.
    pub kpm_version: Option<String>,
    /// SHA-256 identity of the loaded libkpm, if applicable.
    pub lib_sha256: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The compatibility gate rejected the installed libkpm (soft-fail).
    #[error("{0}")]
    Incompatible(String),
    /// libkpm could not be found/loaded.
    #[error("{0}")]
    Unavailable(String),
    /// A KPM operation failed with a `KPMResult` code.
    #[error("KPM error: {name} ({code})")]
    Kpm { code: i32, name: &'static str },
    /// The user aborted the operation at a confirmation prompt.
    #[error("operation aborted")]
    Aborted,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Map a raw `KPMResult` code to an [`Error`] (or `Ok(())`).
pub fn check_kpm_result(code: i32) -> Result<()> {
    let name = match code {
        0 => return Ok(()),
        1 => return Err(Error::Aborted),
        2 => "KPM_GENERIC_ERROR",
        3 => "KPM_SQLITE_ERROR",
        4 => "KPM_CURL_ERROR",
        5 => "KPM_INVALID_RESPONSE_CODE",
        6 => "KPM_INVALID_RESPONSE_CONTENT",
        7 => "KPM_FILE_SYSTEM_ERROR",
        8 => "KPM_LIBARCHIVE_ERROR",
        9 => "KPM_PARSE_ERROR",
        _ => "unknown",
    };
    Err(Error::Kpm { code, name })
}
