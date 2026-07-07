# Kinstaller

A Cydia-style GUI package manager for jailbroken Kindle devices, powered by
[KPM](https://github.com/KindleModding/KPM) (the Kindle Package Manager) under the hood.

Kinstaller runs on-device, drawing directly to the e-ink framebuffer, and gives you
Cydia-like tabs — Home, Sources, Changes, Installed, Search — on top of the same
`libkpm.so` the official `kpm` CLI uses.

## How it works

- **UI**: [Slint](https://slint.dev) with the software renderer and
  [`slint-backend-kindle`](https://github.com/sverrejb/slint-kindle-backend) (no X11 needed).
- **Backend**: the device-installed `libkpm.so` (`/var/local/kmc/<platform>/lib/libkpm.so`)
  is loaded at runtime with `dlopen`/`dlsym`. Kinstaller never bundles its own KPM, so it can
  never disagree with the on-device install.
- **Safety gate**: before any KPM call, Kinstaller hashes the installed `libkpm.so` and
  checks it against a compiled-in compatibility table of verified KPM releases. Unknown
  versions get a friendly full-screen error instead of a call into an ABI-incompatible
  library (default-deny).

## Supported devices

Same platforms as KPM itself, on a modern jailbreak stack (hdnext / SpringBreak):

| Platform   | Devices                                   | Rust target                     |
| ---------- | ----------------------------------------- | ------------------------------- |
| `kindlehf` | Any Kindle on FW >= 5.16.3                | `armv7-unknown-linux-gnueabihf` |
| `kindlepw2`| PW2 and newer on FW < 5.16.3 (best effort)| `armv7-unknown-linux-gnueabi`   |

## Building

First-time setup (fonts, submodules, Rust targets):

```bash
./scripts/setup.sh
```

### UI development (any OS, no cross tools)

Runs with the Slint desktop backend and a mocked KPM backend:

```bash
cargo run -p kinstaller --features mock-backend
```

### Device builds (macOS via OrbStack, Linux native)

Device binaries must be **dynamically linked against Kindle glibc** so they can
`dlopen()` the on-device `libkpm.so`. That requires the KindleModding **koxtoolchain**
(same as KPM CI).

**macOS:** install [OrbStack](https://orbstack.dev), then:

```bash
./scripts/setup-orbstack.sh      # once: OrbStack + Linux build image
./scripts/build-target.sh kindlehf
./scripts/build-target.sh kindlepw2
./scripts/pack.sh                # produce the .kpkg
```

`build-target.sh` runs the compile inside a Linux container automatically. Output lands in
`dist/<platform>/kinstaller` on your Mac filesystem.

**Linux:** koxtoolchain runs natively (`./scripts/setup-koxtoolchain.sh`).

**UI-only smoke test on macOS** (no libkpm; Slint only):

```bash
brew install zig && cargo install cargo-zigbuild
./scripts/build-target.sh --musl kindlehf
```

### Regenerating the KPM compatibility table

```bash
./scripts/gen-compat-table.sh
```

Downloads the pinned official KPM release artifact, hashes each platform's `libkpm.so`,
and regenerates `crates/kpm-sys/src/compat_table.rs`. Review and commit the diff.

## Repository layout

- `crates/kpm-sys` — raw FFI types matching `kpm.h`, runtime loader, compatibility gate
- `crates/kpm` — safe Rust wrapper: owned types, worker-thread job queue, IO bridging, mock backend
- `crates/kinstaller` — the Slint app
- `vendor/KPM` — pinned KPM source submodule (supplies `kpm.h`; not compiled for device builds)
- `package/` — KPM packaging of Kinstaller itself (`manifest.json`, hooks, KUAL scriptlet)
- `scripts/` — build, packaging, and table-generation scripts

## License

GPL-3.0-or-later (required: Kinstaller is a derivative work of the GPL-licensed libkpm API).
