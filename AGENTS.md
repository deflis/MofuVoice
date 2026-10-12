# Repository Guidelines

## Project Structure & Module Organization

This Windows Rust 2021 voice changer uses direct full-duplex ASIO, pitch shifting, and LPC formant preservation. Audio runs at 48 kHz with a default 512-frame buffer. Select an ASIO device explicitly.

- `src/bin/cli.rs` and `src/bin/gui.rs`: independent executable entry points.
- `src/lib.rs`: shared modules and bit-preserving bypass.
- `src/cli.rs` and `src/gui.rs`: argument validation, Win32 GUI FFI, and worker lifecycle.
- `src/asio.rs`: driver configuration, COM initialization, callbacks, metrics, and shutdown.
- `src/pitch.rs` and `src/formant.rs`: audio-independent DSP.
- `native/` and `build.rs`: C++ ASIO latency bridge, GUI, and compilation/linking.
- `tests/dsp_equivalence.rs` and `tests/reference/`: optimized-versus-reference DSP checks.
- `benches/dsp.rs`: paired DSP performance benchmark.
- `scripts/`: session-local toolchain setup and hardware GUI smoke test.
- `README.md`: setup, behavior, and verification guidance. `.tools/`, `target/`, and local measurement `logs/` are ignored; do not commit them.

## Build, Test, and Development Commands

Use Windows x64 Rust MSVC, Visual C++ Build Tools with Windows SDK, libclang, and the ASIO SDK:

```powershell
. .\scripts\dev-env.ps1
cargo check --locked
cargo build --release --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
cargo bench --locked --bench dsp
```

Commands configure the session, check, build, test, lint, format-check, and benchmark. External toolchains require `CPAL_ASIO_DIR` and `LIBCLANG_PATH`; see README.

```powershell
.\target\release\rust-low-latency-voice-changer-gui.exe
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIO device name" --inspect
.\scripts\gui-smoke.ps1 -Device "ASIO device name" -Frames 512
```

Inspection creates no audio buffers; the smoke test starts actual audio through GUI controls.

## Coding Style & Architecture

Use rustfmt, four-space indentation, `snake_case` functions/modules, `PascalCase` types, and uppercase constants. Keep DSP independent of ASIO, preserve exact bits at zero pitch, and silence unused outputs. Reject unsupported settings explicitly.

Preallocate callback state; avoid callback allocation, locks, logging, I/O, and waiting. Document unsafe ABI, buffer, and lifetime boundaries. Keep COM/driver ownership on the audio worker and complete callbacks before disposing buffers.

## Testing Guidelines

Use adjacent unit-test modules and integration tests. Cover formats, channel mapping, pitch transitions, LPC stability, invalid arguments, and block boundaries. DSP optimizations must preserve reference output bits.

Record hardware settings, duration, overruns, sample-position discontinuities, and listening results locally. Distinguish DSP delay, reported driver latency, and measured round-trip latency.

## Commit & Pull Request Guidelines

Follow recent Japanese change descriptions and commit skills: noun-ending messages without type prefixes. PRs should explain behavior, relevant issues, checks, and hardware evidence or untested conditions. Update README when behavior changes; unit tests alone do not establish audio quality.
