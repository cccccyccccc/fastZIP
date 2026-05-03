# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and test commands

```powershell
cargo build                  # debug build
cargo build --release        # release build
cargo run                    # launch GUI (default entrypoint)
cargo run -- list <archive>  # CLI: list archive entries
cargo run -- extract <archive> -o <dir>  # CLI: extract archive
cargo run -- compress <input> -o <output>  # CLI: compress
cargo run -- backends        # CLI: show backend status
cargo run -- formats         # CLI: list supported formats
cargo test                   # run all unit tests
cargo fmt -- --check         # verify formatting
cargo bench                  # run criterion benchmarks
.\run-fastzip.bat            # build release + launch with auto-cleanup
```

The project compiles with Rust edition 2024. The CLI is also available as a separate slim binary: `cargo run --bin fastzip-cli -- <command>`.

## Architecture

This is a Rust archive extraction/compression tool with both a native GUI (egui/eframe) and a CLI (clap). The app runs on Windows primarily but the core extraction logic is cross-platform.

### Crate layout

- **`src/lib.rs`** — modules: `archive`, `gui`, `localization`, `settings`
- **`src/main.rs`** — GUI entrypoint (default binary). Defines `Cli` with clap derive. No-arg runs GUI; subcommands run CLI operations. Maps CLI enums to archive types.
- **`src/bin/fastzip-cli.rs`** — Slim CLI binary without GUI startup overhead. Reuses `archive/mod.rs` via `#[path]` to avoid linking egui/eframe.

### Archive pipeline (`src/archive/`)

- **`mod.rs`** — Core types: `ArchiveFormat` (detection from file extension), `BackendKind`, `ArchiveEntry`, `ExtractOptions`, `ExtractionReport`, `CompressionOptions`, `CompressionReport`, `ZipCompressionMethod`, `CompressionFormat`, `CompressionLevel`, `ExtractPathPlan`. Also contains the actual extraction/compression logic (zip, tar, gz, bz2, xz, 7z, split volumes) behind free functions.
- **`native.rs`** — `NativeBackend` struct. Routes format detection to the correct extraction/list function. Handles split-volume archive joining (concatenates volumes to a temp file before processing). Contains the bulk of unit tests for archive operations.
- **`rar.rs`** — `RarBackend` struct. RAR support via external `unrar.exe`/`rar.exe` process. Discovery checks `FASTZIP_RAR_TOOL` env var, PATH, and standard WinRAR install paths. Extraction stages to a temp directory for per-file cancellation support.
- **`service.rs`** — `ArchiveService` facade. Owns both backends, routes by `ArchiveFormat` (RAR → RarBackend, everything else → NativeBackend). All public API methods delegate here.

### GUI (`src/gui.rs`, ~9400 lines)

Uses `egui` with `eframe` for the native window. `run_native_gui()` is the entry point. Key subsystems:
- Custom window chrome via Windows DWM — rounded corners, custom title bar with drag, resize hit-testing via `WM_NCHITTEST`, min/max/close buttons
- `GuiLaunchRequest` enum: opens the main window or auto-loads an archive
- `ShellCompressionRequest` / `run_shell_compression_progress()`: Windows Explorer context menu integration for "Compress with FastZIP"
- Main UI pages: archive browser, content inspection, extraction/compression tasks, file manager, settings
- Background task system with progress tracking, cancellation, and file conflict resolution dialogs
- Settings page: language switcher, Windows autostart toggle, backend status display

### Localization (`src/localization.rs`)

12 supported locales. Detection order: `FASTZIP_LANG` env → settings.ini → `LC_*`/`LANG` env vars → Windows `GetUserDefaultUILanguage`. Translations stored as TSV files in `locales/` (key[TAB]translation). `localize_message()` takes an English fallback and a Chinese literal; other languages are looked up from the TSV catalog parsed at first use. The catalog includes English as a recognized code (mapped to locale 0) and Chinese/mapped variants under simplified Chinese.

### Settings (`src/settings.rs`)

Language preference persisted to `%LOCALAPPDATA%\FastZIP\settings.ini` as INI format (`[ui]\nlanguage=...`), with a fallback to `%PROGRAMDATA%\FastZIP\settings.ini`. Windows autostart persisted to `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\FastZIP`.

### Tooling (`tools/`)

PowerShell scripts for install/uninstall, shell context menu registration, installer building (Inno Setup via `build-installer.ps1`), and performance comparison (`zip_compare.ps1`, `zip_perf.ps1`). The installer definition is in `installer/FastZIP.iss` with a custom Chinese Simplified translation (`ChineseSimplified.isl`).

## RAR adapter

RAR is intentionally isolated — it never touches the native Rust pipeline. The backend shells out to `unrar.exe`/`rar.exe`. If no tool is found, RAR operations fail gracefully; the GUI backend status panel shows the missing tool. Set `FASTZIP_RAR_TOOL` to override discovery.

## Dependencies

- **Compression**: `zip` (with aes-crypto, bzip2, deflate, zstd, xz, lzma, deflate64 features), `flate2` (zlib-rs backend), `libdeflater`, `bzip2`, `xz2` (multithreaded), `tar`, `sevenz-rust2` (7z read/write)
- **GUI**: `egui` 0.33, `eframe` 0.33, `rfd` (file dialogs), `raw-window-handle`
- **CLI**: `clap` 4.5 with derive
- **Windows**: `windows-sys` 0.61 for DWM, HiDPI, registry, console detection, shell execute
- **Dev**: `criterion` (benchmarks), `tempfile` (test fixtures)
