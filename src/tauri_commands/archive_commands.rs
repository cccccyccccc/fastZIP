use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::archive::BackendStatus;
use crate::archive::service::ArchiveInspection;
use crate::archive::test::TestReport;
use crate::archive::{
    ArchiveEntry, ArchiveService, CompressionOptions, ExtractOptions, OverwriteMode,
};
use crate::encoding::FilenameEncoding;

use super::{cancel_task, register_task, unregister_task};

pub struct AppState {
    pub service: ArchiveService,
}

impl AppState {
    fn new() -> Self {
        Self {
            service: ArchiveService::new(),
        }
    }
}

pub fn init_app_state(app: &mut tauri::App) {
    app.manage(AppState::new());
}

// ── Progress event payloads ─────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskProgressEvent {
    pub task_id: u64,
    pub bytes_processed: u64,
    pub total_bytes: u64,
    #[serde(with = "crate::serde_helpers::serde_duration")]
    pub elapsed: Duration,
    pub speed_mbps: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskCompletedEvent {
    pub task_id: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskFailedEvent {
    pub task_id: u64,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskCanceledEvent {
    pub task_id: u64,
}

// ── Inspection ──────────────────────────────────────────────────────

#[tauri::command]
pub fn inspect_archive(
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<ArchiveInspection, String> {
    state
        .service
        .inspect_archive(&path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_archive(
    state: State<'_, AppState>,
    path: PathBuf,
    password: Option<String>,
) -> Result<Vec<ArchiveEntry>, String> {
    state
        .service
        .list_archive_with_password(&path, password.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn test_archive(
    state: State<'_, AppState>,
    path: PathBuf,
    password: Option<String>,
) -> Result<TestReport, String> {
    state
        .service
        .test_archive_with_password(&path, password.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_backend_statuses(state: State<'_, AppState>) -> Vec<BackendStatus> {
    state.service.backend_statuses()
}

// ── Extract (background task) ───────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractRequest {
    pub path: PathBuf,
    pub output_dir: PathBuf,
    pub overwrite_mode: OverwriteMode,
    pub keep_paths: bool,
    pub password: Option<String>,
    pub filename_encoding: FilenameEncoding,
    pub scan_files: bool,
}

#[tauri::command]
pub fn start_extract(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ExtractRequest,
    task_id: u64,
) {
    let cancel_flag = register_task(task_id);
    let service = state.service.clone();

    thread::spawn(move || {
        let options = ExtractOptions {
            output_dir: request.output_dir,
            overwrite_mode: request.overwrite_mode,
            keep_paths: request.keep_paths,
            password: request.password,
            filename_encoding: request.filename_encoding,
            scan_files: request.scan_files,
        };

        let started = Instant::now();

        let mut last_emit = Instant::now();
        let result = service.extract_archive_with_progress_and_cancel(
            &request.path,
            &options,
            &mut |bytes| {
                if cancel_flag.load(Ordering::SeqCst) {
                    return;
                }
                let now = Instant::now();
                if now.duration_since(last_emit) < Duration::from_millis(120) {
                    return;
                }
                last_emit = now;
                let elapsed = started.elapsed();
                let speed = if elapsed.as_secs_f64() > 0.0 {
                    (bytes as f64 / elapsed.as_secs_f64()) / (1024.0 * 1024.0)
                } else {
                    0.0
                };
                let _ = app.emit(
                    "task-progress",
                    TaskProgressEvent {
                        task_id,
                        bytes_processed: bytes,
                        total_bytes: 0,
                        elapsed,
                        speed_mbps: speed,
                    },
                );
            },
            &mut || cancel_flag.load(Ordering::SeqCst),
        );

        unregister_task(task_id);

        if cancel_flag.load(Ordering::SeqCst) {
            let _ = app.emit("task-canceled", TaskCanceledEvent { task_id });
            return;
        }

        match result {
            Ok(_report) => {
                let _ = app.emit("task-completed", TaskCompletedEvent { task_id });
            }
            Err(e) => {
                let _ = app.emit(
                    "task-failed",
                    TaskFailedEvent {
                        task_id,
                        error: format!("{e:#}"),
                    },
                );
            }
        }
    });
}

// ── Compress (background task) ──────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompressRequest {
    pub sources: Vec<PathBuf>,
    pub output_path: PathBuf,
    pub options: CompressionOptions,
}

#[tauri::command]
pub fn start_compress(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CompressRequest,
    task_id: u64,
) {
    let cancel_flag = register_task(task_id);
    let service = state.service.clone();

    thread::spawn(move || {
        let started = Instant::now();

        let mut last_emit = Instant::now();
        let result = service.compress_with_options_and_exclusions_and_progress_and_cancel(
            &request.sources,
            &[],
            &request.output_path,
            request.options,
            &mut |bytes| {
                if cancel_flag.load(Ordering::SeqCst) {
                    return;
                }
                let now = Instant::now();
                if now.duration_since(last_emit) < Duration::from_millis(120) {
                    return;
                }
                last_emit = now;
                let elapsed = started.elapsed();
                let speed = if elapsed.as_secs_f64() > 0.0 {
                    (bytes as f64 / elapsed.as_secs_f64()) / (1024.0 * 1024.0)
                } else {
                    0.0
                };
                let _ = app.emit(
                    "task-progress",
                    TaskProgressEvent {
                        task_id,
                        bytes_processed: bytes,
                        total_bytes: 0,
                        elapsed,
                        speed_mbps: speed,
                    },
                );
            },
            &mut || cancel_flag.load(Ordering::SeqCst),
        );

        unregister_task(task_id);

        if cancel_flag.load(Ordering::SeqCst) {
            let _ = app.emit("task-canceled", TaskCanceledEvent { task_id });
            return;
        }

        match result {
            Ok(_report) => {
                let _ = app.emit("task-completed", TaskCompletedEvent { task_id });
            }
            Err(e) => {
                let _ = app.emit(
                    "task-failed",
                    TaskFailedEvent {
                        task_id,
                        error: format!("{e:#}"),
                    },
                );
            }
        }
    });
}

// ── Cancel ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn cancel_archive_task(task_id: u64) -> bool {
    cancel_task(task_id)
}
