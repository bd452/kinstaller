//! `kpm-sys`: raw FFI layer for libkpm (the Kindle Package Manager library).
//!
//! - [`types`]: `#[repr(C)]` mirrors of `kpm.h` structs at the pinned
//!   `vendor/KPM` commit.
//! - [`api`]: a `dlopen`/`dlsym` loader ([`api::KpmApi`]) for the
//!   device-installed `libkpm.so` — no link-time binding.
//! - [`io`]: bridges libkpm's variadic `KPMIO` callbacks (via a small C shim)
//!   to a registered Rust handler.
//! - [`compat`]: the identity check + compiled-in compatibility table gate
//!   that must pass before any libkpm call (default-deny).
//!
//! The intended entry point is [`load_verified`].

pub mod api;
pub mod compat;
mod compat_table;
pub mod io;
pub mod types;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("could not load {0}: {1}")]
    Open(String, String),
    #[error("symbol {0} missing from libkpm: {1}")]
    Symbol(&'static str, String),
    #[error(transparent)]
    Compat(#[from] compat::CompatError),
}

/// How the library was selected, for diagnostics.
#[derive(Debug, Clone)]
pub struct LoadedKpm {
    pub identity: compat::KpmIdentity,
    /// The verified table entry, or `None` when loaded through the debug
    /// override (`KINSTALLER_KPM_LIB`).
    pub entry: Option<compat::CompatEntry>,
}

/// Load and verify the device-installed libkpm.
///
/// Runs the compatibility gate (hash + whitelist) against
/// `/var/local/kmc/<platform>/lib/libkpm.so` and only `dlopen`s the library if
/// it is a verified build.
///
/// Debug builds honor the `KINSTALLER_KPM_LIB` environment variable, which
/// loads an arbitrary local `libkpm.so` *bypassing the hash check* (for
/// development against locally-built KPM). This override is compiled out of
/// release builds.
///
/// # Safety
/// Calling into the returned [`api::KpmApi`] is only sound while the ABI
/// assumptions of [`types`] hold; that is exactly what the gate enforces for
/// non-override loads. With the debug override, the caller vouches for the
/// library.
pub unsafe fn load_verified() -> Result<(api::KpmApi, LoadedKpm), LoadError> {
    #[cfg(debug_assertions)]
    if let Ok(override_path) = std::env::var("KINSTALLER_KPM_LIB") {
        let path = PathBuf::from(override_path);
        let identity = compat::measure(&path, None)?;
        let api = api::KpmApi::load(&path)?;
        return Ok((
            api,
            LoadedKpm {
                identity,
                entry: None,
            },
        ));
    }

    let lib_path: PathBuf = compat::default_lib_path();
    let cli_path = compat::default_cli_path();
    let identity = compat::measure(&lib_path, Some(&cli_path))?;
    let entry = compat::verify(&identity)?;
    let api = api::KpmApi::load(&lib_path)?;
    Ok((
        api,
        LoadedKpm {
            identity,
            entry: Some(entry),
        },
    ))
}
