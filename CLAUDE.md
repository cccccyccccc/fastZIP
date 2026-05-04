# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and test commands

```powershell
cargo build                  # debug build (workspace: lib + CLI + Tauri)
cargo build --release        # release build
cargo test --workspace       # run all unit tests
cargo fmt -- --check         # verify formatting
cargo bench                  # run criterion benchmarks

# Frontend
cd frontend
pnpm install                 # install deps
pnpm build                   # tsc + vite build
pnpm dev                     # Vite dev server at :1420

# Tauri (runs dev server + native window)
cargo tauri dev              # interactive dev mode
cargo tauri build            # release + platform bundle

# CLI (slim binary, no GUI deps)
cargo run --bin fastzip-cli -- list <archive>
cargo run --bin fastzip-cli -- extract <archive> -o <dir>
cargo run --bin fastzip-cli -- compress <input> -o <output>

.\run-fastzip.bat            # build release + launch GUI with auto-cleanup
```

The project compiles with Rust edition 2024. Workspace members: `.` (lib + CLI + SFX stub), `src-tauri` (Tauri GUI binary).

## Architecture

FastZIP is an archive extraction/compression tool with a Tauri v2 GUI (React + TypeScript) and a CLI (clap). The core archive logic is cross-platform; the app targets Windows primarily.

### Crate layout

- **`src/lib.rs`** — modules: `archive`, `tauri_commands`, `localization`, `settings`, `encoding`, `hash`, `update`, `benchmark`, `amsi`, `serde_helpers`
- **`src-tauri/`** — Tauri v2 GUI binary (`[[bin]] name = "fastzip"`). Uses `tauri::command` invoke handler. Plugins: dialog, shell.
- **`src/bin/fastzip-cli.rs`** — Slim CLI binary (clap). Reuses `archive/mod.rs` via `#[path]` to avoid linking Tauri.
- **`src/bin/sfx-stub.rs`** — Self-extracting archive stub.
- **`frontend/`** — React 18 + TypeScript + Vite + Tailwind CSS. Zustand state management.

### Tauri commands (`src/tauri_commands/`)

- **`archive_commands.rs`** — `inspect_archive`, `list_archive`, `test_archive`, `start_extract`, `start_compress`, `cancel_archive_task`, `get_backend_statuses`. Extract/compress run in `thread::spawn` + emit `task-progress`/`task-completed`/`task-failed`/`task-canceled` events.
- **`settings_commands.rs`** — Language, theme, autostart, auto-update, presets CRUD, `get_translations`.
- **`hash_commands.rs`** — `calculate_checksum`, `calculate_all_checksums`.
- **`update_commands.rs`** — `check_for_updates`.
- **`file_manager_commands.rs`** — `list_directory`, `get_file_info`.
- **`benchmark_commands.rs`** — `run_benchmark`.
- **`mod.rs`** — Global `TASK_REGISTRY` (OnceLock<Mutex<HashMap<u64, Arc<AtomicBool>>>>) for cancellation.

### Frontend (`frontend/src/`)

- **Pages**: Extract, Compress, Tasks, FileManager, Benchmark, Settings, Logs
- **Components**: Layout, SideNav, TitleBar (custom chrome, `data-tauri-drag-region`), Toast, UpdateDialog, I18nProvider
- **State**: Zustand stores — `uiStore`, `settingsStore`, `taskStore`
- **Hooks**: `useTauriEvent`, `useI18n`, `useTheme`
- **i18n**: Translations loaded at startup via `get_translations` command, cached in React Context

### Archive pipeline (`src/archive/`)

- **`mod.rs`** — Core types with serde Serialize/Deserialize: `ArchiveFormat`, `BackendKind`, `ArchiveEntry`, `ExtractOptions`, `ExtractionReport`, `CompressionOptions`, `CompressionReport`, etc. Also contains extraction/compression logic (zip, tar, gz, bz2, xz, 7z, split volumes).
- **`native.rs`** — `NativeBackend` struct. Routes format detection, handles split-volume archive joining.
- **`rar.rs`** — `RarBackend`. RAR support via external `unrar.exe`/`rar.exe`. Discovery checks `FASTZIP_RAR_TOOL` env var, PATH, and WinRAR install paths.
- **`service.rs`** — `ArchiveService` facade. Routes by `ArchiveFormat` (RAR → RarBackend, everything else → NativeBackend).

### Localization (`src/localization.rs`)

12 supported locales. Detection order: `FASTZIP_LANG` env → settings.ini → `LC_*`/`LANG` env vars → Windows `GetUserDefaultUILanguage`. Translations stored as TSV files in `locales/`. The `translations_for()` function returns a `HashMap<String, String>` for a given locale code (used by the Tauri `get_translations` command).

### Settings (`src/settings.rs`)

Language/theme persisted to `%LOCALAPPDATA%\FastZIP\settings.ini`. Windows autostart via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\FastZIP`.

### Tooling (`tools/`)

PowerShell scripts for install/uninstall, shell context menu registration, installer building (Inno Setup via `build-installer.ps1`), and performance comparison. The installer definition is in `installer/FastZIP.iss`.

## RAR adapter

RAR is isolated from the native Rust pipeline. The backend shells out to `unrar.exe`/`rar.exe`. If no tool is found, RAR operations fail gracefully. Set `FASTZIP_RAR_TOOL` to override discovery.

## Dependencies

- **Compression**: `zip` (aes-crypto, bzip2, deflate, zstd, xz, lzma, deflate64), `flate2` (zlib-rs), `libdeflater`, `bzip2`, `xz2` (mt), `tar`, `sevenz-rust2` (7z r/w)
- **GUI**: Tauri v2, `tauri-plugin-dialog`, `tauri-plugin-shell`
- **CLI**: `clap` 4.5 with derive
- **Frontend**: React 18, Zustand 5, Tailwind CSS 3, Vite 6
- **Windows**: `windows-sys` 0.61 for AMSI, registry, globalization
- **Dev**: `criterion` (benchmarks), `tempfile` (test fixtures)
