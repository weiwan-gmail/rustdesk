# RustDesk Guide

## Project Layout

### Directory Structure
* `src/` Rust app
* `src/server/` audio / clipboard / input / video / network
* `src/platform/` platform-specific code
* `src/ui/` legacy Sciter UI (deprecated)
* `flutter/` current UI
* `libs/hbb_common/` config / proto / shared utils
* `libs/scrap/` screen capture
* `libs/enigo/` input control
* `libs/clipboard/` clipboard
* `libs/hbb_common/src/config.rs` all options

### Key Components
- **Remote Desktop Protocol**: Custom protocol implemented in `src/rendezvous_mediator.rs` for communicating with rustdesk-server
- **Screen Capture**: Platform-specific screen capture in `libs/scrap/`
- **Input Handling**: Cross-platform input simulation in `libs/enigo/`
- **Audio/Video Services**: Real-time audio/video streaming in `src/server/`
- **File Transfer**: Secure file transfer implementation in `libs/hbb_common/`

### UI Architecture
- **Legacy UI**: Sciter-based (deprecated) - files in `src/ui/`
- **Modern UI**: Flutter-based - files in `flutter/`
  - Desktop: `flutter/lib/desktop/`
  - Mobile: `flutter/lib/mobile/`
  - Shared: `flutter/lib/common/` and `flutter/lib/models/`

## Rust Rules

* Avoid `unwrap()` / `expect()` in production code.
* Exceptions:

  * tests;
  * lock acquisition where failure means poisoning, not normal control flow.
* Otherwise prefer `Result` + `?` or explicit handling.
* Do not ignore errors silently.
* Avoid unnecessary `.clone()`.
* Prefer borrowing when practical.
* Do not add dependencies unless needed.
* Keep code simple and idiomatic.

## Tokio Rules

* Assume a Tokio runtime already exists.
* Never create nested runtimes.
* Never call `Runtime::block_on()` inside Tokio / async code.
* Do not hide runtime creation inside helpers or libraries.
* Do not hold locks across `.await`.
* Prefer `.await`, `tokio::spawn`, channels.
* Use `spawn_blocking` or dedicated threads for blocking work.
* Do not use `std::thread::sleep()` in async code.

## Editing Hygiene

* Change only what is required.
* Prefer the smallest valid diff.
* Do not refactor unrelated code.
* Do not make formatting-only changes.
* Keep naming/style consistent with nearby code.

## Localization (`src/lang/*.rs`)

Each file is a `HashMap<key, translation>`. Layout:

* `template.rs` is the master list of every key. **Never edit it** as part of translation work.
* `en.rs` holds only the keys whose English display text differs from the key itself.
* Every other file (`de.rs`, `fr.rs`, …) carries the full key set; an untranslated entry has an empty value: `("key", "")`.

### Finding the English source for a key

When filling an empty entry, determine the source English text with this rule:

* If `key` exists in `en.rs` **with a non-empty value**, that value is the source text (look it up in `en.rs`).
* Otherwise the **key string itself is the source text** (the key is already plain English).

Then translate that source into the file's target language (infer the language from the file's existing non-empty entries / filename).

### Translation hygiene

* Only fill empty values. Never change keys, and never touch existing non-empty translations.
* Preserve placeholders (`{}`) and escape sequences (`\n`, `\"`) exactly as in the source.
* Do not translate brand or technical tokens: `RustDesk`, `Socks5`, `TLS`, `UAC`, `Wayland`, `X11`, `TCP`, `UDP`, `2FA`, `RDP`, `D3D`, etc.
* Copy URL values (e.g. `doc_*` keys) verbatim from `en.rs`.

## Cursor Cloud specific instructions

The VM snapshot already has: system build deps (see `README.md` "How to Build on Linux"), `vcpkg` at `$HOME/vcpkg` with `libvpx/libyuv/opus/aom`, `libsciter-gtk.so` at `$HOME/libsciter-gtk.so`, git submodule `libs/hbb_common` checked out, and multiple Rust toolchains via `rustup`.

### Rust toolchain split (important, non-obvious)
* Default toolchain is `stable` (currently 1.97). Use it for `cargo build`, `cargo test`, and the Flutter `--lib` build.
* Tests require stable — a test-only dependency (`webrtc-util`) uses `is_multiple_of`, which needs Rust ≥ 1.87.
* The deprecated Sciter GUI must be built with Rust ≤ 1.77 (use `1.75.0`). Rust ≥ 1.78 changed the i128 ABI, which breaks the prebuilt `libsciter` and makes the app panic at startup (`invalid value 0x84`). This is documented in `.github/workflows/flutter-build.yml` (`SCITER_RUST_VERSION: "1.75"`).

### Build / lint / test / run
* `VCPKG_ROOT=$HOME/vcpkg` is required for cargo builds (exported in `~/.bashrc`). If a build can't find vpx/opus, export it first.
* Lint/build (project CI gate is build, not clippy): `cargo build --locked`. `cargo clippy` runs but the existing `enigo`/`scrap` code trips default-deny correctness lints (recursive `Display`, raw-ptr deref) — pre-existing, not a setup issue.
* Test (matches `.github/workflows/ci.yml`): `cargo test --locked --workspace --no-fail-fast -- --skip test_get_cursor_pos --skip test_get_key_state`.
* Run the Sciter desktop app: `rustup run 1.75.0 cargo build --locked` then `DISPLAY=:1 ./target/debug/rustdesk`. A real X server is available on `DISPLAY=:1`.
* `libsciter-gtk.so` must sit next to the `rustdesk` binary (`target/debug/` or `target/release/`). `cargo clean` deletes it; re-copy from `$HOME/libsciter-gtk.so`.

### Runtime gotchas
* On launch the app registers with the public rendezvous server `rs-ny.rustdesk.com` over UDP `21116` and reaches a green "Ready" state with a generated ID. A one-time `UUID_MISMATCH` on first run just triggers automatic ID regeneration.
* If `~/.config/rustdesk/RustDesk2.toml` pins `custom-rendezvous-server`/`relay-server` to `127.0.0.1` or sets `allow-websocket = 'Y'`, the app won't reach the public server — clear those options.
* `server not started` / `failed to connect to ipc_service` logs are normal when the elevated background service isn't installed; the main process still handles rendezvous and sessions.

### Flutter UI (current, non-deprecated) — not set up here
Building it additionally needs Flutter 3.24.5 + `flutter_rust_bridge_codegen` + `cargo build --features flutter --lib` then `flutter build linux` (see `.github/workflows/flutter-build.yml` and `build.py`). The Sciter path above shares the same Rust core and is the quickest way to run the app.
