//! GENERATED FILE — do not edit by hand.
//!
//! Regenerate with `scripts/gen-compat-table.sh`, which downloads the pinned
//! official KPM release artifact, extracts each platform's `libkpm.so`, and
//! records its SHA-256 here. Review and commit the diff.
//!
//! Source artifact: https://repo.kindlemodding.org/packages/kpm/artifacts/kpm_0.2.1_kindlehf-kindlepw2-compat.kpkg

use crate::compat::CompatEntry;

/// libkpm builds verified ABI-compatible with this Kinstaller's bindings
/// (`vendor/KPM` @ 799adf431223d2cfa782a6a4ad07d809f120100b, kpm.h v0.2.x).
pub const COMPAT_TABLE: &[CompatEntry] = &[
    CompatEntry {
        kpm_version: "0.2.1",
        platform: "kindlehf",
        sha256: "49ec85cb73ccf540566e1ca0f8e18a9d97a131159c56d2fcad5b05cc17040709",
    },
    CompatEntry {
        kpm_version: "0.2.1",
        platform: "kindlepw2",
        sha256: "6dd64391c9ed3dafd958ba7835ccbac7f64f0c1076dab9399514eee0d6c332e8",
    },
    // 0.2.2 bumps the package/CLI version string only; libkpm.so is unchanged
    // (verified against KindleModding/KPM CI @ 799adf431223d2cfa782a6a4ad07d809f120100b).
    CompatEntry {
        kpm_version: "0.2.2",
        platform: "kindlehf",
        sha256: "49ec85cb73ccf540566e1ca0f8e18a9d97a131159c56d2fcad5b05cc17040709",
    },
    CompatEntry {
        kpm_version: "0.2.2",
        platform: "kindlepw2",
        sha256: "6dd64391c9ed3dafd958ba7835ccbac7f64f0c1076dab9399514eee0d6c332e8",
    },
];
