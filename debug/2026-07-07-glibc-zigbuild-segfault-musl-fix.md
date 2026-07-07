# Kinstaller instant crash on Kindle (2026-07-07)

## Symptom

Launching Kinstaller from **Documents → Kinstaller.sh** failed immediately with no visible UI and an empty log at `/mnt/us/kmc/kpm/tmp/kinstaller.log`.

Over SSH (`root@192.168.1.231`, password `kindle`):

```sh
/mnt/us/kmc/kpm/packages/kinstaller/bin/kindlehf/kinstaller
# Segmentation fault (exit 139)
```

`launch.sh` redirected stdout/stderr to the log file, but the process died during startup before writing anything useful — log size stayed **0 bytes**.

## Device environment

| Item | Value |
|------|-------|
| Hardware | Freescale i.MX6 SoloLite (Lab126 board) |
| CPU | ARMv7 Cortex-A9 (`CPU part: 0xc09`), hard-float + NEON |
| Kernel | `4.1.15-lab126` |
| Userspace glibc | **2.20** (`/lib/libc-2.20.so`) |
| KPM (from `kpm.db`) | **0.2.2**, platform **kindlehf** |
| Jailbreak | Monolithic jb.sh v1.2.1 (Hackerdude), telnet available when debug enabled |
| `ld.so.preload` | `/usr/lib/libenvload.so` (read-only rootfs; cannot disable) |
| Framebuffer | `/dev/fb0` present |

KPM CLI works:

```sh
/var/local/kmc/kindlehf/bin/kpm version
# Kindle Package Manager v0.2.2
```

## Investigation timeline

### 1. Ruled out packaging / launch script

- Package layout under `/mnt/us/kmc/kpm/packages/kinstaller/` was correct.
- `launch.sh` correctly `cd`s to the package dir and execs `./bin/kindlehf/kinstaller`.
- `Documents/Kinstaller.sh` correctly execs `launch.sh`.
- Running as `root` or `framework` — same segfault.

### 2. Ruled out KPM compat gate (for the crash)

The compat table was updated to include KPM **0.2.2** (same `libkpm.so` SHA-256 as 0.2.1; see below). Even if the gate failed, kinstaller would show the soft-fail UI — not segfault before any output.

### 3. Ruled out Slint / kinstaller logic specifically

Added trace writes to `/mnt/us/kmc/kpm/tmp/kinstaller.log` at the start of `main()` — **log file never created**. Crash happens **before `main()`**.

Built a minimal Rust smoke test (`fn main() { write("/mnt/us/kmc/kpm/tmp/smoke.log", "ok") }`) with the same **glibc cargo-zigbuild** toolchain — **also segfaults before main**.

### 4. Dynamic linker failure (root cause)

`LD_DEBUG=libs` on the glibc-linked smoke binary shows libc/libpthread/libdl/libgcc resolve successfully, then:

```
relocation processing: /tmp/kindle-smoke-nopie
symbol=statx; lookup in file=...
symbol=__cxa_thread_atexit_impl; lookup in file=...
symbol=stderr; lookup in file=...
Segmentation fault
```

The crash occurs during **relocation / startup**, not in application code. Kindle’s dynamic linker + `libenvload.so` + glibc **2.20** do not tolerate the glibc-linked PIE binaries produced by `cargo-zigbuild` for `armv7-unknown-linux-gnueabihf`.

### 5. Binary comparison: native KPM vs zigbuild

| Property | Native `kpm` (works) | `cargo-zigbuild` kinstaller (crashes) |
|----------|----------------------|---------------------------------------|
| `file` output | `ELF 32-bit LSB executable` | `ELF 32-bit LSB pie executable` → later `executable` with `-C relocation-model=static` |
| GNU/Linux tag | **4.1.0** | **2.0.0** |
| Built with | koxtoolchain (KindleModding CI) | cargo-zigbuild + Zig glibc 2.17 sysroot |
| GLIBC symbols | ≤ 2.7 | up to **2.17** (+ Rust std may reference newer symbols like `statx`) |

Non-PIE (`-C relocation-model=static`) glibc binaries **still segfault** — PIE alone is not the fix.

### 6. What works: static musl

```sh
cargo zigbuild --release --target armv7-unknown-linux-musleabihf
# file: ELF 32-bit LSB executable, ARM, statically linked
```

Deployed smoke test to Kindle:

```sh
/tmp/kindle-smoke-musl
# exit=0, wrote "smoke ok" to /mnt/us/kmc/kpm/tmp/smoke.log
```

Rebuilt kinstaller with musl — runs on device (verified with `timeout 2 launch.sh`, exit 143 = SIGTERM from timeout, not SIGSEGV).

## KPM 0.2.2 compatibility (separate from crash)

- Official repo manifest lists KPM artifacts only through **0.2.1**; no published `kpm_0.2.2` `.kpkg`.
- Device `kpm.db` registers **0.2.2** (from jailbreak KMC unpack).
- KPM CI artifact at submodule commit `799adf4` produces **identical** `libkpm.so` SHA-256 hashes to 0.2.1:
  - kindlehf: `49ec85cb73ccf540566e1ca0f8e18a9d97a131159c56d2fcad5b05cc17040709`
  - kindlepw2: `6dd64391c9ed3dafd958ba7835ccbac7f64f0c1076dab9399514eee0d6c332e8`
- `compat_table.rs` was extended with 0.2.2 entries (same hashes); `gen-compat-table.sh` now emits both 0.2.1 and 0.2.2 from the 0.2.1 artifact.

## Fix applied (2026-07-07, revised)

### musl interim (superseded)

Switched device cross-build to **static musl** so the UI would launch on device. musl
static executables **cannot `dlopen()`** device `libkpm.so` — kinstaller shows:

```
could not load /var/local/kmc/kindlehf/lib/libkpm.so: Dynamic loading not supported.
```

musl remains available as `./scripts/build-target.sh --musl` for UI-only smoke tests on
macOS, not for real KPM use.

### koxtoolchain glibc (current)

Device builds now use **KindleModding koxtoolchain** on Linux (same as KPM CI):

| Platform | Target | Linker | Glibc pin |
|----------|--------|--------|-----------|
| kindlehf | `armv7-unknown-linux-gnueabihf` | `arm-kindlehf-linux-gnueabihf-gcc` | ≤ 2.17 |
| kindlepw2 | `armv7-unknown-linux-gnueabi` | `arm-kindlepw2-linux-gnueabi-gcc` | ≤ 2.7 |

The kinstaller binary is **dynamically linked** against Kindle glibc 2.20 and can
`dlopen()` the device `libkpm.so` at runtime.

**Why not zig cc + koxtoolchain sysroot on macOS?**

- `zig cc --sysroot=…/koxtoolchain/sysroot` still emits **GLIBC_2.28–2.34** symbol
  requirements (zig bundles its own glibc/compiler-rt). Device rejects the binary before
  `main()` with version errors.
- `cargo-zigbuild` glibc **2.17** pins pass the glibc audit but **segfault during
  relocation** on device (same as original crash) — non-PIE does not fix it.
- `cargo +nightly -Z build-std` with the koxtoolchain sysroot linker wrapper still pulls
  GLIBC_2.28+ (tested 2026-07-07).

**Why not run koxtoolchain on macOS directly?**

koxtoolchain host tools are **Linux x86_64** ELFs — they cannot run natively on macOS.
Use **OrbStack** (`./scripts/setup-orbstack.sh`) to run the Linux build container locally,
or CI `device-build` artifacts.

### OrbStack on macOS (2026-07-07)

```sh
./scripts/setup-orbstack.sh   # once
./scripts/build-target.sh kindlehf
# dist/kindlehf/kinstaller — dynamically linked, GLIBC ≤ 2.18, GNU/Linux 4.1.0
```

Container image uses `--platform linux/amd64` (koxtoolchain gcc is x86_64 even on Apple
Silicon Macs).

Files changed:

- `docker/Dockerfile` — Ubuntu amd64 + Rust + koxtoolchain
- `scripts/build-in-container.sh` — OrbStack/Docker wrapper
- `scripts/setup-orbstack.sh` — install OrbStack + prebuild image
- `scripts/build-target.sh` — auto-delegates to container on macOS

## macOS cross-compile notes

- **koxtoolchain** prebuilt tarball contains **Linux x86_64** host tools — cannot run
  `arm-kindlehf-linux-gnueabihf-gcc` on macOS. Use Linux CI artifacts or `--musl` for
  UI-only testing.
- `zig cc` + koxtoolchain sysroot does **not** produce Kindle-compatible glibc versions
  from macOS (see Fix applied above).
- `RUST_FONTCONFIG_DLOPEN=1` alone is insufficient for glibc Slint cross-build on macOS;
  direct `fontique` dependency with `fontconfig-dlopen` feature unification was required
  for the Linux-target Slint build.

## Verification (2026-07-07)

```sh
# On Mac
./scripts/build-target.sh kindlehf
# Wrote dist/kindlehf/kinstaller — statically linked musl

# Deploy + test on Kindle
scp dist/kindlehf/kinstaller root@192.168.1.231:/mnt/us/kmc/kpm/packages/kinstaller/bin/kindlehf/
ssh root@192.168.1.231 'cd /mnt/us/kmc/kpm/packages/kinstaller && timeout 2 sh launch.sh'
# exit 143 (timeout SIGTERM) — process ran, no segfault
```

## Process cleanup (2026-07-07)

Checked Kindle via SSH — **no kinstaller or kindle-smoke test processes running** at time of cleanup. Earlier `timeout` tests had already exited.

## Recommended dev loop

**macOS (OrbStack + koxtoolchain, full KPM / dlopen):**

```sh
./scripts/setup-orbstack.sh   # once
./scripts/build-target.sh kindlehf
scp dist/kindlehf/kinstaller root@192.168.1.231:/mnt/us/kmc/kpm/packages/kinstaller/bin/kindlehf/kinstaller
# Relaunch Documents → Kinstaller.sh
ssh root@192.168.1.231 'tail -f /mnt/us/kmc/kpm/tmp/kinstaller.log'
```

**macOS (UI-only, no libkpm):**

```sh
./scripts/build-target.sh --musl kindlehf
# deploy as above — expect soft-fail / dlopen error until a Linux-built binary is used
```

UI iteration on desktop still uses `cargo run -p kinstaller --features mock-backend`.

## Open follow-ups

- Deploy kox-built binary to Kindle and confirm libkpm dlopen + full UI (not soft-fail).
- slint-backend-kindle tested on PW3/PW4/Touch 4; i.MX6 SoloLite (this device) is not listed — monitor for e-ink/input quirks.
- Consider a `scripts/deploy-kindle.sh` wrapper (build + scp + optional log tail).
