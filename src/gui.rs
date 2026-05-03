use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::path::{Component, Prefix};
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "windows"))]
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
use std::time::{Duration, Instant};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(target_os = "windows")]
use std::{mem::transmute, sync::atomic::AtomicIsize};

use anyhow::{Context as AnyhowContext, Result, anyhow, bail};
use eframe::egui::{
    self, Align, Button, CentralPanel, Color32, Context, Frame, Layout, Margin,
    ProgressBar, RichText, ScrollArea, SidePanel, Stroke, TextEdit, TopBottomPanel, Vec2,
};
#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use rfd::FileDialog;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmDefWindowProc, DwmSetWindowAttribute,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Shell::ShellExecuteW;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GWL_EXSTYLE, GWL_STYLE, GWLP_WNDPROC, GetWindowLongPtrW,
    GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT,
    HTTOP, HTTOPLEFT, HTTOPRIGHT, SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYSIZEFRAME,
    SW_SHOWMINIMIZED, SW_SHOWNORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_ERASEBKGND, WM_NCCALCSIZE,
    WM_NCDESTROY, WM_NCHITTEST, WNDPROC, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_THICKFRAME,
};

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
#[repr(C)]
struct DwmMargins {
    cxLeftWidth: i32,
    cxRightWidth: i32,
    cyTopHeight: i32,
    cyBottomHeight: i32,
}

#[cfg(target_os = "windows")]
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmExtendFrameIntoClientArea(hwnd: HWND, margins: *const DwmMargins) -> i32;
}
use zip::ZipArchive;

use crate::archive::{
    ArchiveEntry, ArchiveInspection, ArchiveService, BackendKind, CompressionFormat,
    CompressionLevel, CompressionOptions, CompressionReport, ExtractOptions, ExtractPathPlan,
    ExtractionReport, OverwriteMode, TestReport, ZipCompressionMethod, file_dialog_extensions,
    parse_volume_size_spec, resolve_output_path, suggested_extract_output_dir,
};
use crate::encoding::FilenameEncoding;
use crate::localization::{
    AppLocale, detect_app_locale, locale_is_chinese, localize_message, set_current_locale,
    supported_locales,
};
use crate::hash::{ChecksumResult, file_checksums};
use crate::settings::{
    load_autostart_enabled, load_preferred_theme, save_autostart_enabled,
    save_preferred_language_value, save_preferred_theme_value,
};

const WINDOW_WIDTH: f32 = 1360.0;
const WINDOW_HEIGHT: f32 = 780.0;
const WINDOW_ASPECT_RATIO: f32 = WINDOW_WIDTH / WINDOW_HEIGHT;
const MIN_WINDOW_SCALE: f32 = 0.68;
const MIN_WINDOW_WIDTH: f32 = WINDOW_WIDTH * MIN_WINDOW_SCALE;
const MIN_WINDOW_HEIGHT: f32 = WINDOW_HEIGHT * MIN_WINDOW_SCALE;
const TOP_BAR_HEIGHT: f32 = 60.0;
const FOOTER_HEIGHT: f32 = 36.0;
const SIDE_NAV_WIDTH: f32 = 220.0;
const MAIN_PADDING: f32 = 24.0;
const RESIZE_EPSILON: f32 = 1.0;
const TOP_BAR_CONTROL_REGION_WIDTH: f32 = 128.0;
const TASK_TYPE_COLUMN_WIDTH: f32 = 124.0;
const TASK_OUTPUT_COLUMN_WIDTH: f32 = 370.0;
const TASK_PROGRESS_COLUMN_WIDTH: f32 = 210.0;
const TASK_SPEED_COLUMN_WIDTH: f32 = 96.0;
const TASK_ETA_COLUMN_WIDTH: f32 = 88.0;
const TASK_ACTIONS_COLUMN_WIDTH: f32 = 144.0;
const CONTENT_NAME_COLUMN_WIDTH: f32 = 360.0;
const CONTENT_SIZE_COLUMN_WIDTH: f32 = 110.0;
const CONTENT_TYPE_COLUMN_WIDTH: f32 = 100.0;
const CONTENT_STATUS_COLUMN_WIDTH: f32 = 280.0;
const CONTENT_ACTION_COLUMN_WIDTH: f32 = 150.0;
const FILE_MANAGER_SELECT_COLUMN_WIDTH: f32 = 34.0;
const FILE_MANAGER_NAME_COLUMN_MIN_WIDTH: f32 = 320.0;
const FILE_MANAGER_SIZE_COLUMN_WIDTH: f32 = 116.0;
const FILE_MANAGER_TYPE_COLUMN_WIDTH: f32 = 96.0;
const FILE_MANAGER_ACTION_COLUMN_WIDTH: f32 = 182.0;
const FILE_MANAGER_GRID_SPACING_X: f32 = 12.0;
const FILE_MANAGER_GRID_SPACING_Y: f32 = 10.0;
const FILE_MANAGER_NAME_COLUMN_PADDING: f32 = 4.0;
const FILE_KIND_SNIFF_BYTES: usize = 4096;
const ZIP_MIMETYPE_READ_LIMIT: usize = 128;
const CONTENT_NAME_HEADER_PADDING: f32 = 4.0;
const CONTENT_SIZE_HEADER_PADDING: f32 = 2.0;
const TASK_RIGHT_SHIFT_PADDING: f32 = 10.0;
const SHELL_WINDOW_WIDTH: f32 = 620.0;
const SHELL_WINDOW_HEIGHT: f32 = 312.0;
const SHELL_WINDOW_MIN_WIDTH: f32 = 620.0;
const SHELL_WINDOW_MIN_HEIGHT: f32 = 312.0;
const SHELL_WINDOW_MAX_WIDTH: f32 = 780.0;
const SHELL_WINDOW_MAX_HEIGHT: f32 = 420.0;
const SHELL_TOP_BAR_HEIGHT: f32 = 60.0;
const SHELL_FOOTER_HEIGHT: f32 = 82.0;

const ANIM_FAST: f32 = 0.15;
const ANIM_NORMAL: f32 = 0.25;

fn anim_bool(ctx: &egui::Context, id: egui::Id, target: bool, duration: f32) -> f32 {
    let value = ctx.animate_bool_with_time_and_easing(
        id,
        target,
        duration,
        egui::emath::easing::cubic_out,
    );
    if value > 0.0 && value < 1.0 {
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    value
}

fn anim_value(ctx: &egui::Context, id: egui::Id, target: f32, duration: f32) -> f32 {
    let value = ctx.animate_value_with_time(id, target, duration);
    if (value - target).abs() > 0.001 {
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    value
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
        (a.a() as f32 + (b.a() as f32 - a.a() as f32) * t) as u8,
    )
}

#[cfg(target_os = "windows")]
static ROOT_HWND_FOR_HIT_TEST: AtomicIsize = AtomicIsize::new(0);
#[cfg(target_os = "windows")]
static ROOT_PREV_WNDPROC: AtomicIsize = AtomicIsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    English,
    ChineseSimplified,
}

impl Language {
    fn detect() -> Self {
        if locale_is_chinese(detect_app_locale()) {
            Self::ChineseSimplified
        } else {
            Self::English
        }
    }

    fn text(self, english: &'static str, chinese: &'static str) -> &'static str {
        let _ = self;
        localize_message(english, chinese)
    }

    fn format(
        self,
        english: &'static str,
        chinese: &'static str,
        replacements: &[(&str, String)],
    ) -> String {
        apply_template(self.text(english, chinese), replacements)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    fn detect() -> Self {
        for key in ["FASTZIP_THEME", "APP_THEME"] {
            if let Ok(value) = env::var(key) {
                let normalized = value.to_ascii_lowercase();
                if normalized.contains("dark") {
                    return Self::Dark;
                }
                if normalized.contains("light") {
                    return Self::Light;
                }
            }
        }
        if let Some(value) = load_preferred_theme() {
            let normalized = value.to_ascii_lowercase();
            if normalized.contains("dark") {
                return Self::Dark;
            }
            if normalized.contains("light") {
                return Self::Light;
            }
        }
        Self::Light
    }

    fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    fn label(self, language: Language) -> &'static str {
        match self {
            Self::Light => language.text("Light", "浅色"),
            Self::Dark => language.text("Dark", "深色"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ThemePalette {
    is_dark: bool,
    background: Color32,
    panel_fill: Color32,
    surface_low: Color32,
    surface_high: Color32,
    surface_highest: Color32,
    surface_variant: Color32,
    panel_stroke: Color32,
    outline_variant: Color32,
    outline: Color32,
    text: Color32,
    text_secondary: Color32,
    text_muted: Color32,
    primary: Color32,
    primary_strong: Color32,
    primary_soft_fill: Color32,
    primary_soft_stroke: Color32,
    on_primary: Color32,
    tertiary: Color32,
    error: Color32,
    danger_fill: Color32,
    danger_stroke: Color32,
    subtle_fill: Color32,
    subtle_stroke: Color32,
    nav_active_fill: Color32,
    nav_active_stroke: Color32,
    nav_active_text: Color32,
    nav_inactive_text: Color32,
    badge_fill: Color32,
    badge_text: Color32,
    log_fill: Color32,
    log_stroke: Color32,
    log_text: Color32,
    log_index: Color32,
    background_glow_primary: Color32,
    background_glow_secondary: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuiOverwriteMode {
    Ask,
    Overwrite,
    Skip,
    Error,
}

impl GuiOverwriteMode {
    fn label(self, language: Language) -> &'static str {
        match self {
            Self::Ask => language.text("Ask before overwrite", "覆盖前询问"),
            Self::Overwrite => language.text("Overwrite existing files", "覆盖已存在文件"),
            Self::Skip => language.text("Skip existing files", "跳过已存在文件"),
            Self::Error => language.text("Fail on conflicts", "遇到冲突时中止"),
        }
    }

    fn to_core(self) -> OverwriteMode {
        match self {
            Self::Ask | Self::Overwrite => OverwriteMode::Overwrite,
            Self::Skip => OverwriteMode::Skip,
            Self::Error => OverwriteMode::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SideNavItem {
    Compress,
    Extract,
    FileManager,
    Tasks,
    Logs,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceMode {
    Compress,
    Extract,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum QueueStatus {
    Progress(f32),
    Waiting,
    Complete,
}

#[derive(Debug, Clone)]
enum QueueAction {
    Back,
    View,
    ViewAndRemove,
}

#[derive(Debug, Clone)]
struct QueueRowData {
    name: String,
    size: String,
    kind: String,
    status: QueueStatus,
    action: QueueAction,
    target: QueueRowTarget,
    secondary_target: Option<QueueRowTarget>,
}

#[derive(Debug, Clone)]
struct FileManagerEntry {
    path: PathBuf,
    name: String,
    size: String,
    kind: String,
    is_dir: bool,
    show_extract_action: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectedFileKind {
    Directory,
    Zip,
    SevenZip,
    Rar,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    Gzip,
    Bzip2,
    Xz,
    Pdf,
    Png,
    Jpeg,
    Gif,
    Webp,
    Bmp,
    Tiff,
    Ico,
    Avif,
    Heic,
    Docx,
    Xlsx,
    Pptx,
    Epub,
    Odt,
    Ods,
    Odp,
    Apk,
    Jar,
    Exe,
    Mp3,
    Wav,
    Flac,
    Ogg,
    Mp4,
    Mov,
    Webm,
    Avi,
    Svg,
    Json,
    Xml,
    Rtf,
    Txt,
    Other,
}

impl DetectedFileKind {
    fn label(self, language: Language) -> &'static str {
        match self {
            Self::Directory => "DIR",
            Self::Zip => "ZIP",
            Self::SevenZip => "7Z",
            Self::Rar => "RAR",
            Self::Tar => "TAR",
            Self::TarGz => "TAR.GZ",
            Self::TarBz2 => "TAR.BZ2",
            Self::TarXz => "TAR.XZ",
            Self::Gzip => "GZ",
            Self::Bzip2 => "BZ2",
            Self::Xz => "XZ",
            Self::Pdf => "PDF",
            Self::Png => "PNG",
            Self::Jpeg => "JPG",
            Self::Gif => "GIF",
            Self::Webp => "WEBP",
            Self::Bmp => "BMP",
            Self::Tiff => "TIFF",
            Self::Ico => "ICO",
            Self::Avif => "AVIF",
            Self::Heic => "HEIC",
            Self::Docx => "DOCX",
            Self::Xlsx => "XLSX",
            Self::Pptx => "PPTX",
            Self::Epub => "EPUB",
            Self::Odt => "ODT",
            Self::Ods => "ODS",
            Self::Odp => "ODP",
            Self::Apk => "APK",
            Self::Jar => "JAR",
            Self::Exe => "EXE",
            Self::Mp3 => "MP3",
            Self::Wav => "WAV",
            Self::Flac => "FLAC",
            Self::Ogg => "OGG",
            Self::Mp4 => "MP4",
            Self::Mov => "MOV",
            Self::Webm => "WEBM",
            Self::Avi => "AVI",
            Self::Svg => "SVG",
            Self::Json => "JSON",
            Self::Xml => "XML",
            Self::Rtf => "RTF",
            Self::Txt => "TXT",
            Self::Other => language.text("Other", "其他"),
        }
    }

    fn show_extract_action(self) -> bool {
        matches!(self, Self::Zip)
    }
}

#[derive(Debug, Clone)]
enum QueueRowTarget {
    ExtractUp,
    ExtractEntry { path: PathBuf, is_dir: bool },
    CompressUp,
    CompressPath { path: PathBuf, is_dir: bool },
    RemoveCompressSource { path: PathBuf },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingDialogAction {
    BrowseArchive,
    BrowseCompressFiles,
    BrowseCompressFolders,
    BrowseCompressOutputPath,
    BrowseOutputDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtractConflictAction {
    Overwrite,
    Skip,
    Rename,
}

#[derive(Debug, Clone)]
struct ExtractConflictItem {
    relative_path: PathBuf,
    destination: PathBuf,
    existing_is_dir: bool,
}

#[derive(Debug, Clone)]
struct PendingExtractTask {
    archive_path: PathBuf,
    options: ExtractOptions,
    plan: ExtractPathPlan,
    total_bytes: u64,
}

#[derive(Debug, Clone)]
struct ExtractConflictDialogState {
    task: PendingExtractTask,
    conflicts: Vec<ExtractConflictItem>,
    current_index: usize,
    apply_to_all: bool,
    rename_value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskKind {
    Compress,
    Extract,
}

impl TaskKind {
    fn label(self, language: Language) -> &'static str {
        match self {
            Self::Compress => language.text("Compress", "压缩"),
            Self::Extract => language.text("Extract", "解压"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Queued,
    Running,
    Canceling,
    Completed,
    Canceled,
    Failed,
}

impl TaskState {
    fn label(self, language: Language) -> &'static str {
        match self {
            Self::Queued => language.text("Queued", "排队中"),
            Self::Running => language.text("Running", "进行中"),
            Self::Canceling => language.text("Canceling", "正在取消"),
            Self::Completed => language.text("Completed", "已完成"),
            Self::Canceled => language.text("Canceled", "已取消"),
            Self::Failed => language.text("Failed", "失败"),
        }
    }
}

#[derive(Debug, Clone)]
enum TaskSpec {
    Compress {
        sources: Vec<PathBuf>,
        excluded_paths: Vec<PathBuf>,
        output_path: PathBuf,
        options: CompressionOptions,
    },
    Extract {
        archive_path: PathBuf,
        options: ExtractOptions,
        plan: ExtractPathPlan,
    },
}

impl TaskSpec {
    fn output_path(&self) -> &Path {
        match self {
            Self::Compress { output_path, .. } => output_path,
            Self::Extract { options, .. } => &options.output_dir,
        }
    }
}

#[derive(Debug, Clone)]
struct TaskQueueItem {
    id: u64,
    kind: TaskKind,
    spec: TaskSpec,
    total_bytes: u64,
    processed_bytes: u64,
    current_bytes_per_second: Option<f64>,
    state: TaskState,
    started_at: Option<Instant>,
    finished_at: Option<Instant>,
    error_message: Option<String>,
    cancel_flag: Arc<AtomicBool>,
}

struct TaskQueueMetrics {
    progress: f32,
    progress_text: String,
    speed_text: String,
    eta_text: String,
    status_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeedbackTone {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
struct ToastMessage {
    text: String,
    tone: FeedbackTone,
    created_at: Instant,
    duration: Duration,
}

enum ScanJobResult {
    Scanned {
        inspection: ArchiveInspection,
        entries: Vec<ArchiveEntry>,
    },
    Failed(String),
}

enum TaskJobResult {
    Progress {
        task_id: u64,
        processed_bytes: u64,
        bytes_per_second: f64,
    },
    Compressed {
        task_id: u64,
        report: CompressionReport,
    },
    Extracted {
        task_id: u64,
        report: ExtractionReport,
    },
    Canceled {
        task_id: u64,
    },
    Failed {
        task_id: u64,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub enum GuiLaunchRequest {
    OpenArchive(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ShellCompressionRequest {
    pub sources: Vec<PathBuf>,
    pub output_path: PathBuf,
    pub options: CompressionOptions,
}

enum TaskQueueRowEvent {
    ShowOutputPath(String),
    Prioritize(u64),
    Cancel(u64),
    Rerun(u64),
    Delete(u64),
    OpenOutputDir(u64),
}

pub fn run_native_gui(launch_request: Option<GuiLaunchRequest>) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT])
            .with_max_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_icon(application_icon())
            .with_decorations(false)
            .with_taskbar(true)
            .with_resizable(true)
            .with_maximize_button(false)
            .with_fullscreen(false),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "FastZIP",
        options,
        Box::new(move |cc| Ok(Box::new(FastZipGui::new(cc, launch_request.clone())))),
    )
    .map_err(|error| anyhow!("Failed to launch GUI: {error}"))
}

pub fn create_shell_compression_request(
    sources: Vec<PathBuf>,
    format: CompressionFormat,
    output_path: Option<PathBuf>,
) -> Result<ShellCompressionRequest> {
    let mut normalized_sources = Vec::new();
    for source in sources {
        let rendered = source.to_string_lossy();
        if matches!(
            rendered.as_ref(),
            "%1" | "%L"
                | "%V"
                | "%*"
                | "%0"
                | "%2"
                | "%3"
                | "%4"
                | "%5"
                | "%6"
                | "%7"
                | "%8"
                | "%9"
        ) {
            continue;
        }

        if normalized_sources
            .iter()
            .any(|existing: &PathBuf| existing == &source)
        {
            continue;
        }
        normalized_sources.push(source);
    }

    if normalized_sources.is_empty() {
        bail!(
            "{}",
            localize_message(
                "Pick files or folders to compress first.",
                "请先选择待压缩的文件或文件夹。",
            )
        );
    }
    for source in &normalized_sources {
        if !source.exists() {
            bail!(
                "{}",
                localize_format(
                    "Compression source not found: {source_path}",
                    "未找到待压缩源：{source_path}",
                    &[("{source_path}", source.display().to_string())],
                )
            );
        }
    }

    let output_path = output_path
        .or_else(|| suggested_archive_output_path(&normalized_sources, format))
        .ok_or_else(|| {
            anyhow!(localize_message(
                "Failed to determine an output path for the selected sources",
                "无法确定所选源的输出路径",
            ))
        })?;
    let mut options = CompressionOptions {
        format,
        ..CompressionOptions::default()
    };
    options.thread_count = if format.supports_thread_count() {
        default_compression_thread_count()
    } else {
        1
    };

    Ok(ShellCompressionRequest {
        sources: normalized_sources,
        output_path,
        options,
    })
}

pub fn run_shell_compression_progress(request: ShellCompressionRequest) -> Result<()> {
    let window_title = localize_message("FastZIP Background Compression", "FastZIP 后台压缩");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([SHELL_WINDOW_WIDTH, SHELL_WINDOW_HEIGHT])
            .with_min_inner_size([SHELL_WINDOW_MIN_WIDTH, SHELL_WINDOW_MIN_HEIGHT])
            .with_max_inner_size([SHELL_WINDOW_MAX_WIDTH, SHELL_WINDOW_MAX_HEIGHT])
            .with_icon(application_icon())
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(true),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        window_title,
        options,
        Box::new(move |cc| {
            Ok(Box::new(ShellCompressionProgressApp::new(
                cc,
                request.clone(),
            )))
        }),
    )
    .map_err(|error| anyhow!("Failed to launch compression progress window: {error}"))
}

struct FastZipGui {
    service: ArchiveService,
    language: Language,
    locale: &'static AppLocale,
    theme_mode: ThemeMode,
    autostart_enabled: bool,
    file_manager_current_dir: PathBuf,
    file_manager_path_input: String,
    file_manager_selected_paths: BTreeSet<PathBuf>,
    show_selected_file_manager_paths: bool,
    show_checksum_dialog: bool,
    checksum_results: Vec<ChecksumResult>,
    show_test_result_dialog: bool,
    test_report: Option<TestReport>,
    show_save_preset_dialog: bool,
    show_manage_presets_dialog: bool,
    preset_save_name: String,
    side_nav: SideNavItem,
    workspace_mode: WorkspaceMode,
    search_query: String,
    archive_path: String,
    output_dir: String,
    compress_sources: Vec<PathBuf>,
    compress_excluded_paths: Vec<PathBuf>,
    compression_options: CompressionOptions,
    compression_password_confirm: String,
    compression_split_volume_size_input: String,
    compress_output_path: String,
    extract_browser_path: PathBuf,
    compress_browser_path: Option<PathBuf>,
    extract_password: String,
    keep_paths: bool,
    overwrite_mode: GuiOverwriteMode,
    filename_encoding: FilenameEncoding,
    scan_files: bool,
    inspection: Option<ArchiveInspection>,
    entries: Vec<ArchiveEntry>,
    extract_conflict_dialog: Option<ExtractConflictDialogState>,
    preview_archive_path: Option<PathBuf>,
    preview_output_dir: Option<PathBuf>,
    task_queue: Vec<TaskQueueItem>,
    next_task_id: u64,
    expanded_task_output_path: Option<String>,
    pending_dialog_action: Option<PendingDialogAction>,
    show_compress_source_picker: bool,
    dialog_click_guard: bool,
    toast: Option<ToastMessage>,
    logs: Vec<String>,
    scan_receiver: Option<Receiver<ScanJobResult>>,
    task_receiver: Option<Receiver<TaskJobResult>>,
    benchmark_receiver: Option<Receiver<Vec<String>>>,
    requested_viewport_size: Option<Vec2>,
    pending_launch_request: Option<GuiLaunchRequest>,
    #[cfg(target_os = "windows")]
    root_hwnd: Option<HWND>,
}

impl FastZipGui {
    fn new(cc: &eframe::CreationContext<'_>, launch_request: Option<GuiLaunchRequest>) -> Self {
        #[cfg(target_os = "windows")]
        let root_hwnd = configure_native_window(cc);
        #[cfg(not(target_os = "windows"))]
        configure_native_window(cc);
        install_multilingual_fonts(&cc.egui_ctx);
        let service = ArchiveService::new();
        let locale = detect_app_locale();
        set_current_locale(locale);
        let language = if locale_is_chinese(locale) {
            Language::ChineseSimplified
        } else {
            Language::English
        };
        let theme_mode = ThemeMode::detect();
        let autostart_enabled = load_autostart_enabled().unwrap_or(false);
        let file_manager_current_dir = default_file_manager_dir();
        let file_manager_path_input = file_manager_current_dir.display().to_string();
        let mut compression_options = CompressionOptions::default();
        compression_options.thread_count = default_compression_thread_count();
        configure_theme(&cc.egui_ctx, theme_mode);

        Self {
            service,
            language,
            locale,
            theme_mode,
            autostart_enabled,
            file_manager_current_dir,
            file_manager_path_input,
            file_manager_selected_paths: BTreeSet::new(),
            show_selected_file_manager_paths: false,
            show_checksum_dialog: false,
            checksum_results: Vec::new(),
            show_test_result_dialog: false,
            test_report: None,
            show_save_preset_dialog: false,
            show_manage_presets_dialog: false,
            preset_save_name: String::new(),
            side_nav: SideNavItem::Compress,
            workspace_mode: WorkspaceMode::Compress,
            search_query: String::new(),
            archive_path: String::new(),
            output_dir: String::new(),
            compress_sources: Vec::new(),
            compress_excluded_paths: Vec::new(),
            compression_options,
            compression_password_confirm: String::new(),
            compression_split_volume_size_input: String::new(),
            compress_output_path: String::new(),
            extract_browser_path: PathBuf::new(),
            compress_browser_path: None,
            extract_password: String::new(),
            keep_paths: true,
            overwrite_mode: GuiOverwriteMode::Ask,
            filename_encoding: FilenameEncoding::Utf8,
            scan_files: false,
            inspection: None,
            entries: Vec::new(),
            extract_conflict_dialog: None,
            preview_archive_path: None,
            preview_output_dir: None,
            task_queue: Vec::new(),
            next_task_id: 1,
            expanded_task_output_path: None,
            pending_dialog_action: None,
            show_compress_source_picker: false,
            dialog_click_guard: false,
            toast: None,
            logs: vec![initial_ready_log(language), initial_backend_log(language)],
            scan_receiver: None,
            task_receiver: None,
            benchmark_receiver: None,
            requested_viewport_size: None,
            pending_launch_request: launch_request,
            #[cfg(target_os = "windows")]
            root_hwnd,
        }
    }

    fn is_scanning_archive(&self) -> bool {
        self.scan_receiver.is_some()
    }

    fn has_active_task(&self) -> bool {
        self.task_receiver.is_some()
    }

    fn t(&self, english: &'static str, chinese: &'static str) -> &'static str {
        self.language.text(english, chinese)
    }

    fn palette(&self) -> ThemePalette {
        theme_palette(self.theme_mode)
    }

    fn switch_locale(&mut self, locale: &'static AppLocale) {
        if self.locale == locale {
            return;
        }

        self.locale = locale;
        set_current_locale(locale);
        self.language = if locale_is_chinese(locale) {
            Language::ChineseSimplified
        } else {
            Language::English
        };
        self.push_log(format!(
            "{}: {}",
            self.t("Interface Language", "界面语言"),
            locale.display_name
        ));
        if let Err(error) = save_preferred_language_value(locale.code) {
            self.push_log(format!(
                "{}: {error:#}",
                self.t("Failed to save language preference", "保存语言偏好失败")
            ));
        }
    }

    fn switch_theme(&mut self, theme_mode: ThemeMode) {
        if self.theme_mode == theme_mode {
            return;
        }

        self.theme_mode = theme_mode;
        let theme_str = match theme_mode {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        };
        if let Err(error) = save_preferred_theme_value(theme_str) {
            self.push_log(format!(
                "{}: {error:#}",
                self.t("Failed to save theme preference", "保存主题偏好失败")
            ));
        }
        self.push_log(
            match theme_mode {
                ThemeMode::Light => self
                    .language
                    .text("Theme switched to Light mode.", "已切换到浅色模式。"),
                ThemeMode::Dark => self
                    .language
                    .text("Theme switched to Dark mode.", "已切换到深色模式。"),
            }
            .to_string(),
        );
    }

    fn switch_autostart(&mut self, enabled: bool) {
        if self.autostart_enabled == enabled {
            return;
        }

        match env::current_exe()
            .context("Failed to resolve the current FastZIP executable path")
            .and_then(|path| save_autostart_enabled(&path, enabled))
        {
            Ok(()) => {
                self.autostart_enabled = enabled;
                let message = if enabled {
                    self.t("Autostart enabled.", "已启用开机自启动。")
                } else {
                    self.t("Autostart disabled.", "已关闭开机自启动。")
                };
                self.push_log(message.to_string());
                self.show_toast(FeedbackTone::Success, message);
            }
            Err(error) => {
                // If the error is about HKLM access denied, offer UAC elevation to fix
                if !enabled && format!("{error:#}").contains("HKLM") {
                    self.try_elevated_delete_autostart();
                    return;
                }
                let message = self.language.format(
                    "Failed to update autostart: {error}",
                    "更新开机自启动失败：{error}",
                    &[("{error}", format!("{error:#}"))],
                );
                self.push_log(message.clone());
                self.show_toast(FeedbackTone::Error, message);
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn try_elevated_delete_autostart(&mut self) {
        let exe_path = match env::current_exe() {
            Ok(path) => path,
            Err(_) => return,
        };

        let wide_path: Vec<u16> = exe_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let verb: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
        let args: Vec<u16> = OsStr::new("internal-delete-autostart")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                wide_path.as_ptr(),
                args.as_ptr(),
                std::ptr::null(),
                windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE,
            )
        };

        if (result as usize) > 32 {
            // UAC elevation succeeded, re-check autostart state
            self.autostart_enabled = load_autostart_enabled().unwrap_or(false);
            let message = self.t(
                "Autostart disabled successfully.",
                "已成功关闭开机自启动。",
            );
            self.push_log(message.to_string());
            self.show_toast(FeedbackTone::Success, message);
        } else {
            // User declined or elevation failed
            let message = self.t(
                "Autostart was installed for all users. \
                 Run FastZIP as administrator once to turn off autostart, \
                 or delete the 'FastZIP' value manually from:\n\
                 HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "自启动是为所有用户安装的，需要管理员权限才能关闭。\n\n\
                 请以管理员身份运行 FastZIP 来关闭自启动，\n\
                 或手动删除注册表中的 'FastZIP' 值：\n\
                 HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            );
            self.push_log(message.to_string());
            self.show_toast(FeedbackTone::Error, message);
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn try_elevated_delete_autostart(&mut self) {}

    #[cfg(target_os = "windows")]
    fn try_open_default_apps_dialog(&mut self) {
        let settings_uri: Vec<u16> = OsStr::new("ms-settings:defaultapps")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let verb: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();

        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                settings_uri.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW,
            )
        };

        if (result as usize) > 32 {
            let message = self.t(
                "Find FastZIP in the list and set it as the default for archive formats.",
                "在列表中找到 FastZIP，将其设为压缩文件的默认应用。",
            );
            self.push_log(message.to_string());
            self.show_toast(FeedbackTone::Info, message);
        } else {
            let message = self.t(
                "Failed to open Windows Default Apps settings.",
                "无法打开 Windows 默认应用设置。",
            );
            self.push_log(message.to_string());
            self.show_toast(FeedbackTone::Error, message);
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn try_open_default_apps_dialog(&mut self) {}

    fn start_benchmark(&mut self) {
        let msg = self.t(
            "Benchmark started. Results will appear in the log when complete.",
            "基准测试已开始。完成后结果将显示在日志中。",
        );
        self.push_log(msg.to_string());
        self.show_toast(FeedbackTone::Info, msg);
        let (tx, rx) = mpsc::channel();
        self.benchmark_receiver = Some(rx);
        std::thread::spawn(move || {
            let out_dir = std::path::PathBuf::from("benchmark_results");
            let lines = match crate::benchmark::run_benchmark(&out_dir) {
                Ok(entries) => {
                    let mut lines = vec![format!(
                        "Benchmark complete. {} results saved to {}",
                        entries.len(),
                        out_dir.display()
                    )];
                    for entry in &entries {
                        lines.push(format!(
                            "{:?} {:?} ratio={:.4} throughput={:.2} MB/s",
                            entry.format,
                            entry.level,
                            entry.compression_ratio(),
                            entry.throughput_mbps(),
                        ));
                    }
                    lines
                }
                Err(e) => vec![format!("Benchmark failed: {e:#}")],
            };
            let _ = tx.send(lines);
        });
    }

    fn activate_workspace(&mut self, workspace_mode: WorkspaceMode) {
        self.workspace_mode = workspace_mode;
        self.side_nav = self.current_workspace_nav_item();
    }

    fn current_workspace_nav_item(&self) -> SideNavItem {
        match self.workspace_mode {
            WorkspaceMode::Compress => SideNavItem::Compress,
            WorkspaceMode::Extract => SideNavItem::Extract,
        }
    }

    fn set_side_nav(&mut self, item: SideNavItem) {
        self.side_nav = item;
        match item {
            SideNavItem::Compress => self.workspace_mode = WorkspaceMode::Compress,
            SideNavItem::Extract => self.workspace_mode = WorkspaceMode::Extract,
            SideNavItem::FileManager
            | SideNavItem::Tasks
            | SideNavItem::Logs
            | SideNavItem::Settings => {}
        }
    }

    fn reset_extract_browser(&mut self) {
        self.extract_browser_path.clear();
        self.preview_archive_path = None;
        self.preview_output_dir = None;
    }

    fn reset_compress_browser(&mut self) {
        self.compress_browser_path = None;
    }

    fn set_file_manager_directory(&mut self, path: PathBuf) {
        let normalized = normalize_windows_user_path(&path);
        self.file_manager_current_dir = normalized.clone();
        self.file_manager_path_input = normalized.display().to_string();
    }

    fn open_file_manager_directory(&mut self, path: PathBuf) -> Result<()> {
        let resolved = resolve_file_manager_directory(&path)?;
        self.set_side_nav(SideNavItem::FileManager);
        self.set_file_manager_directory(resolved);
        Ok(())
    }

    fn open_file_manager_typed_path(&mut self) {
        let typed_path = PathBuf::from(self.file_manager_path_input.trim());
        if typed_path.as_os_str().is_empty() {
            self.file_manager_path_input = self.file_manager_current_dir.display().to_string();
            return;
        }

        if let Err(error) = self.open_file_manager_directory(typed_path.clone()) {
            self.push_log(self.language.format(
                "Failed to open path: {error}",
                "打开路径失败：{error}",
                &[("{error}", format!("{error:#}"))],
            ));
            self.show_toast(FeedbackTone::Error, error.to_string());
        }
    }

    fn browse_file_manager_folder(&mut self) {
        self.set_side_nav(SideNavItem::FileManager);

        let dialog = FileDialog::new().set_directory(&self.file_manager_current_dir);
        let Some(path) = dialog.pick_folder() else {
            self.show_toast(
                FeedbackTone::Info,
                self.t("Folder selection was canceled.", "已取消文件夹选择。"),
            );
            return;
        };

        if let Err(error) = self.open_file_manager_directory(path.clone()) {
            self.push_log(self.language.format(
                "Failed to open path: {error}",
                "打开路径失败：{error}",
                &[("{error}", format!("{error:#}"))],
            ));
            self.show_toast(FeedbackTone::Error, format!("{error:#}"));
        }
    }

    fn open_file_manager_parent_directory(&mut self) {
        if let Some(parent) = self
            .file_manager_current_dir
            .parent()
            .map(Path::to_path_buf)
        {
            if let Err(error) = self.open_file_manager_directory(parent) {
                self.push_log(self.language.format(
                    "Failed to open path: {error}",
                    "打开路径失败：{error}",
                    &[("{error}", format!("{error:#}"))],
                ));
                self.show_toast(FeedbackTone::Error, format!("{error:#}"));
            }
        }
    }

    fn view_file_manager_entry(&mut self, path: &Path, is_dir: bool) {
        let result = if is_dir {
            self.open_file_manager_directory(path.to_path_buf())
        } else {
            open_path_with_system_default(path)
        };

        if let Err(error) = result {
            self.push_log(self.language.format(
                "Failed to open path: {error}",
                "打开路径失败：{error}",
                &[("{error}", format!("{error:#}"))],
            ));
            self.show_toast(FeedbackTone::Error, format!("{error:#}"));
        }
    }

    fn extract_file_manager_zip(&mut self, path: &Path) {
        if let Err(error) = self.apply_archive_selection(path.to_path_buf()) {
            self.push_log(self.language.format(
                "Archive selection failed: {error}",
                "选择压缩包失败：{error}",
                &[("{error}", format!("{error:#}"))],
            ));
            self.show_toast(FeedbackTone::Error, format!("{error:#}"));
        }
    }

    fn compress_selected_file_manager_paths(&mut self) {
        self.file_manager_selected_paths
            .retain(|path| path.exists());
        let selected_paths =
            normalize_file_manager_compression_paths(&self.file_manager_selected_paths);
        if selected_paths.is_empty() {
            self.show_toast(
                FeedbackTone::Info,
                self.t(
                    "No files or folders are selected in the file manager.",
                    "文件管理器中还没有选中任何文件或文件夹。",
                ),
            );
            return;
        }

        self.activate_workspace(WorkspaceMode::Compress);
        self.apply_compress_sources(selected_paths);
    }

    fn schedule_dialog_action(&mut self, ctx: &Context, action: PendingDialogAction) {
        if self.dialog_click_guard {
            return;
        }

        self.pending_dialog_action = Some(action);
        self.dialog_click_guard = true;
        ctx.request_repaint();
    }

    fn refresh_dialog_click_guard(&mut self, ctx: &Context) {
        if ctx.input(|input| input.pointer.any_down()) {
            self.dialog_click_guard = false;
        }
    }

    fn process_launch_request(&mut self) {
        let Some(request) = self.pending_launch_request.take() else {
            return;
        };

        match request {
            GuiLaunchRequest::OpenArchive(path) => {
                if let Err(error) = self.apply_archive_selection(path.clone()) {
                    self.push_log(self.language.format(
                        "Failed to open {path}: {error}",
                        "打开 {path} 失败：{error}",
                        &[
                            ("{path}", path.display().to_string()),
                            ("{error}", format!("{error:#}")),
                        ],
                    ));
                    self.show_toast(
                        FeedbackTone::Error,
                        self.t("The archive could not be opened.", "无法打开该压缩包。"),
                    );
                }
            }
        }
    }

    fn process_pending_dialog_action(&mut self) {
        let Some(action) = self.pending_dialog_action.take() else {
            return;
        };

        match action {
            PendingDialogAction::BrowseArchive => self.browse_archive(),
            PendingDialogAction::BrowseCompressFiles => self.browse_compress_files(),
            PendingDialogAction::BrowseCompressFolders => self.browse_compress_folders(),
            PendingDialogAction::BrowseCompressOutputPath => self.browse_compress_output_path(),
            PendingDialogAction::BrowseOutputDir => self.browse_output_dir(),
        }
    }

    fn enforce_viewport_aspect_ratio(&mut self, ctx: &Context) {
        let viewport = ctx.input(|input| input.viewport().clone());
        if viewport.minimized == Some(true) {
            return;
        }

        let Some(inner_rect) = viewport.inner_rect else {
            return;
        };

        let current_size = inner_rect.size();
        if current_size.x <= 1.0 || current_size.y <= 1.0 {
            return;
        }

        let desired_size = fitted_window_size(current_size);
        if approx_size_eq(current_size, desired_size) {
            self.requested_viewport_size = None;
            return;
        }

        if self
            .requested_viewport_size
            .is_some_and(|last| approx_size_eq(last, desired_size))
        {
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(desired_size));
        self.requested_viewport_size = Some(desired_size);
    }

    fn browse_archive(&mut self) {
        self.activate_workspace(WorkspaceMode::Extract);

        if let Some(path) = FileDialog::new()
            .add_filter(self.t("Archives", "压缩包"), file_dialog_extensions())
            .pick_file()
        {
            if let Err(error) = self.apply_archive_selection(path) {
                self.push_log(self.language.format(
                    "Archive selection failed: {error}",
                    "选择压缩包失败：{error}",
                    &[("{error}", format!("{error:#}"))],
                ));
                self.show_toast(
                    FeedbackTone::Error,
                    self.t(
                        "Invalid selection. Please choose an archive file again.",
                        "选择格式有误，请重新选择压缩包文件。",
                    ),
                );
            }
        } else {
            self.show_toast(
                FeedbackTone::Info,
                self.t("Archive selection was canceled.", "已取消压缩包选择。"),
            );
        }
    }

    fn apply_archive_selection(&mut self, path: PathBuf) -> Result<()> {
        self.activate_workspace(WorkspaceMode::Extract);

        let inspection = self.service.inspect_archive(&path)?;
        let archive_path = path.display().to_string();
        self.archive_path = archive_path.clone();
        self.reset_extract_browser();
        self.output_dir = inspection.suggested_output_dir.display().to_string();
        self.inspection = Some(inspection);
        self.entries.clear();
        self.push_log(self.language.format(
            "Selected archive: {archive_path}",
            "已选择压缩包：{archive_path}",
            &[("{archive_path}", archive_path.clone())],
        ));
        self.show_toast(
            FeedbackTone::Success,
            self.t("Archive selected.", "已选择压缩包。"),
        );
        self.scan_archive();
        Ok(())
    }

    fn browse_compress_files(&mut self) {
        self.activate_workspace(WorkspaceMode::Compress);

        let Some(sources) = FileDialog::new().pick_files() else {
            self.show_toast(
                FeedbackTone::Info,
                self.t("File selection was canceled.", "已取消文件选择。"),
            );
            return;
        };

        self.apply_compress_sources(sources);
    }

    fn browse_compress_folders(&mut self) {
        self.activate_workspace(WorkspaceMode::Compress);

        let Some(sources) = FileDialog::new().pick_folders() else {
            self.show_toast(
                FeedbackTone::Info,
                self.t("Folder selection was canceled.", "已取消文件夹选择。"),
            );
            return;
        };

        self.apply_compress_sources(sources);
    }

    fn apply_compress_sources(&mut self, sources: Vec<PathBuf>) {
        self.compress_sources = sources;
        self.compress_excluded_paths.clear();
        self.reset_compress_browser();
        if let Some(suggested_output) =
            suggested_archive_output_path(&self.compress_sources, self.compression_options.format)
        {
            self.compress_output_path = suggested_output.display().to_string();
        }

        self.inspection = None;
        self.entries.clear();
        self.push_log(self.language.format(
            "Selected {count} compression source(s).",
            "已选择 {count} 个待压缩源。",
            &[("{count}", self.compress_sources.len().to_string())],
        ));
        self.show_toast(
            FeedbackTone::Success,
            self.language.format(
                "{count} source(s) ready for compression.",
                "已准备 {count} 个待压缩源。",
                &[("{count}", self.compress_sources.len().to_string())],
            ),
        );
    }

    fn append_compress_sources(&mut self, mut dropped_sources: Vec<PathBuf>) {
        let previous_sources = self.compress_sources.clone();
        let previous_suggested_output =
            suggested_archive_output_path(&previous_sources, self.compression_options.format)
                .map(|path| path.display().to_string());
        let current_output = self.compress_output_path.trim().to_string();

        dropped_sources.sort();
        dropped_sources.dedup();

        for path in dropped_sources {
            if let Some(existing_index) =
                self.compress_sources.iter().position(|item| item == &path)
            {
                self.compress_sources[existing_index] = path;
            } else {
                self.compress_sources.push(path);
            }
        }

        self.retain_valid_compress_exclusions();
        self.reset_compress_browser();
        if self.compress_output_path.trim().is_empty()
            || previous_suggested_output
                .as_ref()
                .is_some_and(|value| value == &current_output)
        {
            if let Some(suggested_output) = suggested_archive_output_path(
                &self.compress_sources,
                self.compression_options.format,
            ) {
                self.compress_output_path = suggested_output.display().to_string();
            }
        }

        self.inspection = None;
        self.entries.clear();
        self.push_log(self.language.format(
            "Added dropped sources. {count} source(s) ready for compression.",
            "已添加拖入内容，当前共有 {count} 个待压缩源。",
            &[("{count}", self.compress_sources.len().to_string())],
        ));
        self.show_toast(
            FeedbackTone::Success,
            self.language.format(
                "{count} source(s) ready for compression.",
                "当前共有 {count} 个待压缩源。",
                &[("{count}", self.compress_sources.len().to_string())],
            ),
        );
    }

    fn retain_valid_compress_exclusions(&mut self) {
        let sources = self.compress_sources.clone();
        self.compress_excluded_paths.retain(|path| {
            sources.iter().any(|source| path.starts_with(source))
                && !sources
                    .iter()
                    .any(|source| source == path || source.starts_with(path))
        });
        self.compress_excluded_paths.sort();
        self.compress_excluded_paths.dedup();
    }

    fn exclude_compress_path(&mut self, path: &Path) {
        if self.is_path_excluded_from_compress(path) {
            return;
        }

        self.compress_excluded_paths
            .retain(|excluded| !excluded.starts_with(path));
        self.compress_excluded_paths.push(path.to_path_buf());
        self.retain_valid_compress_exclusions();

        if self
            .compress_browser_path
            .as_ref()
            .is_some_and(|current_dir| self.is_path_excluded_from_compress(current_dir))
        {
            self.reset_compress_browser();
        }

        self.push_log(self.language.format(
            "Excluded from compression: {path}. {count} item(s) excluded.",
            "已从压缩内容中移除：{path}。当前共排除 {count} 项。",
            &[
                ("{path}", path.display().to_string()),
                ("{count}", self.compress_excluded_paths.len().to_string()),
            ],
        ));
        self.show_toast(
            FeedbackTone::Info,
            self.language.format(
                "{count} item(s) excluded from compression.",
                "当前共排除 {count} 个压缩项。",
                &[("{count}", self.compress_excluded_paths.len().to_string())],
            ),
        );
    }

    fn remove_compress_source(&mut self, path: &Path) {
        let previous_sources = self.compress_sources.clone();
        let previous_count = previous_sources.len();
        let previous_suggested_output =
            suggested_archive_output_path(&previous_sources, self.compression_options.format)
                .map(|value| value.display().to_string());
        let current_output = self.compress_output_path.trim().to_string();

        self.compress_sources.retain(|source| source != path);
        if self.compress_sources.len() == previous_count {
            return;
        }

        self.retain_valid_compress_exclusions();
        if self.compress_sources.is_empty() {
            self.compress_excluded_paths.clear();
            self.compress_output_path.clear();
            self.reset_compress_browser();
        } else {
            if self
                .compress_browser_path
                .as_ref()
                .is_some_and(|current_dir| !self.is_path_within_any_compress_source(current_dir))
            {
                self.reset_compress_browser();
            }

            if self.compress_output_path.trim().is_empty()
                || previous_suggested_output
                    .as_ref()
                    .is_some_and(|value| value == &current_output)
            {
                if let Some(suggested_output) = suggested_archive_output_path(
                    &self.compress_sources,
                    self.compression_options.format,
                ) {
                    self.compress_output_path = suggested_output.display().to_string();
                }
            }
        }

        self.inspection = None;
        self.entries.clear();
        self.push_log(self.language.format(
            "Removed compression source: {path}. {count} source(s) remain.",
            "已移除待压缩源：{path}。当前剩余 {count} 项。",
            &[
                ("{path}", path.display().to_string()),
                ("{count}", self.compress_sources.len().to_string()),
            ],
        ));
        self.show_toast(
            FeedbackTone::Info,
            self.language.format(
                "{count} source(s) remain for compression.",
                "当前剩余 {count} 个待压缩源。",
                &[("{count}", self.compress_sources.len().to_string())],
            ),
        );
    }

    fn open_compress_source_picker(&mut self) {
        self.activate_workspace(WorkspaceMode::Compress);
        self.show_compress_source_picker = true;
    }

    fn draw_compress_source_picker(&mut self, ctx: &Context) {
        if !self.show_compress_source_picker {
            return;
        }

        let palette = self.palette();
        let modal_id = egui::Id::new("compress-source-picker");
        let backdrop_fade = anim_bool(ctx, modal_id.with("backdrop"), true, ANIM_NORMAL);
        let modal = egui::Modal::new(modal_id)
            .backdrop_color(Color32::from_black_alpha(
                ((if palette.is_dark { 160 } else { 96 }) as f32 * backdrop_fade) as u8,
            ))
            .frame(
                Frame::default()
                    .fill(palette.panel_fill)
                    .stroke(Stroke::new(1.0, palette.panel_stroke))
                    .corner_radius(18.0)
                    .inner_margin(Margin::same(18)),
            );

        let response = modal.show(ctx, |ui| {
            ui.set_min_width(320.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(self.t("Choose Source Type", "选择压缩源类型"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.add_space(14.0);

                if ui
                    .add_sized(
                        [284.0, 34.0],
                        Button::new(
                            RichText::new(self.t("Select Files", "选择文件"))
                                .strong()
                                .color(palette.on_primary),
                        )
                        .fill(palette.primary_strong)
                        .corner_radius(10.0),
                    )
                    .clicked()
                {
                    self.show_compress_source_picker = false;
                    self.schedule_dialog_action(ctx, PendingDialogAction::BrowseCompressFiles);
                }

                ui.add_space(8.0);

                if ui
                    .add_sized(
                        [284.0, 34.0],
                        Button::new(
                            RichText::new(self.t("Select Folders", "选择文件夹"))
                                .strong()
                                .color(palette.text),
                        )
                        .fill(palette.surface_high)
                        .stroke(Stroke::new(1.0, palette.outline_variant))
                        .corner_radius(10.0),
                    )
                    .clicked()
                {
                    self.show_compress_source_picker = false;
                    self.schedule_dialog_action(ctx, PendingDialogAction::BrowseCompressFolders);
                }
            });
        });

        if response.should_close() {
            self.show_compress_source_picker = false;
        }
    }

    fn browse_compress_output_path(&mut self) {
        self.activate_workspace(WorkspaceMode::Compress);

        let mut dialog = FileDialog::new();
        if let Some(default_output) =
            suggested_archive_output_path(&self.compress_sources, self.compression_options.format)
        {
            if let Some(parent) = default_output.parent() {
                dialog = dialog.set_directory(parent);
            }
            if let Some(file_name) = default_output.file_name() {
                dialog = dialog.set_file_name(file_name.to_string_lossy().as_ref());
            }
        } else if !self.compress_output_path.trim().is_empty() {
            let current_output = PathBuf::from(self.compress_output_path.trim());
            if let Some(parent) = current_output.parent() {
                dialog = dialog.set_directory(parent);
            }
            if let Some(file_name) = current_output.file_name() {
                dialog = dialog.set_file_name(file_name.to_string_lossy().as_ref());
            }
        }

        if let Some(path) = dialog.save_file() {
            self.compress_output_path =
                ensure_archive_extension(path, self.compression_options.format)
                    .display()
                    .to_string();
            self.push_log(self.language.format(
                "Selected archive output: {output_path}",
                "已选择压缩包输出路径：{output_path}",
                &[("{output_path}", self.compress_output_path.clone())],
            ));
            self.show_toast(
                FeedbackTone::Success,
                self.t("Output path updated.", "输出路径已更新。"),
            );
        } else {
            self.show_toast(
                FeedbackTone::Info,
                self.t(
                    "Output path selection was canceled.",
                    "已取消输出路径选择。",
                ),
            );
        }
    }

    fn browse_output_dir(&mut self) {
        let dialog = if self.output_dir.is_empty() {
            FileDialog::new()
        } else {
            FileDialog::new().set_directory(&self.output_dir)
        };

        if let Some(path) = dialog.pick_folder() {
            self.output_dir = path.display().to_string();
            self.push_log(self.language.format(
                "Selected output folder: {output_dir}",
                "已选择输出目录：{output_dir}",
                &[("{output_dir}", path.display().to_string())],
            ));
            self.show_toast(
                FeedbackTone::Success,
                self.t("Output folder updated.", "输出目录已更新。"),
            );
        } else {
            self.show_toast(
                FeedbackTone::Info,
                self.t(
                    "Output folder selection was canceled.",
                    "已取消输出目录选择。",
                ),
            );
        }
    }

    fn handle_dropped_files(&mut self, ctx: &Context) {
        let dropped_paths = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        if dropped_paths.is_empty() {
            return;
        }

        match self.side_nav {
            SideNavItem::Compress => self.handle_compress_drop(dropped_paths),
            SideNavItem::Extract => self.handle_extract_drop(dropped_paths),
            SideNavItem::FileManager
            | SideNavItem::Tasks
            | SideNavItem::Logs
            | SideNavItem::Settings => {}
        }
    }

    fn handle_compress_drop(&mut self, dropped_paths: Vec<PathBuf>) {
        let mut valid_paths = dropped_paths
            .into_iter()
            .filter(|path| path.exists())
            .collect::<Vec<_>>();
        valid_paths.sort();
        valid_paths.dedup();

        if valid_paths.is_empty() {
            self.show_toast(
                FeedbackTone::Error,
                self.t(
                    "No valid files were dropped.",
                    "没有检测到可用文件，请重新拖入。",
                ),
            );
            return;
        }

        self.activate_workspace(WorkspaceMode::Compress);
        self.show_compress_source_picker = false;
        if self.compress_sources.is_empty() {
            self.apply_compress_sources(valid_paths);
        } else {
            self.append_compress_sources(valid_paths);
        }
    }

    fn handle_extract_drop(&mut self, dropped_paths: Vec<PathBuf>) {
        if dropped_paths.len() != 1 {
            self.push_log(
                self.t(
                    "Extract drag-and-drop expects exactly one archive file.",
                    "解压拖拽时只能选择一个压缩包文件。",
                )
                .to_string(),
            );
            self.show_toast(
                FeedbackTone::Error,
                self.t(
                    "Invalid selection. Please choose one archive file again.",
                    "选择格式有误，请重新选择压缩包文件。",
                ),
            );
            return;
        }

        let Some(path) = dropped_paths.into_iter().next() else {
            return;
        };
        if !path.is_file() || self.service.inspect_archive(&path).is_err() {
            self.push_log(self.language.format(
                "Invalid extract drop: {path} is not a supported archive file.",
                "拖拽解压失败：{path} 不是受支持的压缩包文件。",
                &[("{path}", path.display().to_string())],
            ));
            self.show_toast(
                FeedbackTone::Error,
                self.t(
                    "Invalid selection. Please choose an archive file again.",
                    "选择格式有误，请重新选择压缩包文件。",
                ),
            );
            return;
        }

        if let Err(error) = self.apply_archive_selection(path) {
            self.push_log(self.language.format(
                "Archive drop failed: {error}",
                "拖拽压缩包失败：{error}",
                &[("{error}", format!("{error:#}"))],
            ));
            self.show_toast(
                FeedbackTone::Error,
                self.t(
                    "Invalid selection. Please choose an archive file again.",
                    "选择格式有误，请重新选择压缩包文件。",
                ),
            );
        }
    }

    fn scan_archive(&mut self) {
        if self.is_scanning_archive() {
            return;
        }

        let archive_path = match self.archive_path_buf() {
            Ok(path) => path,
            Err(message) => {
                self.push_log(message);
                return;
            }
        };

        let service = self.service.clone();
        let archive_password = self.extract_password.clone();
        let (tx, rx) = mpsc::channel();
        self.scan_receiver = Some(rx);
        self.push_log(self.language.format(
            "Scanning {path}",
            "正在扫描 {path}",
            &[("{path}", archive_path.display().to_string())],
        ));

        std::thread::spawn(move || {
            let result = (|| -> Result<ScanJobResult> {
                let inspection = service.inspect_archive(&archive_path)?;
                let entries = service
                    .list_archive_with_password(&archive_path, Some(archive_password.as_str()))?;
                Ok(ScanJobResult::Scanned {
                    inspection,
                    entries,
                })
            })();

            let job = match result {
                Ok(job) => job,
                Err(error) => ScanJobResult::Failed(format!("{error:#}")),
            };

            let _ = tx.send(job);
        });
    }

    fn test_current_archive(&mut self) {
        let archive_path = match self.archive_path_buf() {
            Ok(path) => path,
            Err(message) => {
                self.push_log(message);
                return;
            }
        };
        let password: Option<String> = (!self.extract_password.is_empty())
            .then_some(self.extract_password.clone());
        match self.service.test_archive_with_password(&archive_path, password.as_deref()) {
            Ok(report) => {
                self.test_report = Some(report);
                self.show_test_result_dialog = true;
            }
            Err(error) => {
                self.show_toast(
                    FeedbackTone::Error,
                    self.language.format(
                        "Archive test failed: {error}",
                        "压缩包测试失败：{error}",
                        &[("{error}", format!("{error:#}"))],
                    ),
                );
            }
        }
    }

    fn extract_archive(&mut self) {
        let archive_path = match self.archive_path_buf() {
            Ok(path) => path,
            Err(message) => {
                self.push_log(message);
                return;
            }
        };
        let output_dir = match self.output_dir_buf() {
            Ok(path) => path,
            Err(message) => {
                self.push_log(message);
                return;
            }
        };

        let options = ExtractOptions {
            output_dir: output_dir.clone(),
            overwrite_mode: self.overwrite_mode.to_core(),
            keep_paths: self.keep_paths,
            password: (!self.extract_password.is_empty()).then_some(self.extract_password.clone()),
            filename_encoding: self.filename_encoding,
            scan_files: self.scan_files,
        };

        let total_bytes = match self.estimate_extract_total_bytes(&archive_path) {
            Ok(total_bytes) => total_bytes,
            Err(error) => {
                self.push_log(self.language.format(
                    "Failed to inspect archive size: {error}",
                    "获取压缩包大小失败：{error}",
                    &[("{error}", format!("{error:#}"))],
                ));
                return;
            }
        };

        let pending_task = PendingExtractTask {
            archive_path: archive_path.clone(),
            options,
            plan: ExtractPathPlan::default(),
            total_bytes,
        };

        if self.overwrite_mode == GuiOverwriteMode::Ask {
            match self.prepare_extract_conflict_dialog(&pending_task) {
                Ok(Some(dialog)) => {
                    self.extract_conflict_dialog = Some(dialog);
                    return;
                }
                Ok(None) => {}
                Err(error) => {
                    self.push_log(self.language.format(
                        "Failed to inspect extraction conflicts: {error}",
                        "检查解压冲突失败：{error}",
                        &[("{error}", format!("{error:#}"))],
                    ));
                    return;
                }
            }
        }

        self.enqueue_extract_task(pending_task);
    }

    fn enqueue_extract_task(&mut self, pending_task: PendingExtractTask) {
        let archive_path = pending_task.archive_path.clone();
        let output_dir = pending_task.options.output_dir.clone();
        let task_id = self.enqueue_task(
            TaskKind::Extract,
            TaskSpec::Extract {
                archive_path: pending_task.archive_path,
                options: pending_task.options,
                plan: pending_task.plan,
            },
            pending_task.total_bytes,
        );
        self.push_log(self.language.format(
            "Queued extract task #{task_id}: {archive_path} -> {output_dir}",
            "已加入解压任务 #{task_id}：{archive_path} -> {output_dir}",
            &[
                ("{task_id}", task_id.to_string()),
                ("{archive_path}", archive_path.display().to_string()),
                ("{output_dir}", output_dir.display().to_string()),
            ],
        ));
        self.show_toast(
            FeedbackTone::Success,
            self.language.format(
                "Extract task #{task_id} added to queue.",
                "解压任务 #{task_id} 已加入队列。",
                &[("{task_id}", task_id.to_string())],
            ),
        );
        self.start_next_task();
    }

    fn prepare_extract_conflict_dialog(
        &self,
        pending_task: &PendingExtractTask,
    ) -> Result<Option<ExtractConflictDialogState>> {
        let entries = self.loaded_or_fetch_archive_entries(
            &pending_task.archive_path,
            pending_task.options.password.as_deref(),
        )?;
        let conflicts = self.collect_extract_conflicts(&entries, &pending_task.options)?;
        if conflicts.is_empty() {
            return Ok(None);
        }

        let rename_value = self
            .default_extract_conflict_rename_value(
                &conflicts[0],
                &pending_task.plan,
                &BTreeSet::new(),
            )
            .unwrap_or_default();

        Ok(Some(ExtractConflictDialogState {
            task: pending_task.clone(),
            conflicts,
            current_index: 0,
            apply_to_all: false,
            rename_value,
        }))
    }

    fn loaded_or_fetch_archive_entries(
        &self,
        archive_path: &Path,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>> {
        let current_archive = self.archive_path.trim();
        if !self.entries.is_empty() && current_archive == archive_path.to_string_lossy() {
            return Ok(self.entries.clone());
        }

        self.service
            .list_archive_with_password(archive_path, password)
    }

    fn collect_extract_conflicts(
        &self,
        entries: &[ArchiveEntry],
        options: &ExtractOptions,
    ) -> Result<Vec<ExtractConflictItem>> {
        let plan = ExtractPathPlan::default();
        let mut conflicts = Vec::new();

        for entry in entries.iter().filter(|entry| !entry.is_dir) {
            let destination = resolve_output_path(&entry.path, options, &plan)?;
            if destination.exists() {
                conflicts.push(ExtractConflictItem {
                    relative_path: entry.path.clone(),
                    existing_is_dir: destination.is_dir(),
                    destination,
                });
            }
        }

        Ok(conflicts)
    }

    fn reserved_extract_conflict_paths(dialog: &ExtractConflictDialogState) -> BTreeSet<PathBuf> {
        dialog.task.plan.renamed_paths.values().cloned().collect()
    }

    fn default_extract_conflict_rename_value(
        &self,
        conflict: &ExtractConflictItem,
        plan: &ExtractPathPlan,
        reserved_paths: &BTreeSet<PathBuf>,
    ) -> Option<String> {
        let candidate =
            suggest_available_conflict_path(&conflict.destination, plan, reserved_paths);
        candidate
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
    }

    fn compress_archive(&mut self) {
        let sources = match self.compress_sources_buf() {
            Ok(sources) => sources,
            Err(message) => {
                self.push_log(message);
                return;
            }
        };
        let output_path = match self.compress_output_path_buf() {
            Ok(path) => path,
            Err(message) => {
                self.push_log(message);
                return;
            }
        };
        let excluded_paths = self.compress_excluded_paths.clone();
        let mut options = self.compression_options.clone();
        let password = options.password.clone().unwrap_or_default();
        if !password.is_empty() || !self.compression_password_confirm.is_empty() {
            if password.is_empty() {
                self.push_log(self.t(
                    "Compression password is empty. Enter a password and confirm it before starting.",
                    "压缩密码为空。开始前请输入密码并再次确认。",
                )
                .to_string());
                return;
            }
            if password != self.compression_password_confirm {
                self.push_log(
                    self.t(
                        "Compression passwords do not match.",
                        "压缩密码两次输入不一致。",
                    )
                    .to_string(),
                );
                return;
            }
        }
        if password.is_empty() {
            options.password = None;
            options.encrypt_file_names = false;
        }
        if options.password.is_some() && !options.format.supports_password_encryption() {
            self.push_log(format!(
                "{} {}",
                compression_format_label(options.format, self.language),
                self.t(
                    "does not support password encryption in FastZIP.",
                    "当前在 FastZIP 中不支持密码加密。"
                )
            ));
            return;
        }
        if options.encrypt_file_names && !options.format.supports_encrypt_file_names() {
            self.push_log(
                self.t(
                    "Only 7Z archives support encrypted file names.",
                    "只有 7Z 格式支持加密文件名。",
                )
                .to_string(),
            );
            return;
        }
        let split_volume_size_input = self.compression_split_volume_size_input.trim();
        options.split_volume_size = if split_volume_size_input.is_empty() {
            None
        } else {
            match parse_volume_size_spec(split_volume_size_input) {
                Ok(size) => Some(size),
                Err(error) => {
                    self.push_log(self.language.format(
                        "Invalid split volume size `{value}`: {error}",
                        "分卷大小 `{value}` 无效：{error}",
                        &[
                            ("{value}", split_volume_size_input.to_string()),
                            ("{error}", format!("{error:#}")),
                        ],
                    ));
                    return;
                }
            }
        };

        let total_bytes = match total_bytes_for_sources(&sources, &excluded_paths) {
            Ok(total_bytes) => total_bytes,
            Err(error) => {
                self.push_log(self.language.format(
                    "Failed to inspect source size: {error}",
                    "获取源文件大小失败：{error}",
                    &[("{error}", format!("{error:#}"))],
                ));
                return;
            }
        };

        let task_id = self.enqueue_task(
            TaskKind::Compress,
            TaskSpec::Compress {
                sources: sources.clone(),
                excluded_paths: excluded_paths.clone(),
                output_path: output_path.clone(),
                options: options.clone(),
            },
            total_bytes,
        );
        self.push_log(self.language.format(
            "Queued compress task #{task_id}: {format}, {sources} source(s), {excluded} exclusion(s) -> {output_path}",
            "已加入压缩任务 #{task_id}：{format}，{sources} 个源，排除 {excluded} 项 -> {output_path}",
            &[
                ("{task_id}", task_id.to_string()),
                (
                    "{format}",
                    compression_format_label(options.format, self.language).to_string(),
                ),
                ("{sources}", sources.len().to_string()),
                ("{excluded}", excluded_paths.len().to_string()),
                ("{output_path}", output_path.display().to_string()),
            ],
        ));
        self.show_toast(
            FeedbackTone::Success,
            self.language.format(
                "Compress task #{task_id} added to queue.",
                "压缩任务 #{task_id} 已加入队列。",
                &[("{task_id}", task_id.to_string())],
            ),
        );
        self.start_next_task();
    }

    fn estimate_extract_total_bytes(&self, archive_path: &Path) -> Result<u64> {
        let fallback = archive_path
            .metadata()
            .ok()
            .map(|meta| meta.len())
            .unwrap_or(0);

        let current_archive = self.archive_path.trim();
        if !self.entries.is_empty() && current_archive == archive_path.to_string_lossy() {
            let all_known = self
                .entries
                .iter()
                .filter(|entry| !entry.is_dir)
                .all(|entry| entry.uncompressed_size.is_some());
            let total: u64 = self
                .entries
                .iter()
                .filter_map(|entry| entry.uncompressed_size)
                .sum();
            return Ok(if all_known && total > 0 {
                total
            } else {
                fallback
            });
        }

        let entries = self
            .service
            .list_archive_with_password(archive_path, Some(self.extract_password.as_str()))?;
        let all_known = entries
            .iter()
            .filter(|entry| !entry.is_dir)
            .all(|entry| entry.uncompressed_size.is_some());
        let total: u64 = entries
            .iter()
            .filter_map(|entry| entry.uncompressed_size)
            .sum();
        Ok(if all_known && total > 0 {
            total
        } else {
            fallback
        })
    }

    fn enqueue_task(&mut self, kind: TaskKind, spec: TaskSpec, total_bytes: u64) -> u64 {
        let task_id = self.next_task_id;
        self.next_task_id += 1;
        self.task_queue.push(TaskQueueItem {
            id: task_id,
            kind,
            spec,
            total_bytes,
            processed_bytes: 0,
            current_bytes_per_second: None,
            state: TaskState::Queued,
            started_at: None,
            finished_at: None,
            error_message: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        });
        task_id
    }

    fn start_next_task(&mut self) {
        if self.has_active_task() {
            return;
        }

        let Some(index) = self
            .task_queue
            .iter()
            .position(|task| task.state == TaskState::Queued)
        else {
            return;
        };

        let task_id;
        let spec;
        {
            let task = &mut self.task_queue[index];
            task.state = TaskState::Running;
            task.started_at = Some(Instant::now());
            task.finished_at = None;
            task.processed_bytes = 0;
            task.current_bytes_per_second = None;
            task.error_message = None;
            task.cancel_flag.store(false, Ordering::Relaxed);
            task_id = task.id;
            spec = task.spec.clone();
        }

        self.push_log(match &spec {
            TaskSpec::Compress { output_path, .. } => self.language.format(
                "Started compress task #{task_id} -> {output_path}",
                "开始执行压缩任务 #{task_id} -> {output_path}",
                &[
                    ("{task_id}", task_id.to_string()),
                    ("{output_path}", output_path.display().to_string()),
                ],
            ),
            TaskSpec::Extract { options, .. } => self.language.format(
                "Started extract task #{task_id} -> {output_dir}",
                "开始执行解压任务 #{task_id} -> {output_dir}",
                &[
                    ("{task_id}", task_id.to_string()),
                    ("{output_dir}", options.output_dir.display().to_string()),
                ],
            ),
        });

        let service = self.service.clone();
        let (tx, rx) = mpsc::channel();
        let cancel_flag = self.task_queue[index].cancel_flag.clone();
        self.task_receiver = Some(rx);

        std::thread::spawn(move || {
            let mut processed_bytes = 0u64;
            let mut last_emit = Instant::now();
            let mut bytes_since_emit = 0u64;
            let progress_tx = tx.clone();
            let cancel_for_progress = cancel_flag.clone();
            let mut progress = |delta: u64| {
                if cancel_for_progress.load(Ordering::Relaxed) {
                    return;
                }
                processed_bytes = processed_bytes.saturating_add(delta);
                bytes_since_emit = bytes_since_emit.saturating_add(delta);
                let now = Instant::now();
                if now.saturating_duration_since(last_emit) >= Duration::from_millis(120) {
                    let elapsed = now.saturating_duration_since(last_emit).as_secs_f64();
                    let bytes_per_second = if elapsed > 0.0 {
                        bytes_since_emit as f64 / elapsed
                    } else {
                        0.0
                    };
                    let _ = progress_tx.send(TaskJobResult::Progress {
                        task_id,
                        processed_bytes,
                        bytes_per_second,
                    });
                    last_emit = now;
                    bytes_since_emit = 0;
                }
            };

            let job = match spec {
                TaskSpec::Compress {
                    sources,
                    excluded_paths,
                    output_path,
                    options,
                } => match service.compress_with_options_and_exclusions_and_progress_and_cancel(
                    &sources,
                    &excluded_paths,
                    &output_path,
                    options,
                    &mut progress,
                    &mut || cancel_flag.load(Ordering::Relaxed),
                ) {
                    Ok(report) => {
                        let elapsed = Instant::now()
                            .saturating_duration_since(last_emit)
                            .as_secs_f64();
                        let bytes_per_second = if elapsed > 0.0 {
                            bytes_since_emit as f64 / elapsed
                        } else {
                            0.0
                        };
                        let _ = tx.send(TaskJobResult::Progress {
                            task_id,
                            processed_bytes,
                            bytes_per_second,
                        });
                        TaskJobResult::Compressed { task_id, report }
                    }
                    Err(error) => {
                        if cancel_flag.load(Ordering::Relaxed) {
                            TaskJobResult::Canceled { task_id }
                        } else {
                            TaskJobResult::Failed {
                                task_id,
                                message: format!("{error:#}"),
                            }
                        }
                    }
                },
                TaskSpec::Extract {
                    archive_path,
                    options,
                    plan,
                } => match service.extract_archive_with_progress_and_cancel_with_plan(
                    &archive_path,
                    &options,
                    &plan,
                    &mut progress,
                    &mut || cancel_flag.load(Ordering::Relaxed),
                ) {
                    Ok(report) => {
                        let elapsed = Instant::now()
                            .saturating_duration_since(last_emit)
                            .as_secs_f64();
                        let bytes_per_second = if elapsed > 0.0 {
                            bytes_since_emit as f64 / elapsed
                        } else {
                            0.0
                        };
                        let _ = tx.send(TaskJobResult::Progress {
                            task_id,
                            processed_bytes,
                            bytes_per_second,
                        });
                        TaskJobResult::Extracted { task_id, report }
                    }
                    Err(error) => {
                        if cancel_flag.load(Ordering::Relaxed) {
                            TaskJobResult::Canceled { task_id }
                        } else {
                            TaskJobResult::Failed {
                                task_id,
                                message: format!("{error:#}"),
                            }
                        }
                    }
                },
            };

            let _ = tx.send(job);
        });
    }

    fn can_prioritize_task(&self, task_id: u64) -> bool {
        let Some(first_queued_index) = self
            .task_queue
            .iter()
            .position(|task| task.state == TaskState::Queued)
        else {
            return false;
        };

        self.task_queue[first_queued_index].id != task_id
    }

    fn prioritize_task(&mut self, task_id: u64) {
        let Some(index) = self
            .task_queue
            .iter()
            .position(|task| task.id == task_id && task.state == TaskState::Queued)
        else {
            return;
        };

        let Some(first_queued_index) = self
            .task_queue
            .iter()
            .position(|task| task.state == TaskState::Queued)
        else {
            return;
        };

        if index == first_queued_index {
            self.show_toast(
                FeedbackTone::Info,
                self.t(
                    "This task is already next in queue.",
                    "该任务已经是下一个执行。",
                ),
            );
            if !self.has_active_task() {
                self.start_next_task();
            }
            return;
        }

        let task = self.task_queue.remove(index);
        self.task_queue.insert(first_queued_index, task);
        self.push_log(self.language.format(
            "Task #{task_id} moved to the front of the queue.",
            "任务 #{task_id} 已提前到队列最前。",
            &[("{task_id}", task_id.to_string())],
        ));
        self.show_toast(
            FeedbackTone::Success,
            self.language.format(
                "Task #{task_id} will run next.",
                "任务 #{task_id} 将优先执行。",
                &[("{task_id}", task_id.to_string())],
            ),
        );

        if !self.has_active_task() {
            self.start_next_task();
        }
    }

    fn cancel_task(&mut self, task_id: u64) {
        let Some(index) = self.task_queue.iter().position(|task| task.id == task_id) else {
            return;
        };

        match self.task_queue[index].state {
            TaskState::Queued => {
                let task = &mut self.task_queue[index];
                task.state = TaskState::Canceled;
                task.finished_at = Some(Instant::now());
                task.current_bytes_per_second = None;
                task.error_message = None;
                task.cancel_flag.store(true, Ordering::Relaxed);
                self.push_log(self.language.format(
                    "Canceled queued task #{task_id}.",
                    "已取消排队任务 #{task_id}。",
                    &[("{task_id}", task_id.to_string())],
                ));
                self.show_toast(
                    FeedbackTone::Info,
                    self.language.format(
                        "Task #{task_id} canceled.",
                        "任务 #{task_id} 已取消。",
                        &[("{task_id}", task_id.to_string())],
                    ),
                );
                if !self.has_active_task() {
                    self.start_next_task();
                }
            }
            TaskState::Running => {
                let task = &mut self.task_queue[index];
                task.state = TaskState::Canceling;
                task.cancel_flag.store(true, Ordering::Relaxed);
                self.push_log(self.language.format(
                    "Cancel requested for task #{task_id}.",
                    "已请求取消任务 #{task_id}。",
                    &[("{task_id}", task_id.to_string())],
                ));
                self.show_toast(
                    FeedbackTone::Info,
                    self.language.format(
                        "Canceling task #{task_id}...",
                        "正在取消任务 #{task_id}...",
                        &[("{task_id}", task_id.to_string())],
                    ),
                );
            }
            TaskState::Canceling => {
                self.show_toast(
                    FeedbackTone::Info,
                    self.t("This task is already canceling.", "该任务已经在取消中。"),
                );
            }
            TaskState::Completed | TaskState::Canceled | TaskState::Failed => {}
        }
    }

    fn rerun_task(&mut self, task_id: u64) {
        let Some(index) = self.task_queue.iter().position(|task| task.id == task_id) else {
            return;
        };

        let task = &mut self.task_queue[index];
        if !matches!(task.state, TaskState::Canceled | TaskState::Failed) {
            return;
        }

        task.state = TaskState::Queued;
        task.processed_bytes = 0;
        task.current_bytes_per_second = None;
        task.started_at = None;
        task.finished_at = None;
        task.error_message = None;
        task.cancel_flag.store(false, Ordering::Relaxed);

        self.push_log(self.language.format(
            "Task #{task_id} queued again.",
            "任务 #{task_id} 已重新加入队列。",
            &[("{task_id}", task_id.to_string())],
        ));
        self.show_toast(
            FeedbackTone::Success,
            self.language.format(
                "Task #{task_id} queued again.",
                "任务 #{task_id} 已重新加入队列。",
                &[("{task_id}", task_id.to_string())],
            ),
        );

        if !self.has_active_task() {
            self.start_next_task();
        }
    }

    fn delete_task(&mut self, task_id: u64) {
        let Some(index) = self.task_queue.iter().position(|task| task.id == task_id) else {
            return;
        };

        if !matches!(
            self.task_queue[index].state,
            TaskState::Completed | TaskState::Canceled | TaskState::Failed
        ) {
            return;
        }

        self.task_queue.remove(index);
        self.push_log(self.language.format(
            "Removed task #{task_id} from the queue.",
            "已从队列删除任务 #{task_id}。",
            &[("{task_id}", task_id.to_string())],
        ));
        self.show_toast(
            FeedbackTone::Info,
            self.language.format(
                "Task #{task_id} removed.",
                "任务 #{task_id} 已删除。",
                &[("{task_id}", task_id.to_string())],
            ),
        );
    }

    fn open_task_output_dir(&mut self, task_id: u64) {
        let Some(task) = self.task_queue.iter().find(|task| task.id == task_id) else {
            return;
        };
        if task.state != TaskState::Completed {
            return;
        }

        let target_dir = match &task.spec {
            TaskSpec::Compress { output_path, .. } => output_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
            TaskSpec::Extract { options, .. } => options.output_dir.clone(),
        };

        match open_path_with_system_default(&target_dir) {
            Ok(()) => {
                self.push_log(self.language.format(
                    "Opened output directory for task #{task_id}.",
                    "已打开任务 #{task_id} 的输出目录。",
                    &[("{task_id}", task_id.to_string())],
                ));
            }
            Err(error) => {
                self.push_log(self.language.format(
                    "Failed to open output directory for task #{task_id}: {error}",
                    "打开任务 #{task_id} 输出目录失败：{error}",
                    &[
                        ("{task_id}", task_id.to_string()),
                        ("{error}", format!("{error:#}")),
                    ],
                ));
                self.show_toast(
                    FeedbackTone::Error,
                    self.language.format(
                        "Failed to open output directory for task #{task_id}.",
                        "无法打开任务 #{task_id} 的输出目录。",
                        &[("{task_id}", task_id.to_string())],
                    ),
                );
            }
        }
    }

    fn handle_task_queue_row_event(&mut self, event: TaskQueueRowEvent) {
        match event {
            TaskQueueRowEvent::ShowOutputPath(path) => {
                self.expanded_task_output_path = Some(path);
            }
            TaskQueueRowEvent::Prioritize(task_id) => self.prioritize_task(task_id),
            TaskQueueRowEvent::Cancel(task_id) => self.cancel_task(task_id),
            TaskQueueRowEvent::Rerun(task_id) => self.rerun_task(task_id),
            TaskQueueRowEvent::Delete(task_id) => self.delete_task(task_id),
            TaskQueueRowEvent::OpenOutputDir(task_id) => self.open_task_output_dir(task_id),
        }
    }

    fn archive_path_buf(&self) -> std::result::Result<PathBuf, String> {
        let path = self.archive_path.trim();
        if path.is_empty() {
            return Err(self
                .t("Pick an archive first.", "请先选择压缩包。")
                .to_string());
        }

        let archive_path = PathBuf::from(path);
        if !archive_path.is_file() {
            return Err(self.language.format(
                "Archive not found: {archive_path}",
                "未找到压缩包：{archive_path}",
                &[("{archive_path}", archive_path.display().to_string())],
            ));
        }

        Ok(archive_path)
    }

    fn compress_sources_buf(&self) -> std::result::Result<Vec<PathBuf>, String> {
        if self.compress_sources.is_empty() {
            return Err(self
                .t(
                    "Pick files or folders to compress first.",
                    "请先选择待压缩的文件或文件夹。",
                )
                .to_string());
        }

        for source in &self.compress_sources {
            if !source.exists() {
                return Err(self.language.format(
                    "Compression source not found: {source_path}",
                    "未找到待压缩源：{source_path}",
                    &[("{source_path}", source.display().to_string())],
                ));
            }
        }

        Ok(self.compress_sources.clone())
    }

    fn compress_output_path_buf(&self) -> std::result::Result<PathBuf, String> {
        let path = self.compress_output_path.trim();
        if path.is_empty() {
            return Err(self
                .t(
                    "Choose the archive output path first.",
                    "请先选择压缩包输出路径。",
                )
                .to_string());
        }

        Ok(ensure_archive_extension(
            PathBuf::from(path),
            self.compression_options.format,
        ))
    }

    fn output_dir_buf(&self) -> std::result::Result<PathBuf, String> {
        let path = self.output_dir.trim();
        if path.is_empty() {
            return Err(self
                .t("Choose an output folder first.", "请先选择输出目录。")
                .to_string());
        }

        Ok(PathBuf::from(path))
    }

    fn push_log(&mut self, line: impl Into<String>) {
        self.logs.push(line.into());
        if self.logs.len() > 200 {
            let drain = self.logs.len() - 200;
            self.logs.drain(0..drain);
        }
    }

    fn show_toast(&mut self, tone: FeedbackTone, text: impl Into<String>) {
        self.toast = Some(ToastMessage {
            text: text.into(),
            tone,
            created_at: Instant::now(),
            duration: Duration::from_millis(2100),
        });
    }

    fn running_task_count(&self) -> usize {
        self.task_queue
            .iter()
            .filter(|task| matches!(task.state, TaskState::Running | TaskState::Canceling))
            .count()
    }

    fn queued_task_count(&self) -> usize {
        self.task_queue
            .iter()
            .filter(|task| task.state == TaskState::Queued)
            .count()
    }

    fn workspace_action_busy(&self) -> bool {
        match self.workspace_mode {
            WorkspaceMode::Compress => {
                self.show_compress_source_picker
                    || matches!(
                        self.pending_dialog_action,
                        Some(
                            PendingDialogAction::BrowseCompressFiles
                                | PendingDialogAction::BrowseCompressFolders
                        )
                    )
            }
            WorkspaceMode::Extract => {
                self.is_scanning_archive()
                    || matches!(
                        self.pending_dialog_action,
                        Some(PendingDialogAction::BrowseArchive)
                    )
            }
        }
    }

    fn live_status(&self) -> (String, bool, FeedbackTone) {
        if self.show_compress_source_picker {
            return (
                self.t("Waiting for source selection", "等待选择压缩源")
                    .to_string(),
                true,
                FeedbackTone::Info,
            );
        }

        if self.pending_dialog_action.is_some() {
            return (
                self.t("Opening file picker", "正在打开选择窗口")
                    .to_string(),
                true,
                FeedbackTone::Info,
            );
        }

        if self.is_scanning_archive() {
            return (
                self.t("Scanning archive contents", "正在扫描压缩包内容")
                    .to_string(),
                true,
                FeedbackTone::Info,
            );
        }

        let running = self.running_task_count();
        let queued = self.queued_task_count();
        if running > 0 {
            return (
                if queued > 0 {
                    self.language.format(
                        "{running} running · {queued} queued",
                        "{running} 个进行中 · {queued} 个排队中",
                        &[
                            ("{running}", running.to_string()),
                            ("{queued}", queued.to_string()),
                        ],
                    )
                } else {
                    self.language.format(
                        "{running} running",
                        "{running} 个进行中",
                        &[("{running}", running.to_string())],
                    )
                },
                true,
                FeedbackTone::Success,
            );
        }

        if queued > 0 {
            return (
                self.language.format(
                    "{queued} queued",
                    "{queued} 个排队中",
                    &[("{queued}", queued.to_string())],
                ),
                false,
                FeedbackTone::Info,
            );
        }

        (
            self.t("Ready", "就绪").to_string(),
            false,
            FeedbackTone::Info,
        )
    }

    fn needs_live_animation(&self) -> bool {
        self.toast.is_some()
            || self.show_compress_source_picker
            || self.pending_dialog_action.is_some()
            || self.is_scanning_archive()
            || self.has_active_task()
    }

    fn backend_label(&self, kind: BackendKind) -> &'static str {
        match kind {
            BackendKind::Native => self.t("Native Rust core", "原生 Rust 内核"),
            BackendKind::RarAdapter => self.t("RAR adapter", "RAR 适配层"),
        }
    }

    fn poll_scan_jobs(&mut self, ctx: &Context) {
        if let Some(receiver) = &self.scan_receiver {
            match receiver.try_recv() {
                Ok(job) => {
                    self.scan_receiver = None;
                    match job {
                        ScanJobResult::Scanned {
                            mut inspection,
                            entries,
                        } => {
                            let previous_suggested_output =
                                inspection.suggested_output_dir.display().to_string();
                            let refined_suggested_output = suggested_extract_output_dir(
                                &inspection.archive_path,
                                &entries,
                                self.keep_paths,
                            );
                            if self.output_dir.trim().is_empty()
                                || self.output_dir.trim() == previous_suggested_output
                            {
                                self.output_dir = refined_suggested_output.display().to_string();
                            }
                            inspection.suggested_output_dir = refined_suggested_output;
                            self.push_log(self.language.format(
                                "Loaded {count} entries with {backend}.",
                                "已通过 {backend} 加载 {count} 个条目。",
                                &[
                                    ("{count}", entries.len().to_string()),
                                    (
                                        "{backend}",
                                        self.backend_label(inspection.backend_kind).to_string(),
                                    ),
                                ],
                            ));
                            self.show_toast(
                                FeedbackTone::Success,
                                self.language.format(
                                    "Loaded {count} archive entries.",
                                    "已加载 {count} 个压缩包条目。",
                                    &[("{count}", entries.len().to_string())],
                                ),
                            );
                            self.inspection = Some(inspection);
                            self.entries = entries;
                        }
                        ScanJobResult::Failed(message) => {
                            self.push_log(self.language.format(
                                "Scan failed: {message}",
                                "扫描失败：{message}",
                                &[("{message}", message)],
                            ));
                            self.show_toast(
                                FeedbackTone::Error,
                                self.t("Archive scan failed.", "压缩包扫描失败。"),
                            );
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scan_receiver = None;
                }
            }
        }
    }

    fn poll_benchmark_jobs(&mut self) {
        if let Some(receiver) = &self.benchmark_receiver {
            match receiver.try_recv() {
                Ok(lines) => {
                    self.benchmark_receiver = None;
                    for line in lines {
                        self.push_log(line);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.benchmark_receiver = None;
                }
            }
        }
    }

    fn poll_task_jobs(&mut self, ctx: &Context) {
        if let Some(receiver) = &self.task_receiver {
            loop {
                match receiver.try_recv() {
                    Ok(TaskJobResult::Progress {
                        task_id,
                        processed_bytes,
                        bytes_per_second,
                    }) => {
                        if let Some(task) =
                            self.task_queue.iter_mut().find(|task| task.id == task_id)
                        {
                            task.processed_bytes = if task.total_bytes == 0 {
                                processed_bytes
                            } else {
                                processed_bytes.min(task.total_bytes)
                            };
                            task.current_bytes_per_second =
                                (bytes_per_second > 0.0).then_some(bytes_per_second);
                        }
                    }
                    Ok(job) => {
                        self.task_receiver = None;
                        self.finish_task(job);
                        self.start_next_task();
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        ctx.request_repaint_after(Duration::from_millis(100));
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.task_receiver = None;
                        break;
                    }
                }
            }
        }
    }

    fn finish_task(&mut self, job: TaskJobResult) {
        match job {
            TaskJobResult::Compressed { task_id, report } => {
                let mut original_output_path = None;
                if let Some(task) = self.task_queue.iter_mut().find(|task| task.id == task_id) {
                    if let TaskSpec::Compress { output_path, .. } = &task.spec {
                        original_output_path = Some(output_path.display().to_string());
                    }
                    task.state = TaskState::Completed;
                    task.processed_bytes = task.total_bytes;
                    task.finished_at = Some(Instant::now());
                    task.current_bytes_per_second = completed_task_rate(task);
                }
                if let Some(original_output_path) = original_output_path {
                    self.compress_output_path = original_output_path;
                }
                self.push_log(self.language.format(
                    "Compression finished for task #{task_id}: {files} file(s), {dirs} folder(s), output {output_path}.",
                    "压缩任务 #{task_id} 完成：{files} 个文件、{dirs} 个文件夹，输出 {output_path}。",
                    &[
                        ("{task_id}", task_id.to_string()),
                        ("{files}", report.files_added.to_string()),
                        ("{dirs}", report.directories_added.to_string()),
                        ("{output_path}", report.archive_path.display().to_string()),
                    ],
                ));
                self.show_toast(
                    FeedbackTone::Success,
                    self.language.format(
                        "Compression task #{task_id} completed.",
                        "压缩任务 #{task_id} 已完成。",
                        &[("{task_id}", task_id.to_string())],
                    ),
                );
            }
            TaskJobResult::Extracted { task_id, report } => {
                if let Some(task) = self.task_queue.iter_mut().find(|task| task.id == task_id) {
                    task.state = TaskState::Completed;
                    task.processed_bytes = task.total_bytes;
                    task.finished_at = Some(Instant::now());
                    task.current_bytes_per_second = completed_task_rate(task);
                }
                self.push_log(self.language.format(
                    "Extraction finished for task #{task_id}: {files} file(s), {dirs} directorie(s).",
                    "解压任务 #{task_id} 完成：{files} 个文件，{dirs} 个目录。",
                    &[
                        ("{task_id}", task_id.to_string()),
                        ("{files}", report.files_written.to_string()),
                        ("{dirs}", report.directories_created.to_string()),
                    ],
                ));
                self.show_toast(
                    FeedbackTone::Success,
                    self.language.format(
                        "Extraction task #{task_id} completed.",
                        "解压任务 #{task_id} 已完成。",
                        &[("{task_id}", task_id.to_string())],
                    ),
                );
            }
            TaskJobResult::Canceled { task_id } => {
                if let Some(task) = self.task_queue.iter_mut().find(|task| task.id == task_id) {
                    task.state = TaskState::Canceled;
                    task.finished_at = Some(Instant::now());
                    task.current_bytes_per_second = None;
                }
                self.push_log(self.language.format(
                    "Task #{task_id} was canceled.",
                    "任务 #{task_id} 已取消。",
                    &[("{task_id}", task_id.to_string())],
                ));
                self.show_toast(
                    FeedbackTone::Info,
                    self.language.format(
                        "Task #{task_id} canceled.",
                        "任务 #{task_id} 已取消。",
                        &[("{task_id}", task_id.to_string())],
                    ),
                );
            }
            TaskJobResult::Failed { task_id, message } => {
                if let Some(task) = self.task_queue.iter_mut().find(|task| task.id == task_id) {
                    task.state = TaskState::Failed;
                    task.finished_at = Some(Instant::now());
                    task.error_message = Some(message.clone());
                }
                self.push_log(self.language.format(
                    "Task #{task_id} failed: {message}",
                    "任务 #{task_id} 失败：{message}",
                    &[("{task_id}", task_id.to_string()), ("{message}", message)],
                ));
                self.show_toast(
                    FeedbackTone::Error,
                    self.language.format(
                        "Task #{task_id} failed.",
                        "任务 #{task_id} 失败。",
                        &[("{task_id}", task_id.to_string())],
                    ),
                );
            }
            TaskJobResult::Progress { .. } => {}
        }
    }

    fn draw_top_bar(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let (live_label, live_busy, live_tone) = self.live_status();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;

            ui.label(
                RichText::new("FastZIP")
                    .size(18.0)
                    .strong()
                    .color(palette.text),
            );

            live_status_chip(ui, &live_label, live_busy, live_tone, palette);

            let controls_width = 92.0;
            let drag_width = (ui.available_width() - controls_width).max(0.0);
            let (_, drag_response) =
                ui.allocate_exact_size(Vec2::new(drag_width, 34.0), egui::Sense::drag());
            if drag_response.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }

            ui.allocate_ui_with_layout(
                Vec2::new(controls_width, 34.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if ui
                        .add_sized([40.0, 30.0], chrome_button("-", false, palette))
                        .clicked()
                    {
                        self.minimize_window(ui.ctx());
                    }
                    if ui
                        .add_sized([40.0, 30.0], chrome_button("x", true, palette))
                        .clicked()
                    {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                },
            );
        });
    }

    #[cfg(target_os = "windows")]
    fn minimize_window(&self, ctx: &Context) {
        if let Some(hwnd) = self.root_hwnd {
            unsafe {
                configure_taskbar_window(hwnd);
                let _ = ShowWindow(hwnd, SW_SHOWMINIMIZED);
            }
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }

    #[cfg(not(target_os = "windows"))]
    fn minimize_window(&self, ctx: &Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }

    fn draw_side_nav(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let separator_x = ui.max_rect().right() + 16.0;

        ui.add_space(12.0);
        ui.label(
            RichText::new(self.t("Library", "资源库"))
                .size(14.0)
                .strong()
                .color(palette.text),
        );
        ui.label(
            RichText::new(self.t("Manage archives", "管理压缩包"))
                .size(11.0)
                .color(palette.text_muted),
        );
        ui.add_space(20.0);

        self.side_nav_item(ui, SideNavItem::Compress, self.t("Compress", "压缩"));
        self.side_nav_item(ui, SideNavItem::Extract, self.t("Extract", "解压"));
        self.side_nav_item(
            ui,
            SideNavItem::FileManager,
            self.t("File Manager", "文件管理器"),
        );
        self.side_nav_item(ui, SideNavItem::Tasks, self.t("Task Queue", "任务队列"));
        self.side_nav_item(ui, SideNavItem::Logs, self.t("Logs", "日志"));
        self.side_nav_item(ui, SideNavItem::Settings, self.t("Settings", "设置"));

        ui.painter().line_segment(
            [
                egui::pos2(separator_x, ui.max_rect().top() - 16.0),
                egui::pos2(separator_x, ui.max_rect().bottom() + 16.0),
            ],
            Stroke::new(1.0, palette.panel_stroke),
        );
    }

    fn side_nav_item(&mut self, ui: &mut egui::Ui, item: SideNavItem, label: &str) {
        let palette = self.palette();
        let active = self.side_nav == item;
        let t = anim_bool(
            ui.ctx(),
            egui::Id::new(("side-nav", item)),
            active,
            ANIM_NORMAL,
        );

        let text_color = lerp_color(palette.nav_inactive_text, palette.nav_active_text, t);
        let fill = lerp_color(Color32::TRANSPARENT, palette.nav_active_fill, t);
        let stroke_color = lerp_color(Color32::TRANSPARENT, palette.nav_active_stroke, t);

        let button = Button::new(RichText::new(label).color(text_color))
            .fill(fill)
            .stroke(Stroke::new(1.0, stroke_color))
            .corner_radius(10.0)
            .min_size(Vec2::new(172.0, 34.0));

        if ui.add(button).clicked() {
            self.set_side_nav(item);
        }
        ui.add_space(4.0);
    }

    fn draw_workspace_dashboard(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();

        paint_background(ui, palette);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                self.draw_workspace_action(ui);
                ui.add_space(24.0);
                self.draw_path_selectors(ui);
                if self.workspace_mode == WorkspaceMode::Compress {
                    ui.add_space(24.0);
                    self.draw_compression_options_panel(ui);
                } else {
                    ui.add_space(24.0);
                    self.draw_extract_options_panel(ui);
                }
                ui.add_space(24.0);
                self.draw_queue_panel(ui);
            });
    }

    fn draw_file_manager_page(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();

        paint_background(ui, palette);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                self.draw_file_manager_toolbar(ui);
                ui.add_space(24.0);
                self.draw_file_manager_panel(ui);
            });
    }

    fn draw_file_manager_toolbar(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let mut current_path = self.file_manager_path_input.clone();
        let selected_count = self
            .file_manager_selected_paths
            .iter()
            .filter(|path| path.exists())
            .count();
        let compress_label = if selected_count == 0 {
            self.t("Compress", "压缩").to_string()
        } else {
            format!("{} ({selected_count})", self.t("Compress", "压缩"))
        };
        let mut open_requested = false;
        let mut browse_requested = false;
        let mut compress_requested = false;
        let mut checksum_requested = false;

        glass_panel(palette.panel_fill, palette).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.t("File Manager", "文件管理器"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    status_badge(
                        ui,
                        self.language.format(
                            "{count} Items",
                            "{count} 项",
                            &[("{count}", selected_count.to_string())],
                        ),
                        palette.badge_fill,
                        palette.badge_text,
                    );
                });
            });
            ui.add_space(10.0);
            thin_separator(ui, palette.outline_variant);
            ui.add_space(14.0);

            ui.label(
                RichText::new(self.t("Current Path", "当前路径"))
                    .size(10.5)
                    .strong()
                    .color(palette.text_secondary),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let input_width = (ui.available_width() - 386.0).max(120.0);
                let response = ui.add_sized(
                    [input_width, 30.0],
                    TextEdit::singleline(&mut current_path)
                        .margin(Margin::symmetric(6, 4))
                        .vertical_align(Align::Center),
                );
                if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    open_requested = true;
                }
                if ui
                    .add(
                        Button::new(
                            RichText::new(self.t("View", "查看"))
                                .size(11.0)
                                .color(palette.text),
                        )
                        .fill(palette.surface_highest)
                        .stroke(Stroke::new(1.0, palette.outline_variant))
                        .corner_radius(8.0)
                        .min_size(Vec2::new(72.0, 30.0)),
                    )
                    .clicked()
                {
                    open_requested = true;
                }
                if ui
                    .add(
                        Button::new(
                            RichText::new(self.t("Browse", "浏览"))
                                .size(11.0)
                                .color(palette.text),
                        )
                        .fill(palette.surface_highest)
                        .stroke(Stroke::new(1.0, palette.outline_variant))
                        .corner_radius(8.0)
                        .min_size(Vec2::new(80.0, 30.0)),
                    )
                    .clicked()
                {
                    browse_requested = true;
                }
                if ui
                    .add_enabled(
                        selected_count > 0,
                        Button::new(
                            RichText::new(compress_label)
                                .size(11.0)
                                .strong()
                                .color(palette.on_primary),
                        )
                        .fill(palette.primary_strong)
                        .corner_radius(8.0)
                        .min_size(Vec2::new(108.0, 30.0)),
                    )
                    .clicked()
                {
                    compress_requested = true;
                }
                if ui
                    .add_enabled(
                        selected_count == 1,
                        Button::new(
                            RichText::new(self.t("Checksum", "校验"))
                                .size(11.0)
                                .strong()
                                .color(palette.on_primary),
                        )
                        .fill(palette.primary_strong)
                        .corner_radius(8.0)
                        .min_size(Vec2::new(96.0, 30.0)),
                    )
                    .clicked()
                {
                    checksum_requested = true;
                }
            });
        });

        self.file_manager_path_input = current_path;

        if open_requested {
            self.open_file_manager_typed_path();
        }
        if browse_requested {
            self.browse_file_manager_folder();
        }
        if compress_requested {
            self.compress_selected_file_manager_paths();
        }
        if checksum_requested {
            self.checksum_selected_file_manager_path();
        }
    }

    fn checksum_selected_file_manager_path(&mut self) {
        self.file_manager_selected_paths
            .retain(|path| path.exists());
        let selected: Vec<&PathBuf> = self
            .file_manager_selected_paths
            .iter()
            .filter(|p| p.is_file())
            .collect();
        if selected.len() != 1 {
            self.show_toast(
                FeedbackTone::Info,
                self.t(
                    "Select a single file to calculate checksums.",
                    "请选中单个文件以计算校验值。",
                ),
            );
            return;
        }
        match file_checksums(selected[0]) {
            Ok(results) => {
                self.checksum_results = results;
                self.show_checksum_dialog = true;
            }
            Err(e) => {
                self.show_toast(
                    FeedbackTone::Error,
                    self.language.format(
                        "Failed to calculate checksum: {error}",
                        "校验计算失败：{error}",
                        &[("{error}", format!("{e:#}"))],
                    ),
                );
            }
        }
    }

    fn draw_checksum_dialog(&mut self, ctx: &Context) {
        if !self.show_checksum_dialog {
            return;
        }
        let palette = self.palette();
        let mut close_requested = false;
        let modal_id = egui::Id::new("checksum-dialog");
        let backdrop_fade = anim_bool(ctx, modal_id.with("backdrop"), true, ANIM_NORMAL);
        let modal = egui::Modal::new(modal_id)
            .backdrop_color(Color32::from_black_alpha(
                ((if palette.is_dark { 160 } else { 96 }) as f32 * backdrop_fade) as u8,
            ))
            .frame(
                Frame::default()
                    .fill(palette.panel_fill)
                    .stroke(Stroke::new(1.0, palette.panel_stroke))
                    .corner_radius(18.0)
                    .inner_margin(Margin::same(18)),
            );

        let results = self.checksum_results.clone();
        let response = modal.show(ctx, |ui| {
            ui.set_min_width(520.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.t("Checksum Results", "校验结果"))
                            .size(16.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let copy_label = self.t("Copy All", "复制全部");
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(copy_label)
                                        .size(11.0)
                                        .color(palette.primary),
                                )
                                .fill(palette.surface_high)
                                .stroke(Stroke::new(1.0, palette.outline_variant))
                                .corner_radius(8.0)
                                .min_size(Vec2::new(96.0, 32.0)),
                            )
                            .clicked()
                        {
                            let text: String = results
                                .iter()
                                .map(|r| {
                                    format!(
                                        "{}  {}  ({} bytes, {:?})",
                                        r.algorithm.label(),
                                        r.hex_digest,
                                        r.file_size,
                                        r.elapsed,
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            ui.ctx().copy_text(text);
                        }
                    });
                });
                ui.add_space(10.0);
                thin_separator(ui, palette.outline_variant);
                ui.add_space(12.0);

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(280.0)
                    .show(ui, |ui| {
                        for result in &results {
                            let card = Frame::default()
                                .fill(palette.surface_low)
                                .stroke(Stroke::new(1.0, palette.outline_variant))
                                .corner_radius(10.0)
                                .inner_margin(Margin::same(12));
                            card.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(result.algorithm.label())
                                            .size(13.0)
                                            .strong()
                                            .color(palette.text),
                                    );
                                    ui.with_layout(
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            if ui
                                                .add(
                                                    Button::new(
                                                        RichText::new(
                                                            self.t("Copy", "复制"),
                                                        )
                                                        .size(11.0)
                                                        .color(palette.primary),
                                                    )
                                                    .fill(palette.surface_high)
                                                    .stroke(Stroke::new(
                                                        1.0,
                                                        palette.outline_variant,
                                                    ))
                                                    .corner_radius(6.0)
                                                    .min_size(Vec2::new(64.0, 26.0)),
                                                )
                                                .clicked()
                                            {
                                                ui.ctx()
                                                    .copy_text(result.hex_digest.clone());
                                            }
                                        },
                                    );
                                });
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(&result.hex_digest)
                                        .size(18.0)
                                        .strong()
                                        .color(palette.text)
                                        .monospace(),
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} bytes  |  {:?}",
                                            result.file_size, result.elapsed,
                                        ))
                                        .size(11.0)
                                        .color(palette.text_secondary),
                                    );
                                });
                            });
                            ui.add_space(8.0);
                        }
                    });

                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            Button::new(self.t("Close", "关闭"))
                                .min_size(Vec2::new(96.0, 36.0)),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });

        if close_requested || response.should_close() {
            self.show_checksum_dialog = false;
        }
    }

    fn draw_file_manager_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let parent_dir = self
            .file_manager_current_dir
            .parent()
            .map(Path::to_path_buf);
        let selected_count = self
            .file_manager_selected_paths
            .iter()
            .filter(|path| path.exists())
            .count();
        let entries = match self.collect_file_manager_entries() {
            Ok(entries) => entries,
            Err(error) => {
                glass_panel(palette.panel_fill, palette).show(ui, |ui| {
                    ui.label(
                        RichText::new(self.t("File Contents", "文件内容"))
                            .size(16.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.add_space(10.0);
                    thin_separator(ui, palette.outline_variant);
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new(self.language.format(
                            "Failed to open path: {error}",
                            "打开路径失败：{error}",
                            &[("{error}", format!("{error:#}"))],
                        ))
                        .size(13.0)
                        .color(palette.error),
                    );
                });
                return;
            }
        };
        let visible_paths = entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        let all_visible_selected = !visible_paths.is_empty()
            && visible_paths
                .iter()
                .all(|path| self.file_manager_selected_paths.contains(path));
        let mut select_all_requested = None;
        let mut open_parent_requested = false;
        let mut view_target = None;
        let mut extract_target = None;
        let item_count = entries.len();

        glass_panel(palette.panel_fill, palette).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.t("File Contents", "文件内容"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                let controls_width = ui.available_width().max(0.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(controls_width, 24.0),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        status_badge(
                            ui,
                            self.language.format(
                                "{count} Items",
                                "{count} 项",
                                &[("{count}", item_count.to_string())],
                            ),
                            palette.badge_fill,
                            palette.badge_text,
                        );
                        ui.add_space(8.0);
                        if ui
                            .add_enabled(
                                selected_count > 0,
                                task_action_button(
                                    self.t("View Selected Files", "查看已选中文件"),
                                    false,
                                    palette,
                                ),
                            )
                            .clicked()
                        {
                            self.show_selected_file_manager_paths = true;
                        }
                    },
                );
            });
            ui.add_space(10.0);
            thin_separator(ui, palette.outline_variant);
            ui.add_space(12.0);

            let min_table_width = FILE_MANAGER_SELECT_COLUMN_WIDTH
                + FILE_MANAGER_NAME_COLUMN_MIN_WIDTH
                + FILE_MANAGER_SIZE_COLUMN_WIDTH
                + FILE_MANAGER_TYPE_COLUMN_WIDTH
                + FILE_MANAGER_ACTION_COLUMN_WIDTH
                + FILE_MANAGER_GRID_SPACING_X * 4.0;
            let table_width = ui.available_width().max(min_table_width);
            let name_column_width =
                FILE_MANAGER_NAME_COLUMN_MIN_WIDTH + (table_width - min_table_width);

            egui::Grid::new("file-manager-grid")
                .num_columns(5)
                .min_row_height(30.0)
                .spacing(Vec2::new(
                    FILE_MANAGER_GRID_SPACING_X,
                    FILE_MANAGER_GRID_SPACING_Y,
                ))
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(FILE_MANAGER_SELECT_COLUMN_WIDTH, 22.0),
                        Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            let mut checked = all_visible_selected;
                            if ui.checkbox(&mut checked, "").clicked() {
                                select_all_requested = Some(!all_visible_selected);
                            }
                        },
                    );
                    padded_header_cell(
                        ui,
                        self.t("Name", "名称"),
                        name_column_width,
                        FILE_MANAGER_NAME_COLUMN_PADDING,
                        palette,
                    );
                    header_cell(
                        ui,
                        self.t("Size", "大小"),
                        FILE_MANAGER_SIZE_COLUMN_WIDTH,
                        palette,
                    );
                    centered_header_cell(
                        ui,
                        self.t("Type", "类型"),
                        FILE_MANAGER_TYPE_COLUMN_WIDTH,
                        palette,
                    );
                    centered_header_cell(
                        ui,
                        self.t("Actions", "操作"),
                        FILE_MANAGER_ACTION_COLUMN_WIDTH,
                        palette,
                    );
                    ui.end_row();

                    if parent_dir.is_some() {
                        ui.allocate_ui_with_layout(
                            Vec2::new(FILE_MANAGER_SELECT_COLUMN_WIDTH, 22.0),
                            Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |_| {},
                        );
                        padded_text_cell(
                            ui,
                            name_column_width,
                            22.0,
                            name_column_width - FILE_MANAGER_NAME_COLUMN_PADDING,
                            FILE_MANAGER_NAME_COLUMN_PADDING,
                            RichText::new("..").size(13.0).strong().color(palette.text),
                            false,
                        );
                        ui.add_sized(
                            [FILE_MANAGER_SIZE_COLUMN_WIDTH, 22.0],
                            egui::Label::new(
                                RichText::new("-").size(13.0).color(palette.text_secondary),
                            ),
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(FILE_MANAGER_TYPE_COLUMN_WIDTH, 22.0),
                            Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| type_badge(ui, "DIR", palette),
                        );
                        let response = ui.add_sized(
                            [FILE_MANAGER_ACTION_COLUMN_WIDTH, 24.0],
                            task_action_button(
                                self.t("Back to Parent Folder", "返回上一层目录"),
                                false,
                                palette,
                            ),
                        );
                        if response.clicked() {
                            open_parent_requested = true;
                        }
                        ui.end_row();
                    }

                    for entry in &entries {
                        let mut selected = self.file_manager_selected_paths.contains(&entry.path);
                        ui.allocate_ui_with_layout(
                            Vec2::new(FILE_MANAGER_SELECT_COLUMN_WIDTH, 22.0),
                            Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                if ui.checkbox(&mut selected, "").clicked() {
                                    if selected {
                                        self.file_manager_selected_paths.insert(entry.path.clone());
                                    } else {
                                        self.file_manager_selected_paths.remove(&entry.path);
                                    }
                                }
                            },
                        );
                        padded_text_cell(
                            ui,
                            name_column_width,
                            22.0,
                            name_column_width - FILE_MANAGER_NAME_COLUMN_PADDING,
                            FILE_MANAGER_NAME_COLUMN_PADDING,
                            RichText::new(&entry.name).size(13.0).color(palette.text),
                            false,
                        );
                        ui.add_sized(
                            [FILE_MANAGER_SIZE_COLUMN_WIDTH, 22.0],
                            egui::Label::new(
                                RichText::new(&entry.size)
                                    .size(13.0)
                                    .color(palette.text_secondary),
                            ),
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(FILE_MANAGER_TYPE_COLUMN_WIDTH, 22.0),
                            Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| type_badge(ui, &entry.kind, palette),
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(FILE_MANAGER_ACTION_COLUMN_WIDTH, 22.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if ui
                                    .add_sized(
                                        [64.0, 22.0],
                                        Button::new(
                                            RichText::new(self.t("View", "查看"))
                                                .size(11.0)
                                                .color(palette.text_muted),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE),
                                    )
                                    .clicked()
                                {
                                    if entry.show_extract_action {
                                        extract_target = Some(entry.path.clone());
                                    } else {
                                        view_target =
                                            Some((entry.path.clone(), entry.is_dir));
                                    }
                                }
                                if entry.show_extract_action
                                    && ui
                                        .add_sized(
                                            [72.0, 24.0],
                                            task_action_button(
                                                self.t("Extract", "解压"),
                                                false,
                                                palette,
                                            ),
                                        )
                                        .clicked()
                                {
                                    extract_target = Some(entry.path.clone());
                                }
                            },
                        );
                        ui.end_row();
                    }
                });

            if entries.is_empty() {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(self.t("The current folder is empty.", "当前文件夹为空。"))
                        .size(13.0)
                        .color(palette.text_secondary),
                );
            }
        });

        if let Some(select_all) = select_all_requested {
            if select_all {
                for path in &visible_paths {
                    self.file_manager_selected_paths.insert(path.clone());
                }
            } else {
                for path in &visible_paths {
                    self.file_manager_selected_paths.remove(path);
                }
            }
        }
        if open_parent_requested {
            self.open_file_manager_parent_directory();
        }
        if let Some((path, is_dir)) = view_target {
            self.view_file_manager_entry(&path, is_dir);
        }
        if let Some(path) = extract_target {
            self.extract_file_manager_zip(&path);
        }
    }

    fn draw_task_queue_page(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();

        paint_background(ui, palette);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                self.draw_task_queue_panel(ui);
            });
    }

    fn draw_current_page(&mut self, ui: &mut egui::Ui) {
        match self.side_nav {
            SideNavItem::Compress => {
                self.workspace_mode = WorkspaceMode::Compress;
                self.draw_workspace_dashboard(ui);
            }
            SideNavItem::Extract => {
                self.workspace_mode = WorkspaceMode::Extract;
                self.draw_workspace_dashboard(ui);
            }
            SideNavItem::FileManager => self.draw_file_manager_page(ui),
            SideNavItem::Tasks => self.draw_task_queue_page(ui),
            SideNavItem::Logs => self.draw_logs_page(ui),
            SideNavItem::Settings => self.draw_settings_page(ui),
        }
    }

    fn draw_workspace_action(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let (title, glyph) = match self.workspace_mode {
            WorkspaceMode::Compress => (self.t("Add Files to Compress", "添加待压缩文件"), "+"),
            WorkspaceMode::Extract => (self.t("Drop Archive to Extract", "拖入压缩包以解压"), "U"),
        };

        let clicked = action_card(
            ui,
            title,
            glyph,
            palette.primary_soft_fill,
            Stroke::new(1.5, palette.primary_soft_stroke),
            true,
            self.workspace_action_busy(),
            palette,
        );

        if clicked {
            match self.workspace_mode {
                WorkspaceMode::Compress => self.open_compress_source_picker(),
                WorkspaceMode::Extract => {
                    let ctx = ui.ctx().clone();
                    self.schedule_dialog_action(&ctx, PendingDialogAction::BrowseArchive)
                }
            }
        }
    }

    fn draw_path_selectors(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let is_compress = self.workspace_mode == WorkspaceMode::Compress;
        let source_label = if is_compress {
            self.t("Source files or folders", "待压缩文件或文件夹")
        } else {
            self.t("Source archive", "源压缩包")
        };
        let output_label = if is_compress {
            self.t("Archive output path", "压缩包输出路径")
        } else {
            self.t("Output directory", "输出目录")
        };
        let browse_label = self.t("Browse", "浏览");
        let start_label = if is_compress {
            self.t("Add compression task", "加入压缩队列")
        } else {
            self.t("Add extraction task", "加入解压队列")
        };

        let mut browse_source_clicked = false;
        let mut browse_output_clicked = false;
        let mut compress_source_display = self.compress_sources_display();

        compact_glass_panel(palette.panel_fill, palette).show(ui, |ui| {
            let button_width = 154.0;
            let field_gap = 12.0;
            let extra_test_button = if is_compress { 0.0 } else { button_width + field_gap };
            let field_width =
                ((ui.available_width() - button_width - extra_test_button - field_gap * 2.0) / 2.0).max(160.0);

            if ui.available_width() >= 900.0 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = field_gap;

                    ui.allocate_ui_with_layout(
                        Vec2::new(field_width, 42.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            browse_source_clicked = if is_compress {
                                draw_path_row(
                                    ui,
                                    source_label,
                                    &mut compress_source_display,
                                    browse_label,
                                    palette,
                                )
                            } else {
                                draw_path_row(
                                    ui,
                                    source_label,
                                    &mut self.archive_path,
                                    browse_label,
                                    palette,
                                )
                            };
                        },
                    );

                    ui.allocate_ui_with_layout(
                        Vec2::new(field_width, 42.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            browse_output_clicked = if is_compress {
                                draw_path_row(
                                    ui,
                                    output_label,
                                    &mut self.compress_output_path,
                                    browse_label,
                                    palette,
                                )
                            } else {
                                draw_path_row(
                                    ui,
                                    output_label,
                                    &mut self.output_dir,
                                    browse_label,
                                    palette,
                                )
                            };
                        },
                    );

                    ui.allocate_ui_with_layout(
                        Vec2::new(button_width, 42.0),
                        Layout::bottom_up(Align::Center),
                        |ui| {
                            let button = Button::new(
                                RichText::new(format!("+ {start_label}"))
                                    .strong()
                                    .color(palette.on_primary),
                            )
                            .fill(palette.primary_strong)
                            .corner_radius(10.0)
                            .min_size(Vec2::new(button_width, 30.0));

                            if ui.add(button).clicked() {
                                if is_compress {
                                    self.compress_archive();
                                } else {
                                    self.extract_archive();
                                }
                            }
                        },
                    );

                    if !is_compress {
                        ui.allocate_ui_with_layout(
                            Vec2::new(button_width, 42.0),
                            Layout::bottom_up(Align::Center),
                            |ui| {
                                let test_label = self.t("Test", "测试");
                                let button = Button::new(
                                    RichText::new(test_label)
                                        .size(11.0)
                                        .color(palette.text),
                                )
                                .fill(palette.surface_highest)
                                .stroke(Stroke::new(1.0, palette.outline_variant))
                                .corner_radius(10.0)
                                .min_size(Vec2::new(button_width, 30.0));

                                if ui.add(button).clicked() {
                                    self.test_current_archive();
                                }
                            },
                        );
                    }
                });
            } else {
                browse_source_clicked = if is_compress {
                    draw_path_row(
                        ui,
                        source_label,
                        &mut compress_source_display,
                        browse_label,
                        palette,
                    )
                } else {
                    draw_path_row(
                        ui,
                        source_label,
                        &mut self.archive_path,
                        browse_label,
                        palette,
                    )
                };
                ui.add_space(6.0);
                browse_output_clicked = if is_compress {
                    draw_path_row(
                        ui,
                        output_label,
                        &mut self.compress_output_path,
                        browse_label,
                        palette,
                    )
                } else {
                    draw_path_row(
                        ui,
                        output_label,
                        &mut self.output_dir,
                        browse_label,
                        palette,
                    )
                };
                ui.add_space(8.0);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let button = Button::new(
                        RichText::new(format!("+ {start_label}"))
                            .strong()
                            .color(palette.on_primary),
                    )
                    .fill(palette.primary_strong)
                    .corner_radius(10.0)
                    .min_size(Vec2::new(160.0, 32.0));

                    if ui.add(button).clicked() {
                        if is_compress {
                            self.compress_archive();
                        } else {
                            self.extract_archive();
                        }
                    }

                    if !is_compress {
                        ui.add_space(6.0);
                        let test_label = self.t("Test", "测试");
                        let test_button = Button::new(
                            RichText::new(test_label)
                                .size(11.0)
                                .color(palette.text),
                        )
                        .fill(palette.surface_highest)
                        .stroke(Stroke::new(1.0, palette.outline_variant))
                        .corner_radius(10.0)
                        .min_size(Vec2::new(160.0, 32.0));

                        if ui.add(test_button).clicked() {
                            self.test_current_archive();
                        }
                    }
                });
            }
        });

        if browse_source_clicked {
            if is_compress {
                self.open_compress_source_picker();
            } else {
                let ctx = ui.ctx().clone();
                self.schedule_dialog_action(&ctx, PendingDialogAction::BrowseArchive);
            }
        }
        if browse_output_clicked {
            let ctx = ui.ctx().clone();
            if is_compress {
                self.schedule_dialog_action(&ctx, PendingDialogAction::BrowseCompressOutputPath);
            } else {
                self.schedule_dialog_action(&ctx, PendingDialogAction::BrowseOutputDir);
            }
        }
    }

    fn apply_compression_format(&mut self, format: CompressionFormat) {
        if self.compression_options.format == format {
            return;
        }

        let previous_format = self.compression_options.format;
        let previous_suggested_output =
            suggested_archive_output_path(&self.compress_sources, previous_format)
                .map(|path| path.display().to_string());
        let current_output = self.compress_output_path.trim().to_string();

        self.compression_options.format = format;
        if !format.supports_encrypt_file_names() {
            self.compression_options.encrypt_file_names = false;
        }
        if self.compress_output_path.trim().is_empty()
            || previous_suggested_output
                .as_ref()
                .is_some_and(|value| value == &current_output)
        {
            if let Some(suggested_output) =
                suggested_archive_output_path(&self.compress_sources, format)
            {
                self.compress_output_path = suggested_output.display().to_string();
            }
        }

        self.push_log(self.language.format(
            "Compression format switched to {format}.",
            "压缩格式已切换为 {format}。",
            &[(
                "{format}",
                compression_format_label(format, self.language).to_string(),
            )],
        ));
    }

    fn compression_method_description_for(&self, options: &CompressionOptions) -> String {
        match options.format {
            CompressionFormat::SevenZip => self.t("LZMA2 stream", "LZMA2 压缩流").to_string(),
            CompressionFormat::Zip => match options.zip_method {
                ZipCompressionMethod::Stored => self
                    .t("Store (direct write)", "仅存储（直接写入）")
                    .to_string(),
                ZipCompressionMethod::Deflate if options.thread_count > 1 => self
                    .t(
                        "Deflate (parallel file workers)",
                        "Deflate（并行文件工作线程）",
                    )
                    .to_string(),
                ZipCompressionMethod::Bzip2 if options.thread_count > 1 => self
                    .t("BZip2 (parallel file workers)", "BZip2（并行文件工作线程）")
                    .to_string(),
                ZipCompressionMethod::Zstd if options.thread_count > 1 => self
                    .t(
                        "Zstandard (parallel file workers)",
                        "Zstandard（并行文件工作线程）",
                    )
                    .to_string(),
                ZipCompressionMethod::Xz if options.thread_count > 1 => self
                    .t("XZ (limited parallel workers)", "XZ（限量并行工作线程）")
                    .to_string(),
                _ => zip_method_label(options.zip_method, self.language).to_string(),
            },
            CompressionFormat::Tar => self.t("Tar stream", "TAR 归档").to_string(),
            CompressionFormat::TarGz => self.t("GZip stream", "GZip 压缩流").to_string(),
            CompressionFormat::TarBz2 => self.t("BZip2 stream", "BZip2 压缩流").to_string(),
            CompressionFormat::TarXz => self.t("XZ stream", "XZ 压缩流").to_string(),
            CompressionFormat::Gz => self.t("GZip stream", "GZip 压缩流").to_string(),
            CompressionFormat::Bz2 => self.t("BZip2 stream", "BZip2 压缩流").to_string(),
            CompressionFormat::Xz => self.t("XZ stream", "XZ 压缩流").to_string(),
            CompressionFormat::TarZst => {
                self.t("Zstd stream", "Zstd 压缩流").to_string()
            }
            CompressionFormat::Zst => self.t("Zstd stream", "Zstd 压缩流").to_string(),
            CompressionFormat::TarLz4 => {
                self.t("LZ4 stream", "LZ4 压缩流").to_string()
            }
            CompressionFormat::Lz4 => self.t("LZ4 stream", "LZ4 压缩流").to_string(),
        }
    }

    fn compression_dictionary_text_for(&self, options: &CompressionOptions) -> String {
        match (options.format, options.zip_method) {
            (CompressionFormat::SevenZip, _) => match options.level {
                CompressionLevel::Fastest => {
                    self.t("LZMA2 preset (~1 MB)", "LZMA2 预设（约 1 MB）")
                }
                CompressionLevel::Fast => self.t("LZMA2 preset (~4 MB)", "LZMA2 预设（约 4 MB）"),
                CompressionLevel::Normal => self.t("LZMA2 preset (~8 MB)", "LZMA2 预设（约 8 MB）"),
                CompressionLevel::Maximum => {
                    self.t("LZMA2 preset (~16 MB)", "LZMA2 预设（约 16 MB）")
                }
                CompressionLevel::Ultra => {
                    self.t("LZMA2 preset (~32 MB)", "LZMA2 预设（约 32 MB）")
                }
            }
            .to_string(),
            (CompressionFormat::Zip, ZipCompressionMethod::Deflate) => self
                .t("Fixed by Deflate (32 KB)", "由 Deflate 固定（32 KB）")
                .to_string(),
            (CompressionFormat::Zip, ZipCompressionMethod::Stored) => self
                .t("Not used for Store mode", "Store 模式不使用字典")
                .to_string(),
            (CompressionFormat::Zip, _) => self
                .t(
                    "Managed internally by the selected codec",
                    "由所选编码器内部自动管理",
                )
                .to_string(),
            _ => self
                .t(
                    "Not exposed for TAR-based formats",
                    "TAR 系列格式当前不暴露此参数",
                )
                .to_string(),
        }
    }

    fn compression_word_size_text_for(&self, options: &CompressionOptions) -> String {
        match options.format {
            CompressionFormat::Zip if options.zip_method == ZipCompressionMethod::Deflate => {
                self.t("Codec default", "编码器默认值").to_string()
            }
            CompressionFormat::SevenZip => self
                .t(
                    "Currently uses the native LZMA2 preset",
                    "当前跟随原生 LZMA2 预设",
                )
                .to_string(),
            _ => self
                .t(
                    "Not configurable in the current backend",
                    "当前后端暂不支持配置",
                )
                .to_string(),
        }
    }

    fn compression_solid_text_for(&self, options: &CompressionOptions) -> String {
        if options.format == CompressionFormat::SevenZip {
            return self
                .t(
                    "Adaptive solid batching for compressible files",
                    "对可压缩文件使用自适应固实分批",
                )
                .to_string();
        }

        self.t(
            "Solid block mode is not available for the current formats",
            "当前支持的格式不提供固实块模式",
        )
        .to_string()
    }

    fn compression_memory_text_for(&self, options: &CompressionOptions) -> String {
        if matches!(
            options.format,
            CompressionFormat::SevenZip | CompressionFormat::TarXz | CompressionFormat::Xz
        ) {
            let thread_count = options.thread_count.max(1);
            return if thread_count == 1 {
                self.t("High (~64 MB+)", "高（约 64 MB+）").to_string()
            } else {
                self.language.format(
                    "High (~64 MB+ x {thread_count} threads)",
                    "高（约 64 MB+ x {thread_count} 个线程）",
                    &[("{thread_count}", thread_count.to_string())],
                )
            };
        }

        if options.format == CompressionFormat::Zip {
            let thread_count = options.thread_count.max(1);
            let memory = match options.zip_method {
                ZipCompressionMethod::Stored => self
                    .t(
                        "Very low (< 8 MB, direct write)",
                        "很低（< 8 MB，直接写入）",
                    )
                    .to_string(),
                ZipCompressionMethod::Deflate if thread_count == 1 => {
                    self.t("Low (< 16 MB)", "低（< 16 MB）").to_string()
                }
                ZipCompressionMethod::Deflate => self.language.format(
                    "Low to medium (< 16 MB x {thread_count} workers)",
                    "低到中（< 16 MB x {thread_count} 个工作线程）",
                    &[("{thread_count}", thread_count.to_string())],
                ),
                ZipCompressionMethod::Bzip2 => self.language.format(
                    "Medium (~32 MB x up to {thread_count} workers)",
                    "中（约 32 MB x 最多 {thread_count} 个工作线程）",
                    &[("{thread_count}", thread_count.min(8).to_string())],
                ),
                ZipCompressionMethod::Zstd => self.language.format(
                    "Medium (32-64 MB x up to {thread_count} workers)",
                    "中（32-64 MB x 最多 {thread_count} 个工作线程）",
                    &[("{thread_count}", thread_count.min(16).to_string())],
                ),
                ZipCompressionMethod::Xz => self.language.format(
                    "High (64 MB+ x up to {thread_count} workers)",
                    "高（64 MB+ x 最多 {thread_count} 个工作线程）",
                    &[("{thread_count}", thread_count.min(4).to_string())],
                ),
            };
            return memory;
        }

        let memory = match (options.format, options.zip_method, options.level) {
            (CompressionFormat::TarGz, _, _) => self.t("Low (< 16 MB)", "低（< 16 MB）"),
            (CompressionFormat::TarBz2, _, _) => self.t("Medium (~32 MB)", "中（约 32 MB）"),
            (CompressionFormat::Tar, _, _) => self.t("Very low (< 8 MB)", "很低（< 8 MB）"),
            (CompressionFormat::TarXz, _, _) => self.t("High (~64 MB+)", "高（约 64 MB+）"),
            (CompressionFormat::SevenZip, _, _) => self.t("High (~64 MB+)", "高（约 64 MB+）"),
            (CompressionFormat::Gz, _, _) => self.t("Low (< 16 MB)", "低（< 16 MB）"),
            (CompressionFormat::Bz2, _, _) => self.t("Medium (~32 MB)", "中（约 32 MB）"),
            (CompressionFormat::Xz, _, _) => self.t("High (~64 MB+)", "高（约 64 MB+）"),
            (CompressionFormat::Zst, _, _) => self.t("Medium (~32 MB)", "中（约 32 MB）"),
            (CompressionFormat::TarZst, _, _) => {
                self.t("Medium (~32 MB)", "中（约 32 MB）")
            }
            (CompressionFormat::Lz4, _, _) => self.t("Very low (< 8 MB)", "很低（< 8 MB）"),
            (CompressionFormat::TarLz4, _, _) => {
                self.t("Very low (< 8 MB)", "很低（< 8 MB）")
            }
            (CompressionFormat::Zip, _, _) => unreachable!("ZIP handled above"),
        };
        memory.to_string()
    }

    fn draw_compression_options_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let mut selected_format = self.compression_options.format;
        let mut selected_level = self.compression_options.level;
        let mut selected_zip_method = self.compression_options.zip_method;
        let mut selected_password = self
            .compression_options
            .password
            .clone()
            .unwrap_or_default();
        let mut selected_password_confirm = self.compression_password_confirm.clone();
        let mut selected_split_volume_size_input = self.compression_split_volume_size_input.clone();
        let mut selected_encrypt_file_names = self.compression_options.encrypt_file_names;
        let detected_threads = default_compression_thread_count();
        let mut selected_thread_count = self
            .compression_options
            .thread_count
            .clamp(1, detected_threads);
        let selected_split_volume_size =
            parse_volume_size_spec(&selected_split_volume_size_input).ok();

        glass_panel(palette.panel_fill, palette).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.t("Compression Options", "压缩选项"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    status_badge(
                        ui,
                        compression_format_label(self.compression_options.format, self.language)
                            .to_string(),
                        palette.primary_soft_fill,
                        palette.nav_active_text,
                    );
                });
            });
            ui.add_space(8.0);
            thin_separator(ui, palette.outline_variant);
            ui.add_space(10.0);

            // Presets row
            let preset_names = crate::settings::list_preset_names();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.t("Preset:", "预设："))
                        .size(12.0)
                        .color(palette.text_secondary),
                );
                let preset_label = self.t("Select preset...", "选择预设...");
                egui::ComboBox::from_id_salt("compression-preset")
                    .width(180.0)
                    .selected_text(preset_label)
                    .show_ui(ui, |ui| {
                        for name in &preset_names {
                            if ui.selectable_label(false, name).clicked() {
                                if let Some(value) = crate::settings::load_preset(name) {
                                    if let Ok((fmt, lvl, meth, thr, enc)) =
                                        crate::settings::decode_preset_value(&value)
                                    {
                                        selected_format = fmt;
                                        selected_level = lvl;
                                        selected_zip_method = meth;
                                        selected_thread_count = thr.clamp(1, detected_threads);
                                        selected_encrypt_file_names = enc;
                                    }
                                }
                            }
                        }
                    });

                ui.separator();

                if ui
                    .add(
                        Button::new(
                            RichText::new(self.t("Save", "保存"))
                                .size(11.0)
                                .color(palette.primary),
                        )
                        .fill(palette.surface_highest)
                        .stroke(Stroke::new(1.0, palette.outline_variant))
                        .corner_radius(6.0)
                        .min_size(Vec2::new(60.0, 24.0)),
                    )
                    .clicked()
                {
                    self.preset_save_name.clear();
                    self.show_save_preset_dialog = true;
                }

                if ui
                    .add(
                        Button::new(
                            RichText::new(self.t("Manage", "管理"))
                                .size(11.0)
                                .color(palette.text_secondary),
                        )
                        .fill(palette.surface_low)
                        .stroke(Stroke::new(1.0, palette.outline_variant))
                        .corner_radius(6.0)
                        .min_size(Vec2::new(64.0, 24.0)),
                    )
                    .clicked()
                {
                    self.show_manage_presets_dialog = true;
                }
            });
            ui.add_space(10.0);

            let preview_options = CompressionOptions {
                format: selected_format,
                level: selected_level,
                zip_method: selected_zip_method,
                thread_count: selected_thread_count,
                password: (!selected_password.is_empty()).then_some(selected_password.clone()),
                encrypt_file_names: selected_encrypt_file_names,
                split_volume_size: selected_split_volume_size,
                sfx: false,
            };
            let encryption_supported = selected_format.supports_password_encryption();
            if ui.available_width() >= 920.0 {
                ui.columns(2, |columns| {
                    columns[0].vertical(|ui| {
                        compression_option_combo(
                            ui,
                            self.t("Archive Format", "压缩格式"),
                            compression_format_label(selected_format, self.language),
                            palette,
                            |ui| {
                                for format in [
                                    CompressionFormat::SevenZip,
                                    CompressionFormat::Zip,
                                    CompressionFormat::Tar,
                                    CompressionFormat::TarGz,
                                    CompressionFormat::TarBz2,
                                    CompressionFormat::TarXz,
                                    CompressionFormat::Gz,
                                    CompressionFormat::Bz2,
                                    CompressionFormat::Xz,
                                    CompressionFormat::Zst,
                                    CompressionFormat::TarZst,
                                    CompressionFormat::Lz4,
                                    CompressionFormat::TarLz4,
                                ] {
                                    ui.selectable_value(
                                        &mut selected_format,
                                        format,
                                        compression_format_label(format, self.language),
                                    );
                                }
                            },
                        );
                        ui.add_space(12.0);

                        if selected_format.supports_zip_method() {
                            compression_option_combo(
                                ui,
                                self.t("Compression Method", "压缩方法"),
                                zip_method_label(selected_zip_method, self.language),
                                palette,
                                |ui| {
                                    for method in [
                                        ZipCompressionMethod::Deflate,
                                        ZipCompressionMethod::Stored,
                                        ZipCompressionMethod::Bzip2,
                                        ZipCompressionMethod::Zstd,
                                        ZipCompressionMethod::Xz,
                                    ] {
                                        ui.selectable_value(
                                            &mut selected_zip_method,
                                            method,
                                            zip_method_label(method, self.language),
                                        );
                                    }
                                },
                            );
                        } else {
                            compression_option_static(
                                ui,
                                self.t("Compression Method", "压缩方法"),
                                &self.compression_method_description_for(&preview_options),
                                palette,
                            );
                        }
                        ui.add_space(12.0);

                        compression_option_drag_value(
                            ui,
                            self.t("Thread Count", "线程数"),
                            &mut selected_thread_count,
                            1..=detected_threads,
                            palette,
                        );
                        ui.add_space(12.0);

                        compression_option_static(
                            ui,
                            self.t("Solid Block Size", "固实数据大小"),
                            &self.compression_solid_text_for(&preview_options),
                            palette,
                        );
                    });

                    columns[1].vertical(|ui| {
                        compression_option_combo(
                            ui,
                            self.t("Compression Level", "压缩等级"),
                            compression_level_label(selected_level, self.language),
                            palette,
                            |ui| {
                                for level in [
                                    CompressionLevel::Fastest,
                                    CompressionLevel::Fast,
                                    CompressionLevel::Normal,
                                    CompressionLevel::Maximum,
                                    CompressionLevel::Ultra,
                                ] {
                                    ui.selectable_value(
                                        &mut selected_level,
                                        level,
                                        compression_level_label(level, self.language),
                                    );
                                }
                            },
                        );
                        ui.add_space(12.0);

                        compression_option_static(
                            ui,
                            self.t("Dictionary Size", "字典大小"),
                            &self.compression_dictionary_text_for(&preview_options),
                            palette,
                        );
                        ui.add_space(12.0);

                        compression_option_static(
                            ui,
                            self.t("Word Size", "单词大小"),
                            &self.compression_word_size_text_for(&preview_options),
                            palette,
                        );
                        ui.add_space(12.0);

                        compression_option_static(
                            ui,
                            self.t("Compression Memory", "压缩所需内存"),
                            &self.compression_memory_text_for(&preview_options),
                            palette,
                        );
                        ui.add_space(12.0);

                        compression_option_split_volume_input(
                            ui,
                            self.t("Split Volume Size", "分卷大小"),
                            &mut selected_split_volume_size_input,
                            self.t(
                                "Leave empty, or use 10M / 700M / 4G",
                                "留空表示不分卷，或输入 10M / 700M / 4G",
                            ),
                            self.language,
                            palette,
                        );
                        ui.add_space(14.0);

                        egui::Grid::new("compression-encryption-grid-inline")
                            .num_columns(2)
                            .spacing(Vec2::new(20.0, 12.0))
                            .show(ui, |ui| {
                                compression_option_password(
                                    ui,
                                    self.t("Enter Password", "输入密码"),
                                    &mut selected_password,
                                    encryption_supported,
                                    palette,
                                );
                                compression_option_password(
                                    ui,
                                    self.t("Reenter Password", "再次输入密码"),
                                    &mut selected_password_confirm,
                                    encryption_supported,
                                    palette,
                                );
                                ui.end_row();

                                compression_option_static(
                                    ui,
                                    self.t("Encryption Method", "加密方法"),
                                    if encryption_supported {
                                        "AES-256"
                                    } else {
                                        self.t("Not available for this format", "当前格式不支持")
                                    },
                                    palette,
                                );
                                compression_option_checkbox(
                                    ui,
                                    self.t("Encrypt File Names", "加密文件名"),
                                    self.t(
                                        "Hide archive contents in 7Z headers",
                                        "在 7Z 头部中隐藏文件内容列表",
                                    ),
                                    &mut selected_encrypt_file_names,
                                    selected_format.supports_encrypt_file_names()
                                        && !selected_password.is_empty(),
                                    palette,
                                );
                                ui.end_row();
                            });

                        let encryption_note = if !encryption_supported {
                            self.t(
                                "Password encryption is currently available only for ZIP and 7Z archives.",
                                "当前只有 ZIP 和 7Z 格式支持密码加密。",
                            )
                        } else if selected_format == CompressionFormat::Zip {
                            self.t(
                                "ZIP only encrypts file data. Archive contents can still be listed by name, but extracting file data requires the password.",
                                "ZIP 只加密文件数据。归档仍然可以看到文件名，但没有密码不能解压出文件内容。",
                            )
                        } else if selected_password.is_empty() {
                            self.t(
                                "Enter and confirm a password to enable archive encryption.",
                                "输入并确认密码后才会启用归档加密。",
                            )
                        } else {
                            self.t(
                                "7Z can also encrypt file names when the checkbox is enabled.",
                                "勾选后 7Z 还可以同时加密文件名。",
                            )
                        };
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(encryption_note)
                                .size(11.0)
                                .color(palette.text_secondary),
                        );
                    });
                });
            } else {
                egui::Grid::new("compression-options-grid")
                    .num_columns(2)
                    .spacing(Vec2::new(20.0, 12.0))
                    .show(ui, |ui| {
                        compression_option_combo(
                            ui,
                            self.t("Archive Format", "压缩格式"),
                            compression_format_label(selected_format, self.language),
                            palette,
                            |ui| {
                                for format in [
                                    CompressionFormat::SevenZip,
                                    CompressionFormat::Zip,
                                    CompressionFormat::Tar,
                                    CompressionFormat::TarGz,
                                    CompressionFormat::TarBz2,
                                    CompressionFormat::TarXz,
                                    CompressionFormat::TarZst,
                                    CompressionFormat::Gz,
                                    CompressionFormat::Bz2,
                                    CompressionFormat::Xz,
                                    CompressionFormat::Zst,
                                    CompressionFormat::TarLz4,
                                    CompressionFormat::Lz4,
                                ] {
                                    ui.selectable_value(
                                        &mut selected_format,
                                        format,
                                        compression_format_label(format, self.language),
                                    );
                                }
                            },
                        );

                        compression_option_combo(
                            ui,
                            self.t("Compression Level", "压缩等级"),
                            compression_level_label(selected_level, self.language),
                            palette,
                            |ui| {
                                for level in [
                                    CompressionLevel::Fastest,
                                    CompressionLevel::Fast,
                                    CompressionLevel::Normal,
                                    CompressionLevel::Maximum,
                                    CompressionLevel::Ultra,
                                ] {
                                    ui.selectable_value(
                                        &mut selected_level,
                                        level,
                                        compression_level_label(level, self.language),
                                    );
                                }
                            },
                        );
                        ui.end_row();

                        if selected_format.supports_zip_method() {
                            compression_option_combo(
                                ui,
                                self.t("Compression Method", "压缩方法"),
                                zip_method_label(selected_zip_method, self.language),
                                palette,
                                |ui| {
                                    for method in [
                                        ZipCompressionMethod::Deflate,
                                        ZipCompressionMethod::Stored,
                                        ZipCompressionMethod::Bzip2,
                                        ZipCompressionMethod::Zstd,
                                        ZipCompressionMethod::Xz,
                                    ] {
                                        ui.selectable_value(
                                            &mut selected_zip_method,
                                            method,
                                            zip_method_label(method, self.language),
                                        );
                                    }
                                },
                            );
                        } else {
                            compression_option_static(
                                ui,
                                self.t("Compression Method", "压缩方法"),
                                &self.compression_method_description_for(&preview_options),
                                palette,
                            );
                        }

                        compression_option_static(
                            ui,
                            self.t("Dictionary Size", "字典大小"),
                            &self.compression_dictionary_text_for(&preview_options),
                            palette,
                        );
                        ui.end_row();

                        compression_option_drag_value(
                            ui,
                            self.t("Thread Count", "线程数"),
                            &mut selected_thread_count,
                            1..=detected_threads,
                            palette,
                        );
                        compression_option_static(
                            ui,
                            self.t("Word Size", "单词大小"),
                            &self.compression_word_size_text_for(&preview_options),
                            palette,
                        );
                        ui.end_row();

                        compression_option_static(
                            ui,
                            self.t("Solid Block Size", "固实数据大小"),
                            &self.compression_solid_text_for(&preview_options),
                            palette,
                        );
                        compression_option_static(
                            ui,
                            self.t("Compression Memory", "压缩所需内存"),
                            &self.compression_memory_text_for(&preview_options),
                            palette,
                        );
                        ui.end_row();

                        compression_option_split_volume_input(
                            ui,
                            self.t("Split Volume Size", "分卷大小"),
                            &mut selected_split_volume_size_input,
                            self.t(
                                "Leave empty, or use 10M / 700M / 4G",
                                "留空表示不分卷，或输入 10M / 700M / 4G",
                            ),
                            self.language,
                            palette,
                        );
                        compression_option_static(
                            ui,
                            self.t("Encryption Method", "加密方法"),
                            if encryption_supported {
                                "AES-256"
                            } else {
                                self.t("Not available for this format", "当前格式不支持")
                            },
                            palette,
                        );
                        ui.end_row();
                    });

                ui.add_space(12.0);
                egui::Grid::new("compression-encryption-grid-stacked")
                    .num_columns(2)
                    .spacing(Vec2::new(20.0, 12.0))
                    .show(ui, |ui| {
                        compression_option_password(
                            ui,
                            self.t("Enter Password", "输入密码"),
                            &mut selected_password,
                            encryption_supported,
                            palette,
                        );
                        compression_option_password(
                            ui,
                            self.t("Reenter Password", "再次输入密码"),
                            &mut selected_password_confirm,
                            encryption_supported,
                            palette,
                        );
                        ui.end_row();

                        compression_option_checkbox(
                            ui,
                            self.t("Encrypt File Names", "加密文件名"),
                            self.t(
                                "Hide archive contents in 7Z headers",
                                "在 7Z 头部中隐藏文件内容列表",
                            ),
                            &mut selected_encrypt_file_names,
                            selected_format.supports_encrypt_file_names()
                                && !selected_password.is_empty(),
                            palette,
                        );
                        ui.end_row();
                    });

                let encryption_note = if !encryption_supported {
                    self.t(
                        "Password encryption is currently available only for ZIP and 7Z archives.",
                        "当前只有 ZIP 和 7Z 格式支持密码加密。",
                    )
                } else if selected_format == CompressionFormat::Zip {
                    self.t(
                        "ZIP only encrypts file data. Archive contents can still be listed by name, but extracting file data requires the password.",
                        "ZIP 只加密文件数据。归档仍然可以看到文件名，但没有密码不能解压出文件内容。",
                    )
                } else if selected_password.is_empty() {
                    self.t(
                        "Enter and confirm a password to enable archive encryption.",
                        "输入并确认密码后才会启用归档加密。",
                    )
                } else {
                    self.t(
                        "7Z can also encrypt file names when the checkbox is enabled.",
                        "勾选后 7Z 还可以同时加密文件名。",
                    )
                };
                ui.add_space(6.0);
                ui.label(
                    RichText::new(encryption_note)
                        .size(11.0)
                        .color(palette.text_secondary),
                );
            }
        });

        if selected_format != self.compression_options.format {
            self.apply_compression_format(selected_format);
        }
        if selected_level != self.compression_options.level {
            self.compression_options.level = selected_level;
        }
        self.compression_options.thread_count = selected_thread_count.max(1);
        if self.compression_options.format.supports_zip_method()
            && selected_zip_method != self.compression_options.zip_method
        {
            self.compression_options.zip_method = selected_zip_method;
        }
        self.compression_options.password =
            (!selected_password.is_empty()).then_some(selected_password);
        self.compression_password_confirm = selected_password_confirm;
        self.compression_split_volume_size_input = selected_split_volume_size_input;
        self.compression_options.split_volume_size = selected_split_volume_size;
        self.compression_options.encrypt_file_names = selected_encrypt_file_names
            && !self
                .compression_options
                .password()
                .unwrap_or_default()
                .is_empty()
            && self
                .compression_options
                .format
                .supports_encrypt_file_names();
    }

    fn draw_extract_options_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let mut selected_password = self.extract_password.clone();
        let mut selected_keep_paths = self.keep_paths;
        let mut selected_overwrite_mode = self.overwrite_mode;
        let mut refresh_preview = false;

        glass_panel(palette.panel_fill, palette).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.t("Extraction Options", "解压选项"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
            });
            ui.add_space(8.0);
            thin_separator(ui, palette.outline_variant);
            ui.add_space(14.0);

            egui::Grid::new("extract-options-grid")
                .num_columns(2)
                .spacing(Vec2::new(20.0, 12.0))
                .show(ui, |ui| {
                    compression_option_password(
                        ui,
                        self.t("Archive Password", "归档密码"),
                        &mut selected_password,
                        true,
                        palette,
                    );
                    compression_option_combo(
                        ui,
                        self.t("Overwrite Mode", "覆盖模式"),
                        selected_overwrite_mode.label(self.language),
                        palette,
                        |ui| {
                            for overwrite_mode in [
                                GuiOverwriteMode::Ask,
                                GuiOverwriteMode::Overwrite,
                                GuiOverwriteMode::Skip,
                                GuiOverwriteMode::Error,
                            ] {
                                ui.selectable_value(
                                    &mut selected_overwrite_mode,
                                    overwrite_mode,
                                    overwrite_mode.label(self.language),
                                );
                            }
                        },
                    );
                    ui.end_row();

                    compression_option_checkbox(
                        ui,
                        self.t("Keep Folder Structure", "保留目录结构"),
                        self.t(
                            "Extract files using their original archive paths",
                            "按照归档中的原始路径还原文件",
                        ),
                        &mut selected_keep_paths,
                        true,
                        palette,
                    );
                    compression_option_button(
                        ui,
                        self.t("Archive Preview", "归档预览"),
                        self.t(
                            "Refresh contents after changing the password",
                            "修改密码后重新刷新归档内容",
                        ),
                        self.t("Refresh Contents", "刷新内容"),
                        palette,
                        &mut refresh_preview,
                    );
                    ui.end_row();
                });

            ui.add_space(6.0);
            ui.label(
                RichText::new(self.t(
                    "Use this password for encrypted ZIP and 7Z archives. If preview fails first, enter the password here and refresh.",
                    "加密 ZIP 和 7Z 归档会使用这里的密码。若首次预览失败，请先输入密码再刷新。",
                ))
                .size(11.0)
                .color(palette.text_secondary),
            );
        });

        self.extract_password = selected_password;
        let keep_paths_changed = self.keep_paths != selected_keep_paths;
        self.keep_paths = selected_keep_paths;
        self.overwrite_mode = selected_overwrite_mode;
        if keep_paths_changed {
            let current_output = self.output_dir.trim().to_string();
            let suggestion_update = self.inspection.as_ref().map(|inspection| {
                (
                    inspection.archive_path.clone(),
                    inspection.suggested_output_dir.display().to_string(),
                )
            });
            if let Some((archive_path, previous_suggested_output)) = suggestion_update {
                let refined_suggested_output =
                    suggested_extract_output_dir(&archive_path, &self.entries, self.keep_paths);
                if current_output.is_empty() || current_output == previous_suggested_output {
                    self.output_dir = refined_suggested_output.display().to_string();
                }
                if let Some(inspection) = self.inspection.as_mut() {
                    inspection.suggested_output_dir = refined_suggested_output;
                }
            }
        }
        if refresh_preview {
            self.scan_archive();
        }
    }

    fn draw_queue_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let rows = self.queue_rows();
        let count = rows.len();

        glass_panel(palette.panel_fill, palette).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.t("File Contents", "文件内容"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    status_badge(
                        ui,
                        self.language.format(
                            "{count} Items",
                            "{count} 项",
                            &[("{count}", count.to_string())],
                        ),
                        palette.badge_fill,
                        palette.badge_text,
                    );
                });
            });
            ui.add_space(10.0);
            thin_separator(ui, palette.outline_variant);

            if rows.is_empty() {
                ui.add_space(18.0);
                ui.label(
                    RichText::new(match self.workspace_mode {
                        WorkspaceMode::Compress => self.t(
                            "Select files or folders to inspect archive contents before compression.",
                            "选择文件或文件夹后，这里会显示待压缩内容。",
                        ),
                        WorkspaceMode::Extract => self.t(
                            "Import an archive to browse its contents before extraction.",
                            "导入压缩包后，这里可以浏览压缩包内的文件内容。",
                        ),
                    })
                    .size(13.0)
                    .color(palette.text_secondary),
                );
                ui.add_space(18.0);
                return;
            }

            ui.add_space(12.0);
            egui::Grid::new("file-contents-grid")
                .num_columns(5)
                .min_row_height(30.0)
                .spacing(Vec2::new(12.0, 10.0))
                .show(ui, |ui| {
                    queue_header(ui, self.language, palette);
                    for row in rows {
                        if let Some(target) = queue_row(ui, &row, self.language, palette) {
                            self.handle_queue_target(target);
                        }
                    }
                });
        });
    }

    fn draw_task_queue_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let count = self.task_queue.len();
        let now = Instant::now();
        let mut pending_event = None;

        glass_panel(palette.panel_fill, palette).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.t("Task Queue", "任务队列"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    status_badge(
                        ui,
                        self.language.format(
                            "{count} Tasks",
                            "{count} 个任务",
                            &[("{count}", count.to_string())],
                        ),
                        palette.badge_fill,
                        palette.badge_text,
                    );
                });
            });
            ui.add_space(10.0);
            thin_separator(ui, palette.outline_variant);

            if self.task_queue.is_empty() {
                ui.add_space(18.0);
                ui.label(
                    RichText::new(self.t(
                        "Compression and extraction jobs will appear here after you add them to the queue.",
                        "加入压缩或解压任务后，这里会显示任务队列。",
                    ))
                    .size(13.0)
                    .color(palette.text_secondary),
                );
                ui.add_space(18.0);
                return;
            }

            ui.add_space(12.0);
            egui::Grid::new("task-queue-grid")
                .num_columns(6)
                .min_row_height(36.0)
                .spacing(Vec2::new(12.0, 10.0))
                .show(ui, |ui| {
                    task_queue_header(ui, self.language, palette);
                    for (index, task) in self.task_queue.iter().enumerate() {
                        let metrics = self.task_metrics(index, task, now);
                        if let Some(event) = task_queue_row(
                            ui,
                            task,
                            &metrics,
                            self.language,
                            self.can_prioritize_task(task.id),
                            palette,
                        ) {
                            pending_event = Some(event);
                        }
                    }
                });
        });

        if let Some(event) = pending_event {
            self.handle_task_queue_row_event(event);
        }
    }

    fn draw_task_output_path_dialog(&mut self, ctx: &Context) {
        let Some(full_path) = self.expanded_task_output_path.clone() else {
            return;
        };

        let palette = self.palette();
        let modal_id = egui::Id::new("task-output-path-dialog");
        let backdrop_fade = anim_bool(ctx, modal_id.with("backdrop"), true, ANIM_NORMAL);
        let modal = egui::Modal::new(modal_id)
            .backdrop_color(Color32::from_black_alpha(
                ((if palette.is_dark { 160 } else { 96 }) as f32 * backdrop_fade) as u8,
            ))
            .frame(
                Frame::default()
                    .fill(palette.panel_fill)
                    .stroke(Stroke::new(1.0, palette.panel_stroke))
                    .corner_radius(18.0)
                    .inner_margin(Margin::same(18)),
            );

        let response = modal.show(ctx, |ui| {
            ui.set_min_width(520.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.t("Output Path", "输出路径"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.add_space(12.0);
                ui.label(
                    RichText::new(full_path)
                        .size(13.0)
                        .color(palette.text)
                        .monospace(),
                );
            });
        });

        if response.should_close() {
            self.expanded_task_output_path = None;
        }
    }

    fn draw_selected_file_manager_paths_dialog(&mut self, ctx: &Context) {
        if !self.show_selected_file_manager_paths {
            return;
        }

        self.file_manager_selected_paths
            .retain(|path| path.exists());
        let selected_paths = self
            .file_manager_selected_paths
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let palette = self.palette();
        let mut close_requested = false;
        let modal_id = egui::Id::new("selected-file-manager-paths-dialog");
        let backdrop_fade = anim_bool(ctx, modal_id.with("backdrop"), true, ANIM_NORMAL);
        let modal = egui::Modal::new(modal_id)
            .backdrop_color(Color32::from_black_alpha(
                ((if palette.is_dark { 160 } else { 96 }) as f32 * backdrop_fade) as u8,
            ))
            .frame(
                Frame::default()
                    .fill(palette.panel_fill)
                    .stroke(Stroke::new(1.0, palette.panel_stroke))
                    .corner_radius(18.0)
                    .inner_margin(Margin::same(18)),
            );

        let response = modal.show(ctx, |ui| {
            ui.set_min_width(660.0);
            ui.set_min_height(360.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.t("Selected Files", "已选中文件"))
                            .size(16.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        status_badge(
                            ui,
                            self.language.format(
                                "{count} Items",
                                "{count} 项",
                                &[("{count}", selected_paths.len().to_string())],
                            ),
                            palette.badge_fill,
                            palette.badge_text,
                        );
                    });
                });
                ui.add_space(10.0);
                thin_separator(ui, palette.outline_variant);
                ui.add_space(12.0);

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(300.0)
                    .show(ui, |ui| {
                        if selected_paths.is_empty() {
                            ui.label(
                                RichText::new(self.t(
                                    "No files or folders are selected in the file manager.",
                                    "文件管理器中还没有选中任何文件或文件夹。",
                                ))
                                .size(13.0)
                                .color(palette.text_secondary),
                            );
                        } else {
                            for path in &selected_paths {
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(path.display().to_string())
                                        .size(13.0)
                                        .color(palette.text)
                                        .monospace(),
                                );
                            }
                        }
                    });

                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(Button::new(self.t("Cancel", "取消")).min_size(Vec2::new(96.0, 36.0)))
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });

        if close_requested || response.should_close() {
            self.show_selected_file_manager_paths = false;
        }
    }

    fn draw_save_preset_dialog(&mut self, ctx: &Context) {
        if !self.show_save_preset_dialog {
            return;
        }
        let palette = self.palette();
        let mut close_requested = false;
        let modal_id = egui::Id::new("save-preset-dialog");
        let backdrop_fade = anim_bool(ctx, modal_id.with("backdrop"), true, ANIM_NORMAL);
        let modal = egui::Modal::new(modal_id)
            .backdrop_color(Color32::from_black_alpha(
                ((if palette.is_dark { 160 } else { 96 }) as f32 * backdrop_fade) as u8,
            ))
            .frame(
                Frame::default()
                    .fill(palette.panel_fill)
                    .stroke(Stroke::new(1.0, palette.panel_stroke))
                    .corner_radius(18.0)
                    .inner_margin(Margin::same(18)),
            );

        let response = modal.show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.t("Save Preset", "保存预设"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new(self.t("Preset name:", "预设名称："))
                        .size(12.0)
                        .color(palette.text_secondary),
                );
                ui.add_space(4.0);
                ui.add_sized(
                    [ui.available_width(), 30.0],
                    TextEdit::singleline(&mut self.preset_save_name)
                        .margin(Margin::symmetric(6, 4)),
                );
                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            Button::new(self.t("Cancel", "取消"))
                                .min_size(Vec2::new(80.0, 32.0)),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                    ui.add_space(8.0);
                    let can_save = !self.preset_save_name.trim().is_empty();
                    if ui
                        .add_enabled(
                            can_save,
                            Button::new(
                                RichText::new(self.t("Save", "保存"))
                                    .color(palette.on_primary),
                            )
                            .fill(palette.primary_strong)
                            .corner_radius(8.0)
                            .min_size(Vec2::new(80.0, 32.0)),
                        )
                        .clicked()
                    {
                        let name = self.preset_save_name.trim().to_string();
                        let value = crate::settings::encode_preset_value(
                            self.compression_options.format,
                            self.compression_options.level,
                            self.compression_options.zip_method,
                            self.compression_options.thread_count,
                            self.compression_options.encrypt_file_names,
                        );
                        match crate::settings::save_preset(&name, &value) {
                            Ok(_) => {
                                close_requested = true;
                            }
                            Err(e) => {
                                self.show_toast(
                                    FeedbackTone::Error,
                                    self.language.format(
                                        "Failed to save preset: {error}",
                                        "保存预设失败：{error}",
                                        &[("{error}", format!("{e:#}"))],
                                    ),
                                );
                            }
                        }
                    }
                });
            });
        });

        if close_requested || response.should_close() {
            self.show_save_preset_dialog = false;
        }
    }

    fn draw_manage_presets_dialog(&mut self, ctx: &Context) {
        if !self.show_manage_presets_dialog {
            return;
        }
        let palette = self.palette();
        let mut close_requested = false;
        let modal_id = egui::Id::new("manage-presets-dialog");
        let backdrop_fade = anim_bool(ctx, modal_id.with("backdrop"), true, ANIM_NORMAL);
        let modal = egui::Modal::new(modal_id)
            .backdrop_color(Color32::from_black_alpha(
                ((if palette.is_dark { 160 } else { 96 }) as f32 * backdrop_fade) as u8,
            ))
            .frame(
                Frame::default()
                    .fill(palette.panel_fill)
                    .stroke(Stroke::new(1.0, palette.panel_stroke))
                    .corner_radius(18.0)
                    .inner_margin(Margin::same(18)),
            );

        let preset_names = crate::settings::list_preset_names();
        let response = modal.show(ctx, |ui| {
            ui.set_min_width(440.0);
            ui.set_min_height(300.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.t("Manage Presets", "管理预设"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.add_space(10.0);
                thin_separator(ui, palette.outline_variant);
                ui.add_space(10.0);

                if preset_names.is_empty() {
                    ui.label(
                        RichText::new(self.t(
                            "No presets saved yet.",
                            "尚未保存任何预设。",
                        ))
                        .size(13.0)
                        .color(palette.text_secondary),
                    );
                } else {
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for name in &preset_names {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(name)
                                            .size(13.0)
                                            .color(palette.text),
                                    );
                                    ui.with_layout(
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            if ui
                                                .add(
                                                    Button::new(
                                                        RichText::new(
                                                            self.t("Delete", "删除"),
                                                        )
                                                        .size(11.0)
                                                        .color(palette.error),
                                                    )
                                                    .fill(palette.surface_high)
                                                    .stroke(Stroke::new(
                                                        1.0,
                                                        palette.outline_variant,
                                                    ))
                                                    .corner_radius(6.0)
                                                    .min_size(Vec2::new(64.0, 26.0)),
                                                )
                                                .clicked()
                                            {
                                                if let Err(e) =
                                                    crate::settings::delete_preset(name)
                                                {
                                                    self.show_toast(
                                                        FeedbackTone::Error,
                                                        self.language.format(
                                                            "Failed to delete preset: {error}",
                                                            "删除预设失败：{error}",
                                                            &[(
                                                                "{error}",
                                                                format!("{e:#}"),
                                                            )],
                                                        ),
                                                    );
                                                }
                                                // Force close/reopen to refresh list
                                                close_requested = true;
                                                ui.ctx().request_repaint();
                                            }
                                        },
                                    );
                                });
                                ui.add_space(4.0);
                            }
                        });
                }

                ui.add_space(10.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            Button::new(self.t("Close", "关闭"))
                                .min_size(Vec2::new(96.0, 36.0)),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });

        if close_requested || response.should_close() {
            self.show_manage_presets_dialog = false;
        }
    }

    fn draw_test_result_dialog(&mut self, ctx: &Context) {
        if !self.show_test_result_dialog {
            return;
        }
        let Some(report) = self.test_report.as_ref() else {
            return;
        };
        let palette = self.palette();
        let mut close_requested = false;
        let modal_id = egui::Id::new("test-result-dialog");
        let backdrop_fade = anim_bool(ctx, modal_id.with("backdrop"), true, ANIM_NORMAL);
        let modal = egui::Modal::new(modal_id)
            .backdrop_color(Color32::from_black_alpha(
                ((if palette.is_dark { 160 } else { 96 }) as f32 * backdrop_fade) as u8,
            ))
            .frame(
                Frame::default()
                    .fill(palette.panel_fill)
                    .stroke(Stroke::new(1.0, palette.panel_stroke))
                    .corner_radius(18.0)
                    .inner_margin(Margin::same(18)),
            );

        let response = modal.show(ctx, |ui| {
            ui.set_min_width(480.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.t("Archive Integrity Test", "压缩包完整性测试"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(report.archive_path.display().to_string())
                        .size(12.0)
                        .color(palette.text_secondary)
                        .monospace(),
                );
                ui.add_space(10.0);
                thin_separator(ui, palette.outline_variant);
                ui.add_space(12.0);

                let status_color = if report.is_healthy() {
                    palette.primary
                } else {
                    palette.error
                };
                let status_text = if report.is_healthy() {
                    self.t("PASSED  -  No errors detected", "通过  -  未检测到错误")
                } else {
                    self.t("CORRUPT  -  Errors found", "损坏  -  检测到错误")
                };
                ui.label(
                    RichText::new(status_text)
                        .size(15.0)
                        .strong()
                        .color(status_color),
                );

                ui.add_space(12.0);
                let info_card = Frame::default()
                    .fill(palette.surface_low)
                    .corner_radius(10.0)
                    .inner_margin(Margin::same(12));
                info_card.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(self.t("Format:", "格式："))
                                .size(12.0)
                                .color(palette.text_secondary),
                        );
                        ui.label(
                            RichText::new(report.format.to_string())
                                .size(12.0)
                                .color(palette.text),
                        );
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(self.t("Entries tested:", "已测试条目："))
                                .size(12.0)
                                .color(palette.text_secondary),
                        );
                        ui.label(
                            RichText::new(report.entries_tested.to_string())
                                .size(12.0)
                                .color(palette.text),
                        );
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(self.t("Bytes read:", "已读取字节："))
                                .size(12.0)
                                .color(palette.text_secondary),
                        );
                        ui.label(
                            RichText::new(report.bytes_read.to_string())
                                .size(12.0)
                                .color(palette.text),
                        );
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(self.t("Elapsed:", "耗时："))
                                .size(12.0)
                                .color(palette.text_secondary),
                        );
                        ui.label(
                            RichText::new(format!("{:?}", report.elapsed))
                                .size(12.0)
                                .color(palette.text),
                        );
                    });
                });

                if !report.errors.is_empty() {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(self.t("Errors:", "错误："))
                            .size(12.0)
                            .strong()
                            .color(palette.error),
                    );
                    ui.add_space(4.0);
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for error in &report.errors {
                                ui.label(
                                    RichText::new(error)
                                        .size(11.0)
                                        .color(palette.error)
                                        .monospace(),
                                );
                            }
                        });
                }

                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            Button::new(self.t("Close", "关闭"))
                                .min_size(Vec2::new(96.0, 36.0)),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });

        if close_requested || response.should_close() {
            self.show_test_result_dialog = false;
        }
    }

    fn draw_extract_conflict_dialog(&mut self, ctx: &Context) {
        let Some(dialog) = self.extract_conflict_dialog.as_ref() else {
            return;
        };

        let Some(conflict) = dialog.conflicts.get(dialog.current_index).cloned() else {
            return;
        };

        let palette = self.palette();
        let mut apply_to_all = dialog.apply_to_all;
        let mut rename_value = dialog.rename_value.clone();
        let mut pending_action = None;
        let mut canceled = false;
        let modal_id = egui::Id::new("extract-conflict-dialog");
        let backdrop_fade = anim_bool(ctx, modal_id.with("backdrop"), true, ANIM_NORMAL);
        let modal = egui::Modal::new(modal_id)
            .backdrop_color(Color32::from_black_alpha(
                ((if palette.is_dark { 160 } else { 96 }) as f32 * backdrop_fade) as u8,
            ))
            .frame(
                Frame::default()
                    .fill(palette.panel_fill)
                    .stroke(Stroke::new(1.0, palette.panel_stroke))
                    .corner_radius(18.0)
                    .inner_margin(Margin::same(18)),
            );

        modal.show(ctx, |ui| {
            ui.set_min_width(620.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.t("Resolve File Conflicts", "处理同名文件冲突"))
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(self.language.format(
                        "Conflict {current} of {total}",
                        "冲突 {current} / {total}",
                        &[
                            ("{current}", (dialog.current_index + 1).to_string()),
                            ("{total}", dialog.conflicts.len().to_string()),
                        ],
                    ))
                    .size(12.0)
                    .color(palette.text_muted),
                );
                ui.add_space(14.0);

                ui.label(
                    RichText::new(self.t("Archive Entry", "压缩包条目"))
                        .size(12.0)
                        .color(palette.text_secondary),
                );
                ui.label(
                    RichText::new(conflict.relative_path.display().to_string())
                        .size(13.0)
                        .color(palette.text)
                        .monospace(),
                );
                ui.add_space(10.0);

                ui.label(
                    RichText::new(self.t("Existing Path", "已存在路径"))
                        .size(12.0)
                        .color(palette.text_secondary),
                );
                ui.label(
                    RichText::new(conflict.destination.display().to_string())
                        .size(13.0)
                        .color(palette.text)
                        .monospace(),
                );
                ui.add_space(12.0);

                ui.label(
                    RichText::new(self.t("New File Name", "新文件名"))
                        .size(12.0)
                        .color(palette.text_secondary),
                );
                ui.add(
                    TextEdit::singleline(&mut rename_value)
                        .desired_width(f32::INFINITY)
                        .hint_text(self.t("New File Name", "新文件名")),
                );
                ui.add_space(10.0);

                if conflict.existing_is_dir {
                    ui.label(
                        RichText::new(self.t(
                            "Overwrite cannot replace a folder destination.",
                            "覆盖操作不能替换目标文件夹。",
                        ))
                        .size(12.0)
                        .color(palette.error),
                    );
                    ui.add_space(8.0);
                }

                ui.checkbox(
                    &mut apply_to_all,
                    self.t(
                        "Apply this action to all remaining conflicts",
                        "将此操作应用到剩余全部冲突",
                    ),
                );
                ui.add_space(14.0);

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !conflict.existing_is_dir,
                            Button::new(self.t("Overwrite", "覆盖"))
                                .min_size(Vec2::new(108.0, 36.0)),
                        )
                        .clicked()
                    {
                        pending_action = Some(ExtractConflictAction::Overwrite);
                    }

                    if ui
                        .add(Button::new(self.t("Skip", "跳过")).min_size(Vec2::new(108.0, 36.0)))
                        .clicked()
                    {
                        pending_action = Some(ExtractConflictAction::Skip);
                    }

                    if ui
                        .add(
                            Button::new(self.t("Rename", "重命名"))
                                .min_size(Vec2::new(108.0, 36.0)),
                        )
                        .clicked()
                    {
                        pending_action = Some(ExtractConflictAction::Rename);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(self.t("Cancel", "取消"))
                                    .min_size(Vec2::new(96.0, 36.0)),
                            )
                            .clicked()
                        {
                            canceled = true;
                        }
                    });
                });
            });
        });

        if let Some(dialog) = self.extract_conflict_dialog.as_mut() {
            dialog.apply_to_all = apply_to_all;
            dialog.rename_value = rename_value;
        }

        if canceled {
            self.extract_conflict_dialog = None;
            self.show_toast(
                FeedbackTone::Info,
                self.t("Conflict resolution canceled.", "已取消冲突处理。"),
            );
            return;
        }

        if let Some(action) = pending_action {
            self.apply_extract_conflict_action(action);
        }
    }

    fn apply_extract_conflict_action(&mut self, action: ExtractConflictAction) {
        let Some(mut dialog) = self.extract_conflict_dialog.take() else {
            return;
        };

        let Some(current_conflict) = dialog.conflicts.get(dialog.current_index).cloned() else {
            return;
        };

        let next_index = if dialog.apply_to_all {
            dialog.conflicts.len()
        } else {
            dialog.current_index + 1
        };

        match action {
            ExtractConflictAction::Overwrite => {
                if dialog.apply_to_all
                    && dialog.conflicts[dialog.current_index..]
                        .iter()
                        .any(|conflict| conflict.existing_is_dir)
                {
                    self.show_toast(
                        FeedbackTone::Error,
                        self.t(
                            "Overwrite cannot replace a folder destination.",
                            "覆盖操作不能替换目标文件夹。",
                        ),
                    );
                    self.extract_conflict_dialog = Some(dialog);
                    return;
                }
            }
            ExtractConflictAction::Skip => {
                for conflict in dialog.conflicts.iter().skip(dialog.current_index).take(
                    if dialog.apply_to_all {
                        dialog.conflicts.len() - dialog.current_index
                    } else {
                        1
                    },
                ) {
                    dialog
                        .task
                        .plan
                        .skipped_paths
                        .insert(conflict.relative_path.clone());
                }
            }
            ExtractConflictAction::Rename => {
                let mut reserved_paths = Self::reserved_extract_conflict_paths(&dialog);
                let renamed_destination = match self.validate_extract_conflict_rename(
                    &current_conflict,
                    dialog.rename_value.trim(),
                    &dialog.task.plan,
                    &reserved_paths,
                ) {
                    Ok(path) => path,
                    Err(message) => {
                        self.show_toast(FeedbackTone::Error, message);
                        self.extract_conflict_dialog = Some(dialog);
                        return;
                    }
                };
                reserved_paths.insert(renamed_destination.clone());
                dialog
                    .task
                    .plan
                    .renamed_paths
                    .insert(current_conflict.relative_path.clone(), renamed_destination);

                if dialog.apply_to_all {
                    for conflict in dialog.conflicts.iter().skip(dialog.current_index + 1) {
                        let destination = suggest_available_conflict_path(
                            &conflict.destination,
                            &dialog.task.plan,
                            &reserved_paths,
                        );
                        reserved_paths.insert(destination.clone());
                        dialog
                            .task
                            .plan
                            .renamed_paths
                            .insert(conflict.relative_path.clone(), destination);
                    }
                }
            }
        }

        if next_index >= dialog.conflicts.len() {
            self.enqueue_extract_task(dialog.task);
            return;
        }

        dialog.current_index = next_index;
        dialog.apply_to_all = false;
        if let Some(next_conflict) = dialog.conflicts.get(dialog.current_index) {
            dialog.rename_value = self
                .default_extract_conflict_rename_value(
                    next_conflict,
                    &dialog.task.plan,
                    &Self::reserved_extract_conflict_paths(&dialog),
                )
                .unwrap_or_default();
        }
        self.extract_conflict_dialog = Some(dialog);
    }

    fn validate_extract_conflict_rename(
        &self,
        conflict: &ExtractConflictItem,
        rename_value: &str,
        plan: &ExtractPathPlan,
        reserved_paths: &BTreeSet<PathBuf>,
    ) -> std::result::Result<PathBuf, String> {
        if rename_value.is_empty() {
            return Err(self
                .t("The new file name cannot be empty.", "新文件名不能为空。")
                .to_string());
        }

        let candidate_name = Path::new(rename_value);
        let components = candidate_name.components().collect::<Vec<_>>();
        if components.len() != 1 || !matches!(components[0], std::path::Component::Normal(_)) {
            return Err(self
                .t(
                    "Enter a file name only, not a folder path.",
                    "这里只能输入文件名，不能输入文件夹路径。",
                )
                .to_string());
        }

        let candidate_path = conflict.destination.with_file_name(rename_value);
        if candidate_path.exists() {
            return Err(self
                .t(
                    "The selected rename target already exists.",
                    "选择的新文件名已存在。",
                )
                .to_string());
        }
        if reserved_paths.contains(&candidate_path)
            || plan
                .renamed_paths
                .values()
                .any(|existing| existing == &candidate_path)
        {
            return Err(self
                .t(
                    "The selected rename target is already reserved by another extracted file.",
                    "选择的新文件名已被其他解压文件占用。",
                )
                .to_string());
        }

        Ok(candidate_path)
    }

    fn draw_logs_page(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();

        paint_background(ui, palette);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);

                glass_panel(palette.panel_fill, palette).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(self.t("System Logs", "系统日志"))
                                    .size(20.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.t(
                                    "Runtime events, backend routing, and extraction results.",
                                    "运行事件、后端路由和解压结果都会显示在这里。",
                                ))
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(self.t("Clear Logs", "清空日志"))
                                            .size(11.0)
                                            .color(palette.text_secondary),
                                    )
                                    .fill(palette.subtle_fill)
                                    .stroke(Stroke::new(1.0, palette.subtle_stroke))
                                    .corner_radius(10.0)
                                    .min_size(Vec2::new(108.0, 32.0)),
                                )
                                .clicked()
                            {
                                self.logs.clear();
                                self.push_log(self.t("Log history cleared.", "日志历史已清空。"));
                            }

                            ui.add(
                                Button::new(
                                    RichText::new(self.language.format(
                                        "{count} Lines",
                                        "{count} 条记录",
                                        &[("{count}", self.logs.len().to_string())],
                                    ))
                                    .size(11.0)
                                    .strong()
                                    .color(palette.badge_text),
                                )
                                .sense(egui::Sense::hover())
                                .fill(palette.badge_fill)
                                .stroke(Stroke::new(1.0, palette.outline_variant))
                                .corner_radius(10.0)
                                .min_size(Vec2::new(92.0, 32.0)),
                            );
                        });
                    });

                    ui.add_space(14.0);
                    thin_separator(ui, palette.outline_variant);
                    ui.add_space(14.0);

                    log_frame(palette).show(ui, |ui| {
                        ui.set_min_height(520.0);

                        if self.logs.is_empty() {
                            ui.label(
                                RichText::new(
                                    self.t("No log entries yet.", "暂时还没有日志记录。"),
                                )
                                .monospace()
                                .size(12.0)
                                .color(palette.log_text),
                            );
                            return;
                        }

                        ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for (index, line) in self.logs.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [36.0, 18.0],
                                            egui::Label::new(
                                                RichText::new(format!("{:03}", index + 1))
                                                    .monospace()
                                                    .size(11.0)
                                                    .color(palette.log_index),
                                            ),
                                        );
                                        ui.label(
                                            RichText::new(line)
                                                .monospace()
                                                .size(11.5)
                                                .color(palette.log_text),
                                        );
                                    });
                                    ui.add_space(4.0);
                                }
                            });
                    });
                });
            });
    }

    fn draw_settings_page(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();

        paint_background(ui, palette);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);

                glass_panel(palette.panel_fill, palette).show(ui, |ui| {
                    ui.label(
                        RichText::new(self.t("Settings", "设置"))
                            .size(20.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.add_space(14.0);
                    thin_separator(ui, palette.outline_variant);
                    ui.add_space(18.0);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(self.t("Interface Language", "界面语言"))
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.t(
                                    "Choose the language used across the dashboard, logs, and actions.",
                                    "选择整个界面、日志和操作区域使用的语言。",
                                ))
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            egui::ComboBox::from_id_salt("settings-language")
                                .selected_text(self.locale.display_name)
                                .width(128.0)
                                .show_ui(ui, |ui| {
                                    for locale in supported_locales() {
                                        if ui
                                            .selectable_label(
                                                self.locale == locale,
                                                locale.display_name,
                                            )
                                            .clicked()
                                        {
                                            self.switch_locale(locale);
                                        }
                                    }
                                });
                        });
                    });

                    ui.add_space(18.0);
                    thin_separator(ui, palette.outline_variant);
                    ui.add_space(18.0);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(self.t("Dark Mode", "深色模式"))
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.t(
                                    "Use the darker glass palette inspired by the new dashboard reference.",
                                    "启用参考稿里的深色毛玻璃界面配色。",
                                ))
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(self.theme_mode.label(self.language))
                                    .size(11.0)
                                    .strong()
                                    .color(if self.theme_mode.is_dark() {
                                        palette.primary
                                    } else {
                                        palette.text_muted
                                    }),
                            );
                            ui.add_space(12.0);

                            if theme_switch(ui, "theme", self.theme_mode.is_dark(), palette) {
                                self.switch_theme(if self.theme_mode.is_dark() {
                                    ThemeMode::Light
                                } else {
                                    ThemeMode::Dark
                                });
                            }
                        });
                    });

                    ui.add_space(18.0);
                    thin_separator(ui, palette.outline_variant);
                    ui.add_space(18.0);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(self.t("Start with Windows", "开机自启动"))
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.t(
                                    "Launch FastZIP automatically when you sign in to Windows.",
                                    "在登录 Windows 时自动启动 FastZIP。",
                                ))
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(if self.autostart_enabled {
                                    self.t("Enabled", "已开启")
                                } else {
                                    self.t("Disabled", "已关闭")
                                })
                                .size(11.0)
                                .strong()
                                .color(if self.autostart_enabled {
                                    palette.primary
                                } else {
                                    palette.text_muted
                                }),
                            );
                            ui.add_space(12.0);

                            if theme_switch(ui, "autostart", self.autostart_enabled, palette) {
                                self.switch_autostart(!self.autostart_enabled);
                            }
                        });
                    });

                    ui.add_space(18.0);
                    thin_separator(ui, palette.outline_variant);
                    ui.add_space(18.0);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(self.t("AMSI Malware Scan", "AMSI 恶意软件扫描"))
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.t(
                                    "Scan extracted files with Windows Antimalware Scan Interface before writing to disk.",
                                    "写入磁盘前，使用 Windows 反恶意软件扫描接口 (AMSI) 扫描提取的文件。",
                                ))
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(if self.scan_files {
                                    self.t("Enabled", "已开启")
                                } else {
                                    self.t("Disabled", "已关闭")
                                })
                                .size(11.0)
                                .strong()
                                .color(if self.scan_files {
                                    palette.primary
                                } else {
                                    palette.text_muted
                                }),
                            );
                            ui.add_space(12.0);

                            if theme_switch(ui, "amsi-scan", self.scan_files, palette) {
                                self.scan_files = !self.scan_files;
                            }
                        });
                    });

                    ui.add_space(18.0);
                    thin_separator(ui, palette.outline_variant);
                    ui.add_space(18.0);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(self.t("Default Apps", "默认应用"))
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.t(
                                    "Set FastZIP as the default app for opening archive files.",
                                    "将 FastZIP 设为打开压缩文件的默认应用。",
                                ))
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let btn_text = self.t(
                                "Set as Default",
                                "设为默认应用",
                            );
                            let button = Button::new(
                                RichText::new(btn_text)
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text_secondary),
                            )
                            .fill(palette.subtle_fill)
                            .stroke(Stroke::new(1.0, palette.subtle_stroke))
                            .corner_radius(10.0)
                            .min_size(Vec2::new(36.0, 30.0));
                            if ui.add(button).clicked() {
                                self.try_open_default_apps_dialog();
                            }
                        });
                    });

                    ui.add_space(18.0);
                    thin_separator(ui, palette.outline_variant);
                    ui.add_space(18.0);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(self.t("Performance Benchmark", "性能基准测试"))
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.t(
                                    "Run compression benchmarks across all formats and levels. Results are saved to disk.",
                                    "运行所有格式和级别的压缩基准测试。结果保存到磁盘。",
                                ))
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let btn_text = self.t(
                                "Run Benchmark",
                                "运行基准测试",
                            );
                            let button = Button::new(
                                RichText::new(btn_text)
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text_secondary),
                            )
                            .fill(palette.subtle_fill)
                            .stroke(Stroke::new(1.0, palette.subtle_stroke))
                            .corner_radius(10.0)
                            .min_size(Vec2::new(36.0, 30.0));
                            if ui.add(button).clicked() {
                                self.start_benchmark();
                            }
                        });
                    });
                });
            });
    }

    fn handle_queue_target(&mut self, target: QueueRowTarget) {
        match target {
            QueueRowTarget::ExtractUp => {
                self.extract_browser_path.pop();
            }
            QueueRowTarget::ExtractEntry { path, is_dir } => {
                if is_dir {
                    self.extract_browser_path = path;
                } else if let Err(error) = self.preview_archive_entry(&path) {
                    self.push_log(self.language.format(
                        "Preview failed: {error}",
                        "预览失败：{error}",
                        &[("{error}", format!("{error:#}"))],
                    ));
                }
            }
            QueueRowTarget::CompressUp => {
                if let Some(current_dir) = self.compress_browser_path.clone() {
                    if self.is_compress_root_path(&current_dir) {
                        self.compress_browser_path = None;
                    } else {
                        self.compress_browser_path = current_dir.parent().map(Path::to_path_buf);
                    }
                }
            }
            QueueRowTarget::CompressPath { path, is_dir } => {
                if is_dir {
                    self.compress_browser_path = Some(path);
                } else if let Err(error) = open_path_with_system_default(&path) {
                    self.push_log(self.language.format(
                        "Open failed: {error}",
                        "打开失败：{error}",
                        &[("{error}", format!("{error:#}"))],
                    ));
                }
            }
            QueueRowTarget::RemoveCompressSource { path } => {
                if self.is_compress_root_path(&path) {
                    self.remove_compress_source(&path);
                } else {
                    self.exclude_compress_path(&path);
                }
            }
        }
    }

    fn is_compress_root_path(&self, path: &Path) -> bool {
        self.compress_sources.iter().any(|source| source == path)
    }

    fn is_path_within_any_compress_source(&self, path: &Path) -> bool {
        self.compress_sources
            .iter()
            .any(|source| path == source || path.starts_with(source))
    }

    fn is_path_excluded_from_compress(&self, path: &Path) -> bool {
        self.compress_excluded_paths
            .iter()
            .any(|excluded| path == excluded || path.starts_with(excluded))
    }

    fn preview_archive_entry(&mut self, relative_path: &Path) -> Result<()> {
        let archive_path = self.archive_path_buf().map_err(anyhow::Error::msg)?;
        let archive_canonical =
            fs::canonicalize(&archive_path).unwrap_or_else(|_| archive_path.clone());
        let preview_root = if self
            .preview_archive_path
            .as_ref()
            .is_some_and(|cached| cached == &archive_canonical)
        {
            self.preview_output_dir.clone().ok_or_else(|| {
                anyhow!(localize_format(
                    "Missing preview cache for {archive_path}",
                    "{archive_path} 的预览缓存不存在",
                    &[("{archive_path}", archive_path.display().to_string())],
                ))
            })?
        } else {
            let unique_id = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let preview_root = env::temp_dir().join("fastzip-preview").join(format!(
                "{}-{}",
                archive_path
                    .file_stem()
                    .map(|value| value.to_string_lossy().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "archive".to_string()),
                unique_id
            ));
            fs::create_dir_all(&preview_root).with_context(|| {
                localize_format(
                    "Failed to create preview folder {preview_root}",
                    "创建预览文件夹 {preview_root} 失败",
                    &[("{preview_root}", preview_root.display().to_string())],
                )
            })?;
            self.service.extract_archive(
                &archive_path,
                &ExtractOptions {
                    output_dir: preview_root.clone(),
                    overwrite_mode: OverwriteMode::Overwrite,
                    keep_paths: true,
                    password: (!self.extract_password.is_empty())
                        .then_some(self.extract_password.clone()),
                    filename_encoding: self.filename_encoding,
                    scan_files: false,
                },
            )?;
            self.preview_archive_path = Some(archive_canonical);
            self.preview_output_dir = Some(preview_root.clone());
            preview_root
        };

        let target = preview_root.join(relative_path);
        if !target.exists() {
            bail!(
                "{}",
                localize_format(
                    "Preview target not found: {target}",
                    "未找到预览目标：{target}",
                    &[("{target}", target.display().to_string())],
                )
            );
        }
        open_path_with_system_default(&target)
    }

    fn compress_sources_display(&self) -> String {
        match self.compress_sources.as_slice() {
            [] => String::new(),
            [single] => single.display().to_string(),
            many => self.language.format(
                "{first} and {rest} more",
                "{first} 等 {rest} 项",
                &[
                    ("{first}", many[0].display().to_string()),
                    ("{rest}", (many.len() - 1).to_string()),
                ],
            ),
        }
    }

    fn collect_file_manager_entries(&self) -> Result<Vec<FileManagerEntry>> {
        let mut entries = fs::read_dir(&self.file_manager_current_dir)
            .with_context(|| {
                format!(
                    "Failed to open file manager directory {}",
                    self.file_manager_current_dir.display()
                )
            })?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                let path = normalize_windows_user_path(&path);
                let metadata = entry.metadata().ok()?;
                let is_dir = metadata.is_dir();
                let name = entry.file_name().to_string_lossy().to_string();
                let file_kind = detect_file_kind(&path, is_dir);
                Some(FileManagerEntry {
                    path: path.clone(),
                    name,
                    size: if is_dir {
                        "-".to_string()
                    } else {
                        format_size(Some(metadata.len()))
                    },
                    kind: file_kind.label(self.language).to_string(),
                    is_dir,
                    show_extract_action: file_kind.show_extract_action(),
                })
            })
            .collect::<Vec<_>>();

        entries.sort_by(|left, right| {
            (!left.is_dir)
                .cmp(&!right.is_dir)
                .then_with(|| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                })
                .then_with(|| left.path.cmp(&right.path))
        });
        Ok(entries)
    }

    fn queue_rows(&self) -> Vec<QueueRowData> {
        let query = self.search_query.trim().to_ascii_lowercase();
        let mut rows = match self.workspace_mode {
            WorkspaceMode::Compress => self.compress_rows(),
            WorkspaceMode::Extract => self.extract_rows(),
        };

        if !query.is_empty() {
            rows.retain(|row| row.name.to_ascii_lowercase().contains(&query));
        }

        rows
    }

    fn compress_rows(&self) -> Vec<QueueRowData> {
        if let Some(current_dir) = &self.compress_browser_path {
            let mut rows = vec![QueueRowData {
                name: "..".to_string(),
                size: "-".to_string(),
                kind: self.t("Other", "其他").to_string(),
                status: QueueStatus::Complete,
                action: QueueAction::Back,
                target: QueueRowTarget::CompressUp,
                secondary_target: None,
            }];
            rows.extend(self.compress_child_rows(current_dir));
            rows
        } else {
            self.compress_root_rows()
        }
    }

    fn extract_rows(&self) -> Vec<QueueRowData> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        let mut rows = Vec::new();
        if !self.extract_browser_path.as_os_str().is_empty() {
            rows.push(QueueRowData {
                name: "..".to_string(),
                size: "-".to_string(),
                kind: self.t("Other", "其他").to_string(),
                status: QueueStatus::Complete,
                action: QueueAction::Back,
                target: QueueRowTarget::ExtractUp,
                secondary_target: None,
            });
        }

        let mut direct_children = BTreeMap::<PathBuf, QueueRowData>::new();
        for entry in &self.entries {
            let Ok(relative_to_current) = entry.path.strip_prefix(&self.extract_browser_path)
            else {
                continue;
            };
            if relative_to_current.as_os_str().is_empty() {
                continue;
            }

            let Some(first_component) = relative_to_current.iter().next() else {
                continue;
            };
            let child_path = self.extract_browser_path.join(first_component);
            let is_direct = relative_to_current.components().count() == 1;
            let is_dir = if is_direct { entry.is_dir } else { true };

            direct_children
                .entry(child_path.clone())
                .or_insert_with(|| QueueRowData {
                    name: first_component.to_string_lossy().to_string(),
                    size: if is_dir {
                        "-".to_string()
                    } else {
                        format_size(entry.uncompressed_size)
                    },
                    kind: file_kind_from_path(&child_path, is_dir, self.language),
                    status: QueueStatus::Complete,
                    action: QueueAction::View,
                    target: QueueRowTarget::ExtractEntry {
                        path: child_path,
                        is_dir,
                    },
                    secondary_target: None,
                });
        }

        rows.extend(direct_children.into_values());
        rows
    }

    fn compress_root_rows(&self) -> Vec<QueueRowData> {
        self.compress_sources
            .iter()
            .filter(|source| !self.is_path_excluded_from_compress(source))
            .map(|source| self.compress_path_row(source))
            .collect()
    }

    fn compress_child_rows(&self, current_dir: &Path) -> Vec<QueueRowData> {
        let Ok(entries) = fs::read_dir(current_dir) else {
            return Vec::new();
        };

        let mut children = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        children.sort();
        children
            .iter()
            .filter(|child| !self.is_path_excluded_from_compress(child))
            .map(|child| self.compress_path_row(child))
            .collect()
    }

    fn compress_path_row(&self, path: &Path) -> QueueRowData {
        let is_dir = path.is_dir();
        let size = if is_dir {
            "-".to_string()
        } else {
            format_size(path.metadata().ok().map(|meta| meta.len()))
        };

        QueueRowData {
            name: path
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string()),
            size,
            kind: file_kind_from_path(path, is_dir, self.language),
            status: QueueStatus::Complete,
            action: QueueAction::ViewAndRemove,
            target: QueueRowTarget::CompressPath {
                path: path.to_path_buf(),
                is_dir,
            },
            secondary_target: Some(QueueRowTarget::RemoveCompressSource {
                path: path.to_path_buf(),
            }),
        }
    }

    fn task_metrics(&self, _index: usize, task: &TaskQueueItem, now: Instant) -> TaskQueueMetrics {
        let progress = real_task_progress(task);
        let speed_text = task
            .current_bytes_per_second
            .map(format_transfer_rate)
            .unwrap_or_else(|| "--".to_string());
        let is_finalizing = task_is_finalizing(task);
        let eta_text = match task.state {
            TaskState::Queued => "--".to_string(),
            TaskState::Running if is_finalizing => self.t("Finalizing", "收尾中").to_string(),
            TaskState::Running => running_task_eta(task, now)
                .map(format_duration_compact)
                .unwrap_or_else(|| "--".to_string()),
            TaskState::Canceling if is_finalizing => self.t("Finalizing", "收尾中").to_string(),
            TaskState::Canceling => self.t("Stopping", "停止中").to_string(),
            TaskState::Completed => self.t("Done", "已完成").to_string(),
            TaskState::Canceled => self.t("Canceled", "已取消").to_string(),
            TaskState::Failed => self.t("Failed", "失败").to_string(),
        };

        TaskQueueMetrics {
            progress,
            progress_text: if is_finalizing {
                self.t("Finalizing", "收尾中").to_string()
            } else {
                format!("{:.0}%", progress * 100.0)
            },
            speed_text,
            eta_text,
            status_text: task.state.label(self.language).to_string(),
        }
    }

    fn draw_footer(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let version_label = format!("v{}", env!("CARGO_PKG_VERSION"));

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("FastZIP")
                    .size(10.0)
                    .strong()
                    .color(palette.primary),
            );

            ui.with_layout(
                Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    ui.label(
                        RichText::new(version_label)
                            .size(10.0)
                            .color(palette.text_muted),
                    );
                },
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                egui::ComboBox::from_id_salt("footer-language")
                    .selected_text(self.locale.display_name)
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for locale in supported_locales() {
                            if ui
                                .selectable_label(self.locale == locale, locale.display_name)
                                .clicked()
                            {
                                self.switch_locale(locale);
                            }
                        }
                    });
                ui.add_space(12.0);
                if footer_link(
                    ui,
                    self.t("Logs", "日志"),
                    self.side_nav == SideNavItem::Logs,
                    palette,
                ) {
                    self.set_side_nav(SideNavItem::Logs);
                }
                if footer_link(
                    ui,
                    self.t("Task Queue", "任务队列"),
                    self.side_nav == SideNavItem::Tasks,
                    palette,
                ) {
                    self.set_side_nav(SideNavItem::Tasks);
                }
            });
        });
    }

    fn draw_feedback_toast(&mut self, ctx: &Context) {
        let Some(toast) = self.toast.clone() else {
            return;
        };

        let elapsed = toast.created_at.elapsed();
        if elapsed >= toast.duration {
            self.toast = None;
            return;
        }

        let progress = (elapsed.as_secs_f32() / toast.duration.as_secs_f32()).clamp(0.0, 1.0);
        let fade_in = egui::emath::easing::quadratic_out((progress / 0.16).clamp(0.0, 1.0));
        let fade_out = egui::emath::easing::cubic_in(((1.0 - progress) / 0.22).clamp(0.0, 1.0));
        let alpha = fade_in.min(fade_out);
        let slide_y = (1.0 - fade_in.min(1.0)) * 12.0;
        let palette = self.palette();
        let (fill, stroke, text, dot) = toast_colors(toast.tone, palette);

        egui::Area::new(egui::Id::new("fastzip-feedback-toast"))
            .order(egui::Order::Foreground)
            .anchor(
                egui::Align2::RIGHT_TOP,
                Vec2::new(-24.0, TOP_BAR_HEIGHT + 16.0 + slide_y),
            )
            .show(ctx, |ui| {
                Frame::default()
                    .fill(fill.gamma_multiply(alpha.max(0.18)))
                    .stroke(Stroke::new(1.0, stroke.gamma_multiply(alpha.max(0.22))))
                    .corner_radius(12.0)
                    .inner_margin(Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (dot_rect, _) =
                                ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(
                                dot_rect.center(),
                                3.5,
                                dot.gamma_multiply(alpha),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(toast.text.clone())
                                    .size(12.0)
                                    .strong()
                                    .color(text.gamma_multiply(alpha.max(0.42))),
                            );
                        });
                    });
            });
    }

    fn draw_drop_overlay(&self, ctx: &Context) {
        let hovered_files = ctx.input(|input| input.raw.hovered_files.len());
        if hovered_files == 0 {
            return;
        }
        if !matches!(self.side_nav, SideNavItem::Compress | SideNavItem::Extract) {
            return;
        }

        let palette = self.palette();
        let (title, detail) = match self.side_nav {
            SideNavItem::Compress => (
                self.t(
                    "Drop files or folders to compress",
                    "拖入文件或文件夹以压缩",
                ),
                self.t(
                    "All dropped local paths will be added as compression sources",
                    "拖入的本地文件和文件夹都会加入压缩源",
                ),
            ),
            SideNavItem::Extract => (
                self.t(
                    "Drop one archive file to extract",
                    "拖入一个压缩包文件以解压",
                ),
                self.t(
                    "Only one supported archive file can be used at a time",
                    "一次只能使用一个受支持的压缩包文件",
                ),
            ),
            SideNavItem::FileManager
            | SideNavItem::Tasks
            | SideNavItem::Logs
            | SideNavItem::Settings => return,
        };

        egui::Area::new(egui::Id::new("fastzip-drop-overlay"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                Frame::default()
                    .fill(palette.panel_fill.gamma_multiply(if palette.is_dark {
                        1.0
                    } else {
                        0.98
                    }))
                    .stroke(Stroke::new(1.5, palette.primary_soft_stroke))
                    .corner_radius(18.0)
                    .inner_margin(Margin::symmetric(22, 18))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(title).size(18.0).strong().color(palette.text));
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(detail)
                                    .size(12.0)
                                    .color(palette.text_secondary),
                            );
                        });
                    });
            });
    }
}

#[derive(Debug, Clone)]
enum ShellCompressionJobResult {
    Progress {
        processed_bytes: u64,
        bytes_per_second: f64,
    },
    Completed(CompressionReport),
    Canceled,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellCompressionState {
    Running,
    Canceling,
    Completed,
    Canceled,
    Failed,
}

struct ShellCompressionProgressApp {
    language: Language,
    theme_mode: ThemeMode,
    request: ShellCompressionRequest,
    receiver: Option<Receiver<ShellCompressionJobResult>>,
    cancel_flag: Arc<AtomicBool>,
    state: ShellCompressionState,
    total_bytes: u64,
    processed_bytes: u64,
    current_bytes_per_second: Option<f64>,
    started_at: Instant,
    finished_at: Option<Instant>,
    report: Option<CompressionReport>,
    error_message: Option<String>,
    #[cfg(target_os = "windows")]
    root_hwnd: Option<HWND>,
}

impl ShellCompressionProgressApp {
    fn new(cc: &eframe::CreationContext<'_>, request: ShellCompressionRequest) -> Self {
        #[cfg(target_os = "windows")]
        let root_hwnd = configure_native_window(cc);
        #[cfg(not(target_os = "windows"))]
        configure_native_window(cc);
        install_multilingual_fonts(&cc.egui_ctx);
        let service = ArchiveService::new();
        let language = Language::detect();
        let theme_mode = ThemeMode::detect();
        configure_theme(&cc.egui_ctx, theme_mode);
        let total_bytes = total_bytes_for_sources(&request.sources, &[]).unwrap_or(0);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let worker_request = request.clone();
        let worker_service = service.clone();
        let worker_cancel_flag = cancel_flag.clone();

        std::thread::spawn(move || {
            let mut processed_bytes = 0u64;
            let mut last_emit = Instant::now();
            let mut bytes_since_emit = 0u64;
            let progress_tx = tx.clone();
            let cancel_for_progress = worker_cancel_flag.clone();
            let mut progress = |delta: u64| {
                if cancel_for_progress.load(Ordering::Relaxed) {
                    return;
                }
                processed_bytes = processed_bytes.saturating_add(delta);
                bytes_since_emit = bytes_since_emit.saturating_add(delta);
                let now = Instant::now();
                if now.saturating_duration_since(last_emit) >= Duration::from_millis(120) {
                    let elapsed = now.saturating_duration_since(last_emit).as_secs_f64();
                    let bytes_per_second = if elapsed > 0.0 {
                        bytes_since_emit as f64 / elapsed
                    } else {
                        0.0
                    };
                    let _ = progress_tx.send(ShellCompressionJobResult::Progress {
                        processed_bytes,
                        bytes_per_second,
                    });
                    last_emit = now;
                    bytes_since_emit = 0;
                }
            };

            let job = match worker_service
                .compress_with_options_and_exclusions_and_progress_and_cancel(
                    &worker_request.sources,
                    &[],
                    &worker_request.output_path,
                    worker_request.options,
                    &mut progress,
                    &mut || worker_cancel_flag.load(Ordering::Relaxed),
                ) {
                Ok(report) => {
                    let elapsed = Instant::now()
                        .saturating_duration_since(last_emit)
                        .as_secs_f64();
                    let bytes_per_second = if elapsed > 0.0 {
                        bytes_since_emit as f64 / elapsed
                    } else {
                        0.0
                    };
                    let _ = tx.send(ShellCompressionJobResult::Progress {
                        processed_bytes,
                        bytes_per_second,
                    });
                    ShellCompressionJobResult::Completed(report)
                }
                Err(error) => {
                    if worker_cancel_flag.load(Ordering::Relaxed) {
                        ShellCompressionJobResult::Canceled
                    } else {
                        ShellCompressionJobResult::Failed(format!("{error:#}"))
                    }
                }
            };

            let _ = tx.send(job);
        });

        Self {
            language,
            theme_mode,
            request,
            receiver: Some(rx),
            cancel_flag,
            state: ShellCompressionState::Running,
            total_bytes,
            processed_bytes: 0,
            current_bytes_per_second: None,
            started_at: Instant::now(),
            finished_at: None,
            report: None,
            error_message: None,
            #[cfg(target_os = "windows")]
            root_hwnd,
        }
    }

    fn palette(&self) -> ThemePalette {
        theme_palette(self.theme_mode)
    }

    fn t(&self, english: &'static str, chinese: &'static str) -> &'static str {
        self.language.text(english, chinese)
    }

    fn status_text(&self) -> String {
        match self.state {
            ShellCompressionState::Running => self.t("Compressing...", "正在压缩...").to_string(),
            ShellCompressionState::Canceling => self.t("Canceling...", "正在取消...").to_string(),
            ShellCompressionState::Completed => {
                self.t("Compression complete.", "压缩完成。").to_string()
            }
            ShellCompressionState::Canceled => {
                self.t("Compression canceled.", "压缩已取消。").to_string()
            }
            ShellCompressionState::Failed => {
                self.t("Compression failed.", "压缩失败。").to_string()
            }
        }
    }

    fn detail_text(&self) -> String {
        match self.state {
            ShellCompressionState::Running | ShellCompressionState::Canceling => {
                if self.total_bytes == 0 {
                    self.t("Preparing compression task...", "正在准备压缩任务...")
                        .to_string()
                } else {
                    format!(
                        "{} / {}",
                        format_size(Some(self.processed_bytes)),
                        format_size(Some(self.total_bytes))
                    )
                }
            }
            ShellCompressionState::Completed => self
                .report
                .as_ref()
                .map(|report| {
                    format!(
                        "{} files, {} dirs  ·  {} → {}",
                        report.files_added,
                        report.directories_added,
                        format_size(Some(report.input_bytes)),
                        format_size(Some(report.output_bytes)),
                    )
                })
                .unwrap_or_default(),
            ShellCompressionState::Canceled => self.request.output_path.display().to_string(),
            ShellCompressionState::Failed => self
                .error_message
                .clone()
                .unwrap_or_else(|| self.t("Unknown error", "未知错误").to_string()),
        }
    }

    fn progress_tone(&self) -> FeedbackTone {
        match self.state {
            ShellCompressionState::Completed => FeedbackTone::Success,
            ShellCompressionState::Failed => FeedbackTone::Error,
            ShellCompressionState::Running
            | ShellCompressionState::Canceling
            | ShellCompressionState::Canceled => FeedbackTone::Info,
        }
    }

    fn open_output_folder(&self) {
        let folder = self
            .report
            .as_ref()
            .and_then(|report| report.archive_path.parent().map(Path::to_path_buf))
            .or_else(|| self.request.output_path.parent().map(Path::to_path_buf));
        if let Some(folder) = folder {
            let _ = open_path_with_system_default(&folder);
        }
    }

    fn draw_top_bar(&self, ui: &mut egui::Ui) {
        let palette = self.palette();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;

            ui.label(
                RichText::new("FastZIP")
                    .size(18.0)
                    .strong()
                    .color(palette.text),
            );

            live_status_chip(
                ui,
                &self.status_text(),
                matches!(
                    self.state,
                    ShellCompressionState::Running | ShellCompressionState::Canceling
                ),
                self.progress_tone(),
                palette,
            );

            let controls_width = 92.0;
            let drag_width = (ui.available_width() - controls_width).max(0.0);
            let (_, drag_response) =
                ui.allocate_exact_size(Vec2::new(drag_width, 34.0), egui::Sense::drag());
            if drag_response.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }

            ui.allocate_ui_with_layout(
                Vec2::new(controls_width, 34.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if ui
                        .add_sized([40.0, 30.0], chrome_button("-", false, palette))
                        .clicked()
                    {
                        self.minimize_window(ui.ctx());
                    }
                    if ui
                        .add_sized([40.0, 30.0], chrome_button("x", true, palette))
                        .clicked()
                    {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                },
            );
        });
    }

    #[cfg(target_os = "windows")]
    fn minimize_window(&self, ctx: &Context) {
        if let Some(hwnd) = self.root_hwnd {
            unsafe {
                configure_taskbar_window(hwnd);
                let _ = ShowWindow(hwnd, SW_SHOWMINIMIZED);
            }
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }

    #[cfg(not(target_os = "windows"))]
    fn minimize_window(&self, ctx: &Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }

    fn poll_jobs(&mut self, ctx: &Context) {
        let Some(receiver) = self.receiver.as_ref() else {
            return;
        };

        loop {
            match receiver.try_recv() {
                Ok(ShellCompressionJobResult::Progress {
                    processed_bytes,
                    bytes_per_second,
                }) => {
                    self.processed_bytes =
                        processed_bytes.min(self.total_bytes.max(processed_bytes));
                    self.current_bytes_per_second =
                        (bytes_per_second > 0.0).then_some(bytes_per_second);
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                Ok(ShellCompressionJobResult::Completed(report)) => {
                    self.receiver = None;
                    self.processed_bytes = self.total_bytes.max(self.processed_bytes);
                    self.current_bytes_per_second = completed_shell_task_rate(
                        self.total_bytes,
                        self.started_at,
                        Instant::now(),
                    );
                    self.finished_at = Some(Instant::now());
                    self.report = Some(report);
                    self.state = ShellCompressionState::Completed;
                    break;
                }
                Ok(ShellCompressionJobResult::Canceled) => {
                    self.receiver = None;
                    self.finished_at = Some(Instant::now());
                    self.state = ShellCompressionState::Canceled;
                    break;
                }
                Ok(ShellCompressionJobResult::Failed(message)) => {
                    self.receiver = None;
                    self.finished_at = Some(Instant::now());
                    self.error_message = Some(message);
                    self.state = ShellCompressionState::Failed;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    if matches!(
                        self.state,
                        ShellCompressionState::Running | ShellCompressionState::Canceling
                    ) {
                        self.finished_at = Some(Instant::now());
                        self.state = ShellCompressionState::Failed;
                        self.error_message = Some(
                            self.t(
                                "Compression worker disconnected unexpectedly.",
                                "压缩后台任务意外断开。",
                            )
                            .to_string(),
                        );
                    }
                    break;
                }
            }
        }
    }

    fn request_cancel(&mut self) {
        if !matches!(self.state, ShellCompressionState::Running) {
            return;
        }
        self.cancel_flag.store(true, Ordering::Relaxed);
        self.state = ShellCompressionState::Canceling;
    }

    fn maybe_cancel_window_close(&mut self, ctx: &Context) {
        if ctx.input(|input| input.viewport().close_requested())
            && matches!(
                self.state,
                ShellCompressionState::Running | ShellCompressionState::Canceling
            )
        {
            self.request_cancel();
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }
}

impl eframe::App for ShellCompressionProgressApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        configure_theme(ctx, self.theme_mode);
        self.maybe_cancel_window_close(ctx);
        self.poll_jobs(ctx);

        let palette = self.palette();
        let progress = if self.total_bytes == 0 {
            0.0
        } else {
            (self.processed_bytes as f32 / self.total_bytes as f32).clamp(0.0, 1.0)
        };
        let is_final = matches!(
            self.state,
            ShellCompressionState::Completed
                | ShellCompressionState::Canceled
                | ShellCompressionState::Failed
        );
        let elapsed = self
            .finished_at
            .unwrap_or_else(Instant::now)
            .saturating_duration_since(self.started_at);

        TopBottomPanel::top("shell-top-bar")
            .exact_height(SHELL_TOP_BAR_HEIGHT)
            .frame(
                Frame::default()
                    .fill(palette.background)
                    .stroke(Stroke::NONE)
                    .inner_margin(Margin::symmetric(16, 10)),
            )
            .show(ctx, |ui| self.draw_top_bar(ui));

        TopBottomPanel::bottom("shell-footer")
            .exact_height(SHELL_FOOTER_HEIGHT)
            .frame(
                Frame::default()
                    .fill(palette.background)
                    .stroke(Stroke::NONE)
                    .inner_margin(Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            matches!(self.state, ShellCompressionState::Running),
                            Button::new(self.t("Cancel", "取消")).min_size(Vec2::new(110.0, 34.0)),
                        )
                        .clicked()
                    {
                        self.request_cancel();
                    }

                    if ui
                        .add_enabled(
                            matches!(self.state, ShellCompressionState::Completed),
                            Button::new(self.t("Open Folder", "打开目录"))
                                .min_size(Vec2::new(124.0, 34.0)),
                        )
                        .clicked()
                    {
                        self.open_output_folder();
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                is_final,
                                Button::new(self.t("Close", "关闭"))
                                    .min_size(Vec2::new(110.0, 34.0)),
                            )
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
            });

        CentralPanel::default()
            .frame(
                Frame::default()
                    .fill(palette.background)
                    .inner_margin(Margin {
                        left: 18,
                        right: 18,
                        top: 14,
                        bottom: 12,
                    }),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    Frame::default()
                        .fill(palette.surface_low)
                        .stroke(Stroke::new(1.0, palette.panel_stroke))
                        .corner_radius(16.0)
                        .inner_margin(Margin::same(18))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(
                                    self.t("FastZIP Background Compression", "FastZIP 后台压缩"),
                                )
                                .size(22.0)
                                .strong()
                                .color(palette.text),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(self.t("Output archive", "输出文件"))
                                    .size(12.0)
                                    .strong()
                                    .color(palette.text_muted),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.request.output_path.display().to_string())
                                    .size(13.0)
                                    .color(palette.text_secondary),
                            );
                            ui.add_space(12.0);
                            ui.label(RichText::new(self.status_text()).size(15.5).strong().color(
                                match self.state {
                                    ShellCompressionState::Failed => palette.error,
                                    ShellCompressionState::Completed => palette.primary,
                                    _ => palette.text,
                                },
                            ));
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(self.detail_text())
                                    .size(12.5)
                                    .color(palette.text_secondary),
                            );
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(format!(
                                    "{}  ·  {}",
                                    self.t("Format", "格式"),
                                    self.request.options.format.label()
                                ))
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                            ui.add_space(10.0);
                            ui.add(
                                ProgressBar::new(anim_value(
                                    ui.ctx(),
                                    egui::Id::new("shell-compress-progress"),
                                    progress,
                                    ANIM_FAST,
                                ))
                                .desired_width(f32::INFINITY)
                                .show_percentage(),
                            );
                            ui.add_space(12.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(18.0, 8.0);
                                ui.label(
                                    RichText::new(format!(
                                        "{}: {}",
                                        self.t("Sources", "源项目"),
                                        self.request.sources.len()
                                    ))
                                    .size(12.0)
                                    .color(palette.text_secondary),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{}: {}",
                                        self.t("Speed", "速度"),
                                        self.current_bytes_per_second
                                            .map(format_transfer_rate)
                                            .unwrap_or_else(|| "--".to_string())
                                    ))
                                    .size(12.0)
                                    .color(palette.text_secondary),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{}: {}",
                                        self.t("Elapsed", "耗时"),
                                        format_duration_compact(elapsed)
                                    ))
                                    .size(12.0)
                                    .color(palette.text_secondary),
                                );
                            });
                        });
                });
            });

        if matches!(
            self.state,
            ShellCompressionState::Running | ShellCompressionState::Canceling
        ) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette().background.to_normalized_gamma_f32()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }
}

impl eframe::App for FastZipGui {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        configure_theme(ctx, self.theme_mode);
        self.process_launch_request();
        self.refresh_dialog_click_guard(ctx);
        self.enforce_viewport_aspect_ratio(ctx);
        self.poll_scan_jobs(ctx);
        self.poll_task_jobs(ctx);
        self.poll_benchmark_jobs();
        self.handle_dropped_files(ctx);
        let palette = self.palette();

        TopBottomPanel::top("top-bar")
            .exact_height(TOP_BAR_HEIGHT)
            .frame(top_bar_frame(palette))
            .show(ctx, |ui| self.draw_top_bar(ui));

        TopBottomPanel::bottom("footer")
            .exact_height(FOOTER_HEIGHT)
            .frame(footer_frame(palette))
            .show(ctx, |ui| self.draw_footer(ui));

        SidePanel::left("side-nav")
            .exact_width(SIDE_NAV_WIDTH)
            .frame(side_nav_frame(palette))
            .show(ctx, |ui| self.draw_side_nav(ui));

        CentralPanel::default()
            .frame(canvas_frame(palette))
            .show(ctx, |ui| self.draw_current_page(ui));

        self.draw_drop_overlay(ctx);
        self.draw_feedback_toast(ctx);
        self.draw_task_output_path_dialog(ctx);
        self.draw_selected_file_manager_paths_dialog(ctx);
        self.draw_checksum_dialog(ctx);
        self.draw_save_preset_dialog(ctx);
        self.draw_manage_presets_dialog(ctx);
        self.draw_test_result_dialog(ctx);
        self.draw_extract_conflict_dialog(ctx);
        self.draw_compress_source_picker(ctx);
        self.process_pending_dialog_action();

        if self.needs_live_animation() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette().background.to_normalized_gamma_f32()
    }
}

#[cfg(target_os = "windows")]
fn configure_native_window(cc: &eframe::CreationContext<'_>) -> Option<HWND> {
    let Ok(window_handle) = cc.window_handle() else {
        return None;
    };

    let RawWindowHandle::Win32(handle) = window_handle.as_raw() else {
        return None;
    };

    let hwnd = handle.hwnd.get() as HWND;
    let preference = DWMWCP_ROUND;

    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &preference as *const _ as _,
            std::mem::size_of_val(&preference) as u32,
        );

        let margins = DwmMargins {
            cxLeftWidth: 1,
            cxRightWidth: 1,
            cyTopHeight: 1,
            cyBottomHeight: 1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

        configure_taskbar_window(hwnd);
        install_custom_frame_hook(hwnd);
    }

    Some(hwnd)
}

#[cfg(target_os = "windows")]
unsafe fn configure_taskbar_window(hwnd: HWND) {
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let desired_ex_style = (ex_style | WS_EX_APPWINDOW as isize) & !(WS_EX_TOOLWINDOW as isize);

    if ex_style != desired_ex_style {
        let _ = unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired_ex_style) };
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )
        };
    }
}

#[cfg(target_os = "windows")]
unsafe fn install_custom_frame_hook(hwnd: HWND) {
    unsafe {
        ensure_resizable_border(hwnd);
    }

    if ROOT_HWND_FOR_HIT_TEST.load(Ordering::Acquire) == hwnd as isize
        && ROOT_PREV_WNDPROC.load(Ordering::Acquire) != 0
    {
        return;
    }

    let previous = unsafe {
        SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            custom_frame_wnd_proc as unsafe extern "system" fn(_, _, _, _) -> _ as isize,
        )
    };

    if previous != 0 {
        ROOT_HWND_FOR_HIT_TEST.store(hwnd as isize, Ordering::Release);
        ROOT_PREV_WNDPROC.store(previous, Ordering::Release);
    }
}

#[cfg(target_os = "windows")]
unsafe fn ensure_resizable_border(hwnd: HWND) {
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    let desired_style = style | WS_THICKFRAME;

    if style != desired_style {
        let _ = unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, desired_style as isize) };
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )
        };
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn custom_frame_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => {
            return 1;
        }
        WM_NCCALCSIZE => {
            if wparam != 0 {
                return 0;
            }
        }
        WM_NCHITTEST => {
            let mut dwm_result: LRESULT = 0;
            if unsafe { DwmDefWindowProc(hwnd, msg, wparam, lparam, &mut dwm_result) } != 0 {
                return dwm_result;
            }

            if let Some(hit) = unsafe { custom_frame_hit_test(hwnd, lparam) } {
                return hit;
            }
        }
        WM_NCDESTROY => {
            let previous = take_previous_wnd_proc(hwnd);
            return unsafe { call_previous_wnd_proc(previous, hwnd, msg, wparam, lparam) };
        }
        _ => {}
    }

    let previous = current_previous_wnd_proc();
    unsafe { call_previous_wnd_proc(previous, hwnd, msg, wparam, lparam) }
}

#[cfg(target_os = "windows")]
unsafe fn custom_frame_hit_test(hwnd: HWND, lparam: LPARAM) -> Option<LRESULT> {
    if ROOT_HWND_FOR_HIT_TEST.load(Ordering::Acquire) != hwnd as isize {
        return None;
    }

    let mut window_rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut window_rect) } == 0 {
        return None;
    }

    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let border_x = unsafe { GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) }
        + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    let border_y = unsafe { GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) }
        + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    let cursor_x = signed_loword(lparam);
    let cursor_y = signed_hiword(lparam);

    let on_left = cursor_x < window_rect.left + border_x;
    let on_right = cursor_x >= window_rect.right - border_x;
    let on_top = cursor_y < window_rect.top + border_y;
    let on_bottom = cursor_y >= window_rect.bottom - border_y;

    let hit = match (on_left, on_right, on_top, on_bottom) {
        (true, _, true, _) => Some(HTTOPLEFT as LRESULT),
        (_, true, true, _) => Some(HTTOPRIGHT as LRESULT),
        (true, _, _, true) => Some(HTBOTTOMLEFT as LRESULT),
        (_, true, _, true) => Some(HTBOTTOMRIGHT as LRESULT),
        (_, _, true, _) => Some(HTTOP as LRESULT),
        (_, _, _, true) => Some(HTBOTTOM as LRESULT),
        (true, _, _, _) => Some(HTLEFT as LRESULT),
        (_, true, _, _) => Some(HTRIGHT as LRESULT),
        _ => None,
    };

    if hit.is_some() {
        return hit;
    }

    let top_bar_bottom = window_rect.top + scale_for_dpi(TOP_BAR_HEIGHT, dpi);
    let controls_left_edge = window_rect.right - scale_for_dpi(TOP_BAR_CONTROL_REGION_WIDTH, dpi);
    if cursor_y < top_bar_bottom && cursor_x < controls_left_edge {
        return Some(HTCAPTION as LRESULT);
    }

    Some(HTCLIENT as LRESULT)
}

#[cfg(target_os = "windows")]
fn signed_loword(lparam: LPARAM) -> i32 {
    (lparam as u32 & 0xffff) as i16 as i32
}

#[cfg(target_os = "windows")]
fn signed_hiword(lparam: LPARAM) -> i32 {
    ((lparam as u32 >> 16) & 0xffff) as i16 as i32
}

#[cfg(target_os = "windows")]
fn scale_for_dpi(value: f32, dpi: u32) -> i32 {
    ((value * dpi as f32) / 96.0).round() as i32
}

#[cfg(target_os = "windows")]
fn current_previous_wnd_proc() -> WNDPROC {
    let previous = ROOT_PREV_WNDPROC.load(Ordering::Acquire);
    if previous == 0 {
        None
    } else {
        Some(unsafe { transmute(previous) })
    }
}

#[cfg(target_os = "windows")]
fn take_previous_wnd_proc(hwnd: HWND) -> WNDPROC {
    if ROOT_HWND_FOR_HIT_TEST.load(Ordering::Acquire) == hwnd as isize {
        ROOT_HWND_FOR_HIT_TEST.store(0, Ordering::Release);
    }

    let previous = ROOT_PREV_WNDPROC.swap(0, Ordering::AcqRel);
    if previous == 0 {
        None
    } else {
        unsafe {
            let _ = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, previous);
        }
        Some(unsafe { transmute(previous) })
    }
}

#[cfg(target_os = "windows")]
unsafe fn call_previous_wnd_proc(
    previous: WNDPROC,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match previous {
        Some(proc) => unsafe { CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam) },
        None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(not(target_os = "windows"))]
fn configure_native_window(_cc: &eframe::CreationContext<'_>) {}

fn configure_theme(ctx: &Context, theme_mode: ThemeMode) {
    let palette = theme_palette(theme_mode);
    let mut visuals = if theme_mode.is_dark() {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.override_text_color = Some(palette.text);
    visuals.widgets.noninteractive.bg_fill = palette.panel_fill;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.panel_stroke);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text_secondary);
    visuals.widgets.inactive.bg_fill = palette.surface_high;
    visuals.widgets.hovered.bg_fill = palette.surface_highest;
    visuals.widgets.active.bg_fill = palette.surface_variant;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.outline_variant);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.outline);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.outline);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text_secondary);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text_secondary);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text_secondary);
    visuals.selection.bg_fill = palette.primary;
    visuals.window_fill = palette.background;
    visuals.panel_fill = palette.background;
    visuals.faint_bg_color = palette.surface_low;
    visuals.extreme_bg_color = palette.surface_low;

    ctx.set_visuals(visuals);
}

fn application_icon() -> Arc<egui::IconData> {
    Arc::new(
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/fastzip-icon.png"))
            .expect("embedded FastZIP icon should be a valid PNG"),
    )
}

fn theme_palette(theme_mode: ThemeMode) -> ThemePalette {
    match theme_mode {
        ThemeMode::Light => ThemePalette {
            is_dark: false,
            background: Color32::from_rgb(255, 255, 255),
            panel_fill: Color32::from_rgb(255, 255, 255),
            surface_low: Color32::from_rgb(246, 243, 245),
            surface_high: Color32::from_rgb(240, 237, 239),
            surface_highest: Color32::from_rgb(234, 231, 234),
            surface_variant: Color32::from_rgb(228, 226, 228),
            panel_stroke: Color32::from_rgb(236, 240, 245),
            outline_variant: Color32::from_rgba_unmultiplied(193, 198, 215, 90),
            outline: Color32::from_rgb(113, 119, 134),
            text: Color32::from_rgb(27, 27, 29),
            text_secondary: Color32::from_rgb(65, 71, 85),
            text_muted: Color32::from_rgb(100, 116, 139),
            primary: Color32::from_rgb(0, 88, 188),
            primary_strong: Color32::from_rgb(0, 88, 188),
            primary_soft_fill: Color32::from_rgba_unmultiplied(0, 112, 235, 22),
            primary_soft_stroke: Color32::from_rgba_unmultiplied(0, 112, 235, 76),
            on_primary: Color32::WHITE,
            tertiary: Color32::from_rgb(0, 101, 123),
            error: Color32::from_rgb(186, 26, 26),
            danger_fill: Color32::from_rgb(255, 245, 245),
            danger_stroke: Color32::from_rgb(254, 226, 226),
            subtle_fill: Color32::from_rgb(248, 250, 252),
            subtle_stroke: Color32::from_rgb(226, 232, 240),
            nav_active_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 235),
            nav_active_stroke: Color32::from_rgba_unmultiplied(255, 255, 255, 180),
            nav_active_text: Color32::from_rgb(0, 88, 188),
            nav_inactive_text: Color32::from_rgb(97, 104, 118),
            badge_fill: Color32::from_rgba_unmultiplied(223, 223, 228, 255),
            badge_text: Color32::from_rgb(70, 71, 75),
            log_fill: Color32::from_rgba_unmultiplied(48, 48, 50, 220),
            log_stroke: Color32::from_rgba_unmultiplied(193, 198, 215, 48),
            log_text: Color32::from_rgb(243, 240, 242),
            log_index: Color32::from_rgb(148, 163, 184),
            background_glow_primary: Color32::from_rgba_unmultiplied(0, 112, 235, 8),
            background_glow_secondary: Color32::from_rgba_unmultiplied(255, 255, 255, 0),
        },
        ThemeMode::Dark => ThemePalette {
            is_dark: true,
            background: Color32::from_rgb(18, 19, 22),
            panel_fill: Color32::from_rgba_unmultiplied(29, 31, 35, 235),
            surface_low: Color32::from_rgb(27, 27, 29),
            surface_high: Color32::from_rgb(40, 42, 45),
            surface_highest: Color32::from_rgb(51, 53, 56),
            surface_variant: Color32::from_rgb(69, 70, 79),
            panel_stroke: Color32::from_rgba_unmultiplied(69, 70, 79, 120),
            outline_variant: Color32::from_rgba_unmultiplied(69, 70, 79, 110),
            outline: Color32::from_rgb(143, 144, 153),
            text: Color32::from_rgb(228, 226, 228),
            text_secondary: Color32::from_rgb(197, 198, 208),
            text_muted: Color32::from_rgb(169, 169, 174),
            primary: Color32::from_rgb(173, 198, 255),
            primary_strong: Color32::from_rgb(0, 68, 147),
            primary_soft_fill: Color32::from_rgba_unmultiplied(173, 198, 255, 24),
            primary_soft_stroke: Color32::from_rgba_unmultiplied(173, 198, 255, 86),
            on_primary: Color32::from_rgb(216, 226, 255),
            tertiary: Color32::from_rgb(71, 214, 255),
            error: Color32::from_rgb(255, 180, 171),
            danger_fill: Color32::from_rgb(147, 0, 10),
            danger_stroke: Color32::from_rgb(255, 180, 171),
            subtle_fill: Color32::from_rgb(51, 53, 56),
            subtle_stroke: Color32::from_rgba_unmultiplied(69, 70, 79, 120),
            nav_active_fill: Color32::from_rgb(18, 19, 22),
            nav_active_stroke: Color32::from_rgba_unmultiplied(69, 70, 79, 80),
            nav_active_text: Color32::from_rgb(173, 198, 255),
            nav_inactive_text: Color32::from_rgb(197, 198, 208),
            badge_fill: Color32::from_rgb(69, 71, 75),
            badge_text: Color32::from_rgb(226, 226, 231),
            log_fill: Color32::from_rgba_unmultiplied(228, 226, 228, 230),
            log_stroke: Color32::from_rgba_unmultiplied(69, 70, 79, 96),
            log_text: Color32::from_rgb(48, 48, 50),
            log_index: Color32::from_rgb(97, 104, 118),
            background_glow_primary: Color32::from_rgba_unmultiplied(173, 198, 255, 18),
            background_glow_secondary: Color32::from_rgba_unmultiplied(71, 214, 255, 10),
        },
    }
}

fn install_multilingual_fonts(ctx: &Context) {
    let loaded_fonts = load_multilingual_fonts();
    if loaded_fonts.is_empty() {
        return;
    }

    let mut fonts = egui::FontDefinitions::default();
    let mut font_names = Vec::new();

    for (font_name, font_bytes) in loaded_fonts {
        fonts.font_data.insert(
            font_name.clone(),
            std::sync::Arc::new(egui::FontData::from_owned(font_bytes)),
        );
        font_names.push(font_name);
    }

    if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        for font_name in font_names.iter().rev() {
            proportional.insert(0, font_name.clone());
        }
    }
    if let Some(monospace) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        for font_name in font_names.iter().rev() {
            monospace.insert(0, font_name.clone());
        }
    }

    ctx.set_fonts(fonts);
}

fn top_bar_frame(palette: ThemePalette) -> Frame {
    Frame::default()
        .fill(palette.background)
        .stroke(Stroke::NONE)
        .inner_margin(Margin::symmetric(16, 10))
}

fn side_nav_frame(palette: ThemePalette) -> Frame {
    Frame::default()
        .fill(palette.background)
        .stroke(Stroke::NONE)
        .inner_margin(Margin::same(16))
}

fn footer_frame(palette: ThemePalette) -> Frame {
    Frame::default()
        .fill(palette.background)
        .stroke(Stroke::NONE)
        .inner_margin(Margin::symmetric(16, 8))
}

fn canvas_frame(palette: ThemePalette) -> Frame {
    Frame::default()
        .fill(palette.background)
        .inner_margin(Margin {
            left: MAIN_PADDING as i8,
            right: MAIN_PADDING as i8,
            top: 20,
            bottom: 16,
        })
}

fn glass_panel(fill: Color32, palette: ThemePalette) -> Frame {
    Frame::default()
        .fill(fill)
        .stroke(Stroke::new(1.0, palette.panel_stroke))
        .corner_radius(14.0)
        .inner_margin(Margin::same(20))
}

fn compact_glass_panel(fill: Color32, palette: ThemePalette) -> Frame {
    Frame::default()
        .fill(fill)
        .stroke(Stroke::new(1.0, palette.panel_stroke))
        .corner_radius(14.0)
        .inner_margin(Margin {
            left: 18,
            right: 18,
            top: 12,
            bottom: 10,
        })
}

fn log_frame(palette: ThemePalette) -> Frame {
    Frame::default()
        .fill(palette.log_fill)
        .stroke(Stroke::new(1.0, palette.log_stroke))
        .corner_radius(12.0)
        .inner_margin(Margin::same(12))
}

fn chrome_button(label: &'static str, danger: bool, palette: ThemePalette) -> Button<'static> {
    Button::new(RichText::new(label).size(15.0).strong().color(if danger {
        palette.error
    } else {
        palette.text_secondary
    }))
    .fill(if danger {
        palette.danger_fill
    } else {
        palette.subtle_fill
    })
    .stroke(Stroke::new(
        1.0,
        if danger {
            palette.danger_stroke
        } else {
            palette.subtle_stroke
        },
    ))
    .corner_radius(10.0)
    .min_size(Vec2::new(36.0, 30.0))
}

fn live_status_chip(
    ui: &mut egui::Ui,
    label: &str,
    busy: bool,
    tone: FeedbackTone,
    palette: ThemePalette,
) {
    let (fill, stroke, text, dot) = toast_colors(tone, palette);
    let active = anim_bool(ui.ctx(), egui::Id::new("status-chip-pulse"), busy, ANIM_NORMAL);
    let pulse = if active > 0.001 {
        let time = ui.ctx().input(|input| input.time) as f32;
        0.7 + (0.5 + 0.5 * (time * 3.0).sin()) * 0.6 * active
    } else {
        1.0
    };

    Frame::default()
        .fill(fill.gamma_multiply(if palette.is_dark { 1.0 } else { 0.95 }))
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(999.0)
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (dot_rect, _) =
                    ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                ui.painter().circle_filled(
                    dot_rect.center(),
                    if busy { 3.2 + pulse } else { 3.6 },
                    dot.gamma_multiply(pulse),
                );
                ui.add_space(4.0);
                ui.label(RichText::new(label).size(11.0).strong().color(text));
            });
        });
}

fn toast_colors(tone: FeedbackTone, palette: ThemePalette) -> (Color32, Color32, Color32, Color32) {
    match tone {
        FeedbackTone::Info => (
            palette.surface_highest,
            palette.outline_variant,
            palette.text,
            palette.primary,
        ),
        FeedbackTone::Success => (
            palette
                .primary_soft_fill
                .gamma_multiply(if palette.is_dark { 1.8 } else { 1.2 }),
            palette.primary_soft_stroke,
            if palette.is_dark {
                palette.on_primary
            } else {
                palette.primary_strong
            },
            if palette.is_dark {
                palette.tertiary
            } else {
                palette.primary
            },
        ),
        FeedbackTone::Error => (
            palette
                .danger_fill
                .gamma_multiply(if palette.is_dark { 1.0 } else { 0.9 }),
            palette.danger_stroke,
            if palette.is_dark {
                palette.error
            } else {
                palette.text
            },
            palette.error,
        ),
    }
}

fn action_card(
    ui: &mut egui::Ui,
    title: &str,
    glyph: &str,
    fill: Color32,
    stroke: Stroke,
    highlighted: bool,
    busy: bool,
    palette: ThemePalette,
) -> bool {
    let inner = Frame::default()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(16.0)
        .inner_margin(Margin::same(24))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                round_icon(ui, glyph, highlighted, busy, palette);
                ui.add_space(12.0);
                ui.label(RichText::new(title).size(24.0).strong().color(palette.text));
                ui.add_space(6.0);
            });
        });
    let id = ui.id().with(("action-card", title, glyph));
    let response = ui.interact(inner.response.rect, id, egui::Sense::click());
    let hover_t = ui.ctx().animate_bool(id.with("hover"), response.hovered());
    let press_t = ui
        .ctx()
        .animate_bool(id.with("press"), response.is_pointer_button_down_on());
    let busy_wave = if busy {
        let time = ui.ctx().input(|input| input.time) as f32;
        0.5 + 0.5 * (time * 4.8).sin()
    } else {
        0.0
    };
    let overlay_strength =
        (hover_t * 0.12 + press_t * 0.22 + busy_wave * 0.10 + if busy { 0.10 } else { 0.0 })
            .clamp(0.0, 0.32);
    if overlay_strength > 0.01 || response.clicked() {
        ui.painter().rect_filled(
            inner.response.rect.shrink(1.0),
            16.0,
            palette.primary_soft_fill.gamma_multiply(
                (overlay_strength + if response.clicked() { 0.18 } else { 0.0 }).clamp(0.0, 0.45),
            ),
        );
    }
    if response.hovered() || busy {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

fn round_icon(
    ui: &mut egui::Ui,
    glyph: &str,
    highlighted: bool,
    busy: bool,
    palette: ThemePalette,
) {
    let base_fill = if highlighted {
        if palette.is_dark {
            palette.primary
        } else {
            palette.primary_soft_fill
        }
    } else {
        palette.surface_highest
    };
    let text = if highlighted {
        if palette.is_dark {
            palette.on_primary
        } else {
            palette.primary_strong
        }
    } else {
        palette.text
    };
    let active = anim_bool(
        ui.ctx(),
        egui::Id::new(("round-icon-pulse", glyph)),
        busy,
        ANIM_NORMAL,
    );
    let pulse = if active > 0.001 {
        let time = ui.ctx().input(|input| input.time) as f32;
        1.02 + (0.5 + 0.5 * (time * 3.2).sin()) * 0.12 * active
    } else {
        1.0
    };
    let fill = base_fill.gamma_multiply(pulse);

    Frame::default()
        .fill(fill)
        .corner_radius(999.0)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.label(RichText::new(glyph).size(30.0).strong().color(text));
        });
}

fn draw_path_row(
    ui: &mut egui::Ui,
    label: &str,
    path_value: &mut String,
    browse_label: &str,
    palette: ThemePalette,
) -> bool {
    let mut clicked = false;
    let input_width = (ui.available_width() - 96.0).max(0.0);

    ui.label(
        RichText::new(label)
            .size(10.0)
            .strong()
            .color(palette.text_secondary),
    );
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_sized(
            [input_width, 28.0],
            TextEdit::singleline(path_value)
                .margin(Margin::symmetric(6, 4))
                .vertical_align(Align::Center),
        );
        if ui
            .add(
                Button::new(RichText::new(browse_label).size(11.0).color(palette.text))
                    .fill(palette.surface_highest)
                    .stroke(Stroke::new(1.0, palette.outline_variant))
                    .corner_radius(8.0)
                    .min_size(Vec2::new(80.0, 28.0)),
            )
            .clicked()
        {
            clicked = true;
        }
    });

    clicked
}

fn compression_option_combo(
    ui: &mut egui::Ui,
    label: &str,
    selected_text: &str,
    _palette: ThemePalette,
    add_items: impl FnOnce(&mut egui::Ui),
) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(10.5).strong());
        ui.add_space(3.0);
        egui::ComboBox::from_id_salt(label)
            .selected_text(selected_text)
            .width(220.0)
            .show_ui(ui, add_items);
    });
}

fn compression_option_static(ui: &mut egui::Ui, label: &str, value: &str, palette: ThemePalette) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(10.5)
                .strong()
                .color(palette.text_secondary),
        );
        ui.add_space(3.0);
        ui.add_sized(
            [220.0, 32.0],
            egui::Label::new(RichText::new(value).size(11.5).color(palette.text)).truncate(),
        );
    });
}

fn compression_option_password(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    enabled: bool,
    palette: ThemePalette,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(10.5)
                .strong()
                .color(palette.text_secondary),
        );
        ui.add_space(3.0);
        ui.add_enabled_ui(enabled, |ui| {
            ui.add_sized(
                [220.0, 28.0],
                TextEdit::singleline(value)
                    .password(true)
                    .margin(Margin::symmetric(6, 4))
                    .vertical_align(Align::Center),
            );
        });
    });
}

fn compression_option_split_volume_input(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    hint_text: &str,
    language: Language,
    palette: ThemePalette,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(10.5)
                .strong()
                .color(palette.text_secondary),
        );
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [150.0, 28.0],
                TextEdit::singleline(value)
                    .hint_text(hint_text)
                    .margin(Margin::symmetric(6, 4))
                    .vertical_align(Align::Center),
            );
            egui::ComboBox::from_id_salt((label, "split-volume-presets"))
                .selected_text(language.text("Presets", "预设"))
                .width(64.0)
                .show_ui(ui, |ui| {
                    for (preset_value, preset_label) in split_volume_size_presets(language) {
                        let is_selected = if preset_value.is_empty() {
                            value.trim().is_empty()
                        } else {
                            value.trim().eq_ignore_ascii_case(preset_value)
                        };
                        if ui.selectable_label(is_selected, preset_label).clicked() {
                            value.clear();
                            value.push_str(preset_value);
                            ui.close();
                        }
                    }
                });
        });
    });
}

fn compression_option_checkbox(
    ui: &mut egui::Ui,
    label: &str,
    description: &str,
    value: &mut bool,
    enabled: bool,
    palette: ThemePalette,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(10.5)
                .strong()
                .color(palette.text_secondary),
        );
        ui.add_space(3.0);
        ui.add_enabled_ui(enabled, |ui| {
            ui.checkbox(value, description);
        });
    });
}

fn compression_option_button(
    ui: &mut egui::Ui,
    label: &str,
    description: &str,
    button_text: &str,
    palette: ThemePalette,
    clicked: &mut bool,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(10.5)
                .strong()
                .color(palette.text_secondary),
        );
        ui.add_space(3.0);
        if ui
            .add(
                Button::new(RichText::new(button_text).color(palette.text))
                    .fill(palette.surface_highest)
                    .stroke(Stroke::new(1.0, palette.outline_variant))
                    .corner_radius(8.0)
                    .min_size(Vec2::new(220.0, 28.0)),
            )
            .clicked()
        {
            *clicked = true;
        }
        ui.add_space(2.0);
        ui.label(
            RichText::new(description)
                .size(10.0)
                .color(palette.text_secondary),
        );
    });
}

fn compression_option_drag_value(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
    palette: ThemePalette,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(10.5)
                .strong()
                .color(palette.text_secondary),
        );
        ui.add_space(3.0);
        ui.add_sized(
            [220.0, 28.0],
            egui::DragValue::new(value).range(range).speed(0.25),
        );
    });
}

fn compression_format_label(format: CompressionFormat, _language: Language) -> &'static str {
    match format {
        CompressionFormat::SevenZip => "7Z",
        CompressionFormat::Zip => "ZIP",
        CompressionFormat::Tar => "TAR",
        CompressionFormat::TarGz => "TAR.GZ",
        CompressionFormat::TarBz2 => "TAR.BZ2",
        CompressionFormat::TarXz => "TAR.XZ",
        CompressionFormat::Gz => "GZ",
        CompressionFormat::Bz2 => "BZ2",
        CompressionFormat::Xz => "XZ",
        CompressionFormat::Zst => "ZST",
        CompressionFormat::TarZst => "TAR.ZST",
        CompressionFormat::Lz4 => "LZ4",
        CompressionFormat::TarLz4 => "TAR.LZ4",
    }
}

fn default_compression_thread_count() -> u32 {
    std::thread::available_parallelism()
        .map(|value| value.get() as u32)
        .unwrap_or(1)
        .max(1)
}

fn split_volume_size_presets(language: Language) -> Vec<(&'static str, &'static str)> {
    vec![
        ("", language.text("No split volumes", "不分卷")),
        ("1.44M", language.text("1.44M (Floppy)", "1.44M（软盘）")),
        ("2.88M", language.text("2.88M (Floppy)", "2.88M（软盘）")),
        ("10M", "10M"),
        ("100M", "100M"),
        ("1G", "1G"),
        ("4G", "4G"),
        ("650M", language.text("650M (CD)", "650M（CD）")),
        ("700M", language.text("700M (CD)", "700M（CD）")),
        ("4092M", language.text("4092M (FAT32)", "4092M（FAT32）")),
        ("4480M", language.text("4480M (DVD)", "4480M（DVD）")),
        (
            "8128M",
            language.text("8128M (DVD DL)", "8128M（双层 DVD）"),
        ),
        (
            "23040M",
            language.text("23040M (Blu-ray)", "23040M（蓝光）"),
        ),
    ]
}

fn compression_level_label(level: CompressionLevel, language: Language) -> &'static str {
    match level {
        CompressionLevel::Fastest => language.text("Fastest", "极速"),
        CompressionLevel::Fast => language.text("Fast", "快速"),
        CompressionLevel::Normal => language.text("Normal", "标准"),
        CompressionLevel::Maximum => language.text("Maximum", "高压缩"),
        CompressionLevel::Ultra => language.text("Ultra", "极限"),
    }
}

fn zip_method_label(method: ZipCompressionMethod, language: Language) -> &'static str {
    match method {
        ZipCompressionMethod::Deflate => "Deflate",
        ZipCompressionMethod::Stored => language.text("Store", "仅存储"),
        ZipCompressionMethod::Bzip2 => "BZip2",
        ZipCompressionMethod::Zstd => "Zstandard",
        ZipCompressionMethod::Xz => "XZ",
    }
}

fn status_badge(ui: &mut egui::Ui, label: String, fill: Color32, text_color: Color32) {
    Frame::default()
        .fill(fill)
        .corner_radius(8.0)
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(11.0).strong().color(text_color));
        });
}

fn queue_header(ui: &mut egui::Ui, language: Language, palette: ThemePalette) {
    padded_header_cell(
        ui,
        language.text("Name", "名称"),
        CONTENT_NAME_COLUMN_WIDTH,
        CONTENT_NAME_HEADER_PADDING,
        palette,
    );
    padded_header_cell(
        ui,
        language.text("Size", "大小"),
        CONTENT_SIZE_COLUMN_WIDTH,
        CONTENT_SIZE_HEADER_PADDING,
        palette,
    );
    centered_header_cell(
        ui,
        language.text("Type", "类型"),
        CONTENT_TYPE_COLUMN_WIDTH,
        palette,
    );
    centered_header_cell(
        ui,
        language.text("Status", "状态"),
        CONTENT_STATUS_COLUMN_WIDTH,
        palette,
    );
    centered_header_cell(
        ui,
        language.text("Action", "操作"),
        CONTENT_ACTION_COLUMN_WIDTH,
        palette,
    );
    ui.end_row();
}

fn header_cell(ui: &mut egui::Ui, label: &str, width: f32, palette: ThemePalette) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 18.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.label(
                RichText::new(label)
                    .size(11.0)
                    .strong()
                    .color(palette.text_secondary),
            );
        },
    );
}

fn centered_header_cell(ui: &mut egui::Ui, label: &str, width: f32, palette: ThemePalette) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 18.0),
        Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| {
            ui.label(
                RichText::new(label)
                    .size(11.0)
                    .strong()
                    .color(palette.text_secondary),
            );
        },
    );
}

fn padded_header_cell(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    left_padding: f32,
    palette: ThemePalette,
) {
    padded_text_cell(
        ui,
        width,
        18.0,
        width - left_padding,
        left_padding,
        RichText::new(label)
            .size(11.0)
            .strong()
            .color(palette.text_secondary),
        false,
    );
}

fn padded_label_cell(
    ui: &mut egui::Ui,
    text: &str,
    width: f32,
    left_padding: f32,
    font_size: f32,
    color: Color32,
) {
    padded_text_cell(
        ui,
        width,
        36.0,
        width - left_padding,
        left_padding,
        RichText::new(text).size(font_size).color(color),
        false,
    );
}

fn padded_clickable_label_cell(
    ui: &mut egui::Ui,
    text: &str,
    width: f32,
    left_padding: f32,
    font_size: f32,
    color: Color32,
) -> egui::Response {
    padded_text_cell(
        ui,
        width,
        36.0,
        width - left_padding,
        left_padding,
        RichText::new(text).size(font_size).color(color),
        true,
    )
}

fn padded_text_cell(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    text_width: f32,
    left_padding: f32,
    rich_text: RichText,
    clickable: bool,
) -> egui::Response {
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.add_space(left_padding);
            let label = egui::Label::new(rich_text).truncate().sense(if clickable {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            });
            ui.add_sized([text_width, height], label)
        },
    )
    .inner
}

fn queue_row(
    ui: &mut egui::Ui,
    row: &QueueRowData,
    language: Language,
    palette: ThemePalette,
) -> Option<QueueRowTarget> {
    let name = egui::Label::new(RichText::new(&row.name).size(13.0).color(palette.text)).truncate();
    ui.add_sized([CONTENT_NAME_COLUMN_WIDTH, 22.0], name);
    ui.add_sized(
        [CONTENT_SIZE_COLUMN_WIDTH, 22.0],
        egui::Label::new(
            RichText::new(&row.size)
                .size(13.0)
                .color(palette.text_secondary),
        ),
    );
    ui.allocate_ui_with_layout(
        Vec2::new(CONTENT_TYPE_COLUMN_WIDTH, 22.0),
        Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| type_badge(ui, &row.kind, palette),
    );
    ui.allocate_ui_with_layout(
        Vec2::new(CONTENT_STATUS_COLUMN_WIDTH, 22.0),
        Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| draw_status_cell(ui, &row.status, language, palette),
    );
    let clicked = ui
        .allocate_ui_with_layout(
            Vec2::new(CONTENT_ACTION_COLUMN_WIDTH, 22.0),
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| draw_action_cell(ui, &row.action, language, palette),
        )
        .inner;
    ui.end_row();
    match clicked {
        QueueActionClick::Primary => Some(row.target.clone()),
        QueueActionClick::Secondary => row.secondary_target.clone(),
        QueueActionClick::None => None,
    }
}

fn task_queue_header(ui: &mut egui::Ui, language: Language, palette: ThemePalette) {
    header_cell(
        ui,
        language.text("Type", "类型"),
        TASK_TYPE_COLUMN_WIDTH,
        palette,
    );
    padded_header_cell(
        ui,
        language.text("Output Path", "输出路径"),
        TASK_OUTPUT_COLUMN_WIDTH,
        TASK_RIGHT_SHIFT_PADDING,
        palette,
    );
    header_cell(
        ui,
        language.text("Progress", "进度"),
        TASK_PROGRESS_COLUMN_WIDTH,
        palette,
    );
    padded_header_cell(
        ui,
        language.text("Speed", "速率"),
        TASK_SPEED_COLUMN_WIDTH,
        TASK_RIGHT_SHIFT_PADDING,
        palette,
    );
    padded_header_cell(
        ui,
        language.text("ETA", "预计时间"),
        TASK_ETA_COLUMN_WIDTH,
        TASK_RIGHT_SHIFT_PADDING,
        palette,
    );
    header_cell(
        ui,
        language.text("Actions", "操作"),
        TASK_ACTIONS_COLUMN_WIDTH,
        palette,
    );
    ui.end_row();
}

fn task_queue_row(
    ui: &mut egui::Ui,
    task: &TaskQueueItem,
    metrics: &TaskQueueMetrics,
    language: Language,
    can_prioritize: bool,
    palette: ThemePalette,
) -> Option<TaskQueueRowEvent> {
    let full_output_path = task.spec.output_path().display().to_string();

    ui.allocate_ui_with_layout(
        Vec2::new(TASK_TYPE_COLUMN_WIDTH, 36.0),
        Layout::top_down(Align::Min),
        |ui| {
            ui.label(
                RichText::new(task.kind.label(language))
                    .size(13.0)
                    .strong()
                    .color(palette.text),
            );
            ui.label(
                RichText::new(&metrics.status_text)
                    .size(11.0)
                    .color(match task.state {
                        TaskState::Completed => palette.tertiary,
                        TaskState::Canceled => palette.text_muted,
                        TaskState::Canceling => palette.badge_text,
                        TaskState::Failed => palette.error,
                        TaskState::Queued | TaskState::Running => palette.text_secondary,
                    }),
            );
        },
    );

    let response = padded_clickable_label_cell(
        ui,
        &full_output_path,
        TASK_OUTPUT_COLUMN_WIDTH,
        TASK_RIGHT_SHIFT_PADDING,
        13.0,
        palette.text,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let response = response.on_hover_text(full_output_path.clone());
    let clicked_output_path = response.clicked();

    ui.allocate_ui_with_layout(
        Vec2::new(TASK_PROGRESS_COLUMN_WIDTH, 36.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            let fill = if task.state == TaskState::Failed {
                palette.error
            } else {
                palette.primary
            };
            ui.add(
                ProgressBar::new(anim_value(
                    ui.ctx(),
                    egui::Id::new(("task-progress", task.id)),
                    metrics.progress,
                    ANIM_FAST,
                ))
                .desired_width(TASK_PROGRESS_COLUMN_WIDTH - 12.0)
                .fill(match task.state {
                        TaskState::Canceled => palette.text_muted,
                        TaskState::Canceling => palette.badge_fill,
                        _ => fill,
                    })
                    .text(metrics.progress_text.clone()),
            );
        },
    );

    padded_label_cell(
        ui,
        &metrics.speed_text,
        TASK_SPEED_COLUMN_WIDTH,
        TASK_RIGHT_SHIFT_PADDING,
        12.0,
        palette.text_secondary,
    );
    padded_label_cell(
        ui,
        &metrics.eta_text,
        TASK_ETA_COLUMN_WIDTH,
        TASK_RIGHT_SHIFT_PADDING,
        12.0,
        palette.text_secondary,
    );
    let mut row_event =
        clicked_output_path.then_some(TaskQueueRowEvent::ShowOutputPath(full_output_path.clone()));

    ui.allocate_ui_with_layout(
        Vec2::new(TASK_ACTIONS_COLUMN_WIDTH, 36.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            match task.state {
                TaskState::Queued => {
                    let priority_label = if can_prioritize {
                        language.text("Run Next", "提前执行")
                    } else {
                        language.text("Next", "下一项")
                    };
                    if ui
                        .add_enabled(
                            can_prioritize,
                            task_action_button(priority_label, false, palette),
                        )
                        .clicked()
                    {
                        row_event = Some(TaskQueueRowEvent::Prioritize(task.id));
                    }
                    if ui
                        .add(task_action_button(
                            language.text("Cancel", "取消"),
                            true,
                            palette,
                        ))
                        .clicked()
                    {
                        row_event = Some(TaskQueueRowEvent::Cancel(task.id));
                    }
                }
                TaskState::Running => {
                    ui.add_enabled_ui(true, |ui| {
                        ui.add_enabled(
                            false,
                            task_action_button(language.text("Running", "执行中"), false, palette),
                        );
                    });
                    if ui
                        .add(task_action_button(
                            language.text("Cancel", "取消"),
                            true,
                            palette,
                        ))
                        .clicked()
                    {
                        row_event = Some(TaskQueueRowEvent::Cancel(task.id));
                    }
                }
                TaskState::Canceling => {
                    ui.add_enabled(
                        false,
                        task_action_button(language.text("Stopping", "停止中"), false, palette),
                    );
                    ui.add_enabled(
                        false,
                        task_action_button(language.text("Wait", "等待"), true, palette),
                    );
                }
                TaskState::Canceled | TaskState::Failed => {
                    if ui
                        .add(task_action_button(
                            language.text("Rerun", "重新运行"),
                            false,
                            palette,
                        ))
                        .clicked()
                    {
                        row_event = Some(TaskQueueRowEvent::Rerun(task.id));
                    }
                    if ui
                        .add(task_action_button(
                            language.text("Delete", "删除"),
                            true,
                            palette,
                        ))
                        .clicked()
                    {
                        row_event = Some(TaskQueueRowEvent::Delete(task.id));
                    }
                }
                TaskState::Completed => {
                    if ui
                        .add(task_action_button(
                            language.text("Open Folder", "打开目录"),
                            false,
                            palette,
                        ))
                        .clicked()
                    {
                        row_event = Some(TaskQueueRowEvent::OpenOutputDir(task.id));
                    }
                    if ui
                        .add(task_action_button(
                            language.text("Delete", "删除"),
                            true,
                            palette,
                        ))
                        .clicked()
                    {
                        row_event = Some(TaskQueueRowEvent::Delete(task.id));
                    }
                }
            }
        },
    );
    ui.end_row();
    row_event
}

fn type_badge(ui: &mut egui::Ui, label: &str, palette: ThemePalette) {
    ui.add(
        Button::new(RichText::new(label).size(11.0).color(palette.text))
            .sense(egui::Sense::hover())
            .fill(palette.surface_high)
            .stroke(Stroke::new(1.0, palette.outline_variant))
            .corner_radius(8.0)
            .min_size(Vec2::new(70.0, 22.0)),
    );
}

fn task_action_button(label: &str, danger: bool, palette: ThemePalette) -> Button<'static> {
    let text = if danger { palette.error } else { palette.text };
    let fill = if danger {
        palette
            .danger_fill
            .gamma_multiply(if palette.is_dark { 0.85 } else { 1.0 })
    } else {
        palette.surface_high
    };
    let stroke = if danger {
        palette.danger_stroke.gamma_multiply(0.85)
    } else {
        palette.outline_variant
    };

    Button::new(
        RichText::new(label.to_string())
            .size(11.0)
            .strong()
            .color(text),
    )
    .fill(fill)
    .stroke(Stroke::new(1.0, stroke))
    .corner_radius(8.0)
    .min_size(Vec2::new(66.0, 24.0))
}

fn draw_status_cell(
    ui: &mut egui::Ui,
    status: &QueueStatus,
    language: Language,
    palette: ThemePalette,
) {
    match status {
        QueueStatus::Progress(value) => {
            ui.vertical(|ui| {
                ui.set_width(280.0);
                ui.horizontal(|ui| {
                    ui.add(
                        ProgressBar::new(anim_value(
                            ui.ctx(),
                            ui.id().with("queue-progress"),
                            *value,
                            ANIM_FAST,
                        ))
                        .desired_width(220.0)
                        .fill(palette.primary)
                        .show_percentage(),
                    );
                    ui.label(
                        RichText::new(format!("{:.0}%", value * 100.0))
                            .size(11.0)
                            .color(palette.primary),
                    );
                });
            });
        }
        QueueStatus::Waiting => {
            ui.add_sized(
                [280.0, 22.0],
                egui::Label::new(
                    RichText::new(language.text("Waiting", "等待中"))
                        .size(12.0)
                        .color(palette.text_secondary),
                ),
            );
        }
        QueueStatus::Complete => {
            ui.add_sized(
                [280.0, 22.0],
                egui::Label::new(
                    RichText::new(language.text("Complete", "已完成"))
                        .size(12.0)
                        .color(palette.tertiary),
                ),
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueActionClick {
    None,
    Primary,
    Secondary,
}

fn draw_action_cell(
    ui: &mut egui::Ui,
    action: &QueueAction,
    language: Language,
    palette: ThemePalette,
) -> QueueActionClick {
    match action {
        QueueAction::Back | QueueAction::View => {
            let label = match action {
                QueueAction::Back => language.text("Back to Parent Folder", "返回上一层目录"),
                QueueAction::View => language.text("View", "查看"),
                QueueAction::ViewAndRemove => unreachable!(),
            };

            let response = ui.add_sized(
                [CONTENT_ACTION_COLUMN_WIDTH, 22.0],
                Button::new(RichText::new(label).size(11.0).color(palette.text_muted))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE),
            );
            if response.clicked() {
                QueueActionClick::Primary
            } else {
                QueueActionClick::None
            }
        }
        QueueAction::ViewAndRemove => {
            let mut clicked = QueueActionClick::None;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                let view_response = ui.add_sized(
                    [56.0, 22.0],
                    Button::new(
                        RichText::new(language.text("View", "查看"))
                            .size(11.0)
                            .color(palette.text_muted),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE),
                );
                if view_response.clicked() {
                    clicked = QueueActionClick::Primary;
                }

                let remove_response = ui.add_sized(
                    [64.0, 22.0],
                    Button::new(
                        RichText::new(language.text("Remove", "移除"))
                            .size(11.0)
                            .strong()
                            .color(palette.error),
                    )
                    .fill(palette.danger_fill.gamma_multiply(if palette.is_dark {
                        0.85
                    } else {
                        1.0
                    }))
                    .stroke(Stroke::new(1.0, palette.danger_stroke.gamma_multiply(0.85)))
                    .corner_radius(8.0),
                );
                if remove_response.clicked() {
                    clicked = QueueActionClick::Secondary;
                }
            });
            clicked
        }
    }
}

fn footer_link(ui: &mut egui::Ui, label: &str, active: bool, palette: ThemePalette) -> bool {
    let response = ui.add(
        Button::new(RichText::new(label).size(10.0).strong().color(if active {
            palette.primary
        } else {
            palette.text_muted
        }))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .min_size(Vec2::ZERO),
    );
    ui.add_space(12.0);
    response.clicked()
}

fn approx_size_eq(left: Vec2, right: Vec2) -> bool {
    (left.x - right.x).abs() <= RESIZE_EPSILON && (left.y - right.y).abs() <= RESIZE_EPSILON
}

fn fitted_window_size(current_size: Vec2) -> Vec2 {
    let min_scale = MIN_WINDOW_WIDTH / WINDOW_WIDTH;
    let scale = (current_size.x / WINDOW_WIDTH)
        .min(current_size.y / WINDOW_HEIGHT)
        .clamp(min_scale, 1.0);
    let width = (WINDOW_WIDTH * scale).round();

    Vec2::new(width, (width / WINDOW_ASPECT_RATIO).round())
}

fn theme_switch(ui: &mut egui::Ui, id_salt: &str, enabled: bool, palette: ThemePalette) -> bool {
    let desired_size = Vec2::new(54.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    let t = anim_bool(
        ui.ctx(),
        egui::Id::new(("theme-switch", id_salt)),
        enabled,
        ANIM_FAST,
    );

    let track_off = if palette.is_dark {
        palette.surface_variant
    } else {
        palette.surface_high
    };
    let track_fill = lerp_color(track_off, palette.primary, t);

    let knob_color = lerp_color(palette.surface_highest, palette.on_primary, t);

    let knob_range = rect.width() - 28.0;
    let knob_x = rect.left() + 14.0 + knob_range * t;

    ui.painter().rect_filled(rect, 14.0, track_fill);
    ui.painter()
        .circle_filled(egui::pos2(knob_x, rect.center().y), 10.0, knob_color);

    response.clicked()
}

fn thin_separator(ui: &mut egui::Ui, color: Color32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, color),
    );
}

fn file_kind_from_path(path: &Path, is_dir: bool, language: Language) -> String {
    detect_file_kind(path, is_dir).label(language).to_string()
}

fn detect_file_kind(path: &Path, is_dir: bool) -> DetectedFileKind {
    if is_dir {
        return DetectedFileKind::Directory;
    }

    if path.is_file() {
        if let Some(kind) = detect_existing_file_kind(path) {
            return kind;
        }
    }

    detect_name_based_file_kind(path, is_dir)
}

fn detect_existing_file_kind(path: &Path) -> Option<DetectedFileKind> {
    let mut file = File::open(path).ok()?;
    let mut header = [0u8; FILE_KIND_SNIFF_BYTES];
    let bytes_read = file.read(&mut header).ok()?;
    let bytes = &header[..bytes_read];

    if bytes.is_empty() {
        return Some(DetectedFileKind::Other);
    }

    let base_kind = if looks_like_png_signature(bytes) {
        DetectedFileKind::Png
    } else if looks_like_jpeg_signature(bytes) {
        DetectedFileKind::Jpeg
    } else if looks_like_gif_signature(bytes) {
        DetectedFileKind::Gif
    } else if looks_like_ico_signature(bytes) {
        DetectedFileKind::Ico
    } else if looks_like_bmp_signature(bytes) {
        DetectedFileKind::Bmp
    } else if looks_like_tiff_signature(bytes) {
        DetectedFileKind::Tiff
    } else if looks_like_riff_kind(bytes, b"WEBP") {
        DetectedFileKind::Webp
    } else if looks_like_pdf_signature(bytes) {
        DetectedFileKind::Pdf
    } else if looks_like_rtf_signature(bytes) {
        DetectedFileKind::Rtf
    } else if looks_like_7z_signature(bytes) {
        DetectedFileKind::SevenZip
    } else if looks_like_rar_signature(bytes) {
        DetectedFileKind::Rar
    } else if looks_like_zip_signature(bytes) {
        detect_zip_container_kind(path).unwrap_or(DetectedFileKind::Zip)
    } else if looks_like_tar_signature(bytes) {
        DetectedFileKind::Tar
    } else if looks_like_gzip_signature(bytes) {
        DetectedFileKind::Gzip
    } else if looks_like_bzip2_signature(bytes) {
        DetectedFileKind::Bzip2
    } else if looks_like_xz_signature(bytes) {
        DetectedFileKind::Xz
    } else if looks_like_flac_signature(bytes) {
        DetectedFileKind::Flac
    } else if looks_like_ogg_signature(bytes) {
        DetectedFileKind::Ogg
    } else if looks_like_riff_kind(bytes, b"WAVE") {
        DetectedFileKind::Wav
    } else if looks_like_riff_kind(bytes, b"AVI ") {
        DetectedFileKind::Avi
    } else if let Some(kind) = detect_iso_base_media_kind(bytes) {
        kind
    } else if looks_like_webm_signature(bytes) {
        DetectedFileKind::Webm
    } else if looks_like_mp3_signature(bytes) {
        DetectedFileKind::Mp3
    } else if looks_like_exe_signature(bytes) {
        DetectedFileKind::Exe
    } else if looks_like_text(bytes) {
        detect_text_file_kind(bytes)
    } else {
        DetectedFileKind::Other
    };

    Some(refine_file_kind_with_name(path, base_kind))
}

fn refine_file_kind_with_name(path: &Path, kind: DetectedFileKind) -> DetectedFileKind {
    let lower_name = lower_file_name(path);
    match kind {
        DetectedFileKind::Gzip
            if lower_name.ends_with(".tar.gz") || lower_name.ends_with(".tgz") =>
        {
            DetectedFileKind::TarGz
        }
        DetectedFileKind::Bzip2
            if lower_name.ends_with(".tar.bz2") || lower_name.ends_with(".tbz2") =>
        {
            DetectedFileKind::TarBz2
        }
        DetectedFileKind::Xz if lower_name.ends_with(".tar.xz") || lower_name.ends_with(".txz") => {
            DetectedFileKind::TarXz
        }
        _ => kind,
    }
}

fn detect_zip_container_kind(path: &Path) -> Option<DetectedFileKind> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;

    if archive.index_for_name("[Content_Types].xml").is_some() {
        if archive.index_for_name("word/document.xml").is_some() {
            return Some(DetectedFileKind::Docx);
        }
        if archive.index_for_name("xl/workbook.xml").is_some() {
            return Some(DetectedFileKind::Xlsx);
        }
        if archive.index_for_name("ppt/presentation.xml").is_some() {
            return Some(DetectedFileKind::Pptx);
        }
    }

    if let Some(mimetype) = read_zip_mimetype(&mut archive) {
        let normalized = mimetype.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "application/epub+zip" => return Some(DetectedFileKind::Epub),
            "application/vnd.oasis.opendocument.text" => return Some(DetectedFileKind::Odt),
            "application/vnd.oasis.opendocument.spreadsheet" => {
                return Some(DetectedFileKind::Ods);
            }
            "application/vnd.oasis.opendocument.presentation" => {
                return Some(DetectedFileKind::Odp);
            }
            _ => {}
        }
    }

    if archive.index_for_name("AndroidManifest.xml").is_some()
        && archive.index_for_name("classes.dex").is_some()
    {
        return Some(DetectedFileKind::Apk);
    }

    if archive.index_for_name("META-INF/MANIFEST.MF").is_some() {
        return Some(DetectedFileKind::Jar);
    }

    Some(DetectedFileKind::Zip)
}

fn read_zip_mimetype(archive: &mut ZipArchive<File>) -> Option<String> {
    let mut entry = archive.by_name("mimetype").ok()?;
    let bytes_to_read = entry.size().min(ZIP_MIMETYPE_READ_LIMIT as u64) as usize;
    let mut buffer = vec![0u8; bytes_to_read];
    let read = entry.read(&mut buffer).ok()?;
    buffer.truncate(read);
    Some(
        String::from_utf8_lossy(&buffer)
            .trim_matches(char::from(0))
            .trim()
            .to_string(),
    )
}

fn detect_name_based_file_kind(path: &Path, is_dir: bool) -> DetectedFileKind {
    if is_dir {
        return DetectedFileKind::Directory;
    }

    let lower_name = lower_file_name(path);
    match () {
        _ if lower_name.ends_with(".tar.gz") || lower_name.ends_with(".tgz") => {
            DetectedFileKind::TarGz
        }
        _ if lower_name.ends_with(".tar.bz2") || lower_name.ends_with(".tbz2") => {
            DetectedFileKind::TarBz2
        }
        _ if lower_name.ends_with(".tar.xz") || lower_name.ends_with(".txz") => {
            DetectedFileKind::TarXz
        }
        _ if lower_name.ends_with(".7z") => DetectedFileKind::SevenZip,
        _ if lower_name.ends_with(".zip") => DetectedFileKind::Zip,
        _ if lower_name.ends_with(".rar") => DetectedFileKind::Rar,
        _ if lower_name.ends_with(".tar") => DetectedFileKind::Tar,
        _ if lower_name.ends_with(".gz") => DetectedFileKind::Gzip,
        _ if lower_name.ends_with(".bz2") || lower_name.ends_with(".bzip2") => {
            DetectedFileKind::Bzip2
        }
        _ if lower_name.ends_with(".xz") => DetectedFileKind::Xz,
        _ if lower_name.ends_with(".png") => DetectedFileKind::Png,
        _ if lower_name.ends_with(".jpg") || lower_name.ends_with(".jpeg") => {
            DetectedFileKind::Jpeg
        }
        _ if lower_name.ends_with(".gif") => DetectedFileKind::Gif,
        _ if lower_name.ends_with(".webp") => DetectedFileKind::Webp,
        _ if lower_name.ends_with(".bmp") => DetectedFileKind::Bmp,
        _ if lower_name.ends_with(".tif") || lower_name.ends_with(".tiff") => {
            DetectedFileKind::Tiff
        }
        _ if lower_name.ends_with(".ico") => DetectedFileKind::Ico,
        _ if lower_name.ends_with(".avif") => DetectedFileKind::Avif,
        _ if lower_name.ends_with(".heic") || lower_name.ends_with(".heif") => {
            DetectedFileKind::Heic
        }
        _ if lower_name.ends_with(".pdf") => DetectedFileKind::Pdf,
        _ if lower_name.ends_with(".docx") => DetectedFileKind::Docx,
        _ if lower_name.ends_with(".xlsx") => DetectedFileKind::Xlsx,
        _ if lower_name.ends_with(".pptx") => DetectedFileKind::Pptx,
        _ if lower_name.ends_with(".epub") => DetectedFileKind::Epub,
        _ if lower_name.ends_with(".odt") => DetectedFileKind::Odt,
        _ if lower_name.ends_with(".ods") => DetectedFileKind::Ods,
        _ if lower_name.ends_with(".odp") => DetectedFileKind::Odp,
        _ if lower_name.ends_with(".apk") => DetectedFileKind::Apk,
        _ if lower_name.ends_with(".jar") => DetectedFileKind::Jar,
        _ if lower_name.ends_with(".exe") => DetectedFileKind::Exe,
        _ if lower_name.ends_with(".mp3") => DetectedFileKind::Mp3,
        _ if lower_name.ends_with(".wav") => DetectedFileKind::Wav,
        _ if lower_name.ends_with(".flac") => DetectedFileKind::Flac,
        _ if lower_name.ends_with(".ogg") => DetectedFileKind::Ogg,
        _ if lower_name.ends_with(".mp4") || lower_name.ends_with(".m4v") => DetectedFileKind::Mp4,
        _ if lower_name.ends_with(".mov") => DetectedFileKind::Mov,
        _ if lower_name.ends_with(".webm") => DetectedFileKind::Webm,
        _ if lower_name.ends_with(".avi") => DetectedFileKind::Avi,
        _ if lower_name.ends_with(".svg") => DetectedFileKind::Svg,
        _ if lower_name.ends_with(".json") => DetectedFileKind::Json,
        _ if lower_name.ends_with(".xml") => DetectedFileKind::Xml,
        _ if lower_name.ends_with(".rtf") => DetectedFileKind::Rtf,
        _ if lower_name.ends_with(".txt")
            || lower_name.ends_with(".md")
            || lower_name.ends_with(".toml")
            || lower_name.ends_with(".yaml")
            || lower_name.ends_with(".yml")
            || lower_name.ends_with(".ini")
            || lower_name.ends_with(".log") =>
        {
            DetectedFileKind::Txt
        }
        _ => DetectedFileKind::Other,
    }
}

fn lower_file_name(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
        .to_ascii_lowercase()
}

fn looks_like_png_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
}

fn looks_like_jpeg_signature(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
}

fn looks_like_gif_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")
}

fn looks_like_ico_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x00, 0x00, 0x01, 0x00])
}

fn looks_like_bmp_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"BM")
}

fn looks_like_tiff_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || bytes.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
}

fn looks_like_pdf_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

fn looks_like_rtf_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(br"{\rtf")
}

fn looks_like_zip_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04])
        || bytes.starts_with(&[0x50, 0x4B, 0x05, 0x06])
        || bytes.starts_with(&[0x50, 0x4B, 0x07, 0x08])
}

fn looks_like_7z_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C])
}

fn looks_like_rar_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00])
        || bytes.starts_with(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00])
}

fn looks_like_tar_signature(bytes: &[u8]) -> bool {
    bytes.len() > 262 && bytes[257..262] == *b"ustar"
}

fn looks_like_gzip_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1F, 0x8B])
}

fn looks_like_bzip2_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"BZh")
}

fn looks_like_xz_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFD, b'7', b'z', b'X', b'Z', 0x00])
}

fn looks_like_riff_kind(bytes: &[u8], kind: &[u8; 4]) -> bool {
    bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == kind
}

fn looks_like_flac_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"fLaC")
}

fn looks_like_ogg_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"OggS")
}

fn looks_like_webm_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) && bytes.windows(4).any(|window| window == b"webm")
}

fn looks_like_mp3_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"ID3")
        || (bytes.len() >= 4
            && bytes[0] == 0xFF
            && (bytes[1] & 0xE0) == 0xE0
            && (bytes[1] & 0x06) != 0
            && (bytes[2] & 0xF0) != 0xF0
            && (bytes[2] & 0x0C) != 0x0C)
}

fn looks_like_exe_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"MZ")
}

fn detect_iso_base_media_kind(bytes: &[u8]) -> Option<DetectedFileKind> {
    if bytes.len() < 16 || &bytes[4..8] != b"ftyp" {
        return None;
    }

    let has_brand = |brand: &[u8; 4]| bytes[8..].chunks_exact(4).any(|chunk| chunk == brand);

    if has_brand(b"avif") || has_brand(b"avis") {
        return Some(DetectedFileKind::Avif);
    }

    if [
        *b"heic", *b"heix", *b"hevc", *b"hevx", *b"heif", *b"heim", *b"heis", *b"qt  ",
    ]
    .contains(&<[u8; 4]>::try_from(&bytes[8..12]).ok()?)
    {
        if &bytes[8..12] == b"qt  " {
            return Some(DetectedFileKind::Mov);
        }
        return Some(DetectedFileKind::Heic);
    }

    if has_brand(b"heic")
        || has_brand(b"heix")
        || has_brand(b"hevc")
        || has_brand(b"hevx")
        || has_brand(b"heif")
        || has_brand(b"heim")
        || has_brand(b"heis")
    {
        return Some(DetectedFileKind::Heic);
    }

    if has_brand(b"qt  ") {
        return Some(DetectedFileKind::Mov);
    }

    if has_brand(b"isom")
        || has_brand(b"iso2")
        || has_brand(b"mp41")
        || has_brand(b"mp42")
        || has_brand(b"mp71")
        || has_brand(b"MSNV")
        || has_brand(b"avc1")
        || has_brand(b"dash")
        || has_brand(b"M4V ")
        || has_brand(b"M4A ")
    {
        return Some(DetectedFileKind::Mp4);
    }

    None
}

fn looks_like_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return true;
    }
    if bytes.contains(&0) {
        return false;
    }

    let sample = &bytes[..bytes.len().min(2048)];
    let control_count = sample
        .iter()
        .filter(|&&byte| matches!(byte, 0x01..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F))
        .count();
    control_count * 10 <= sample.len()
}

fn detect_text_file_kind(bytes: &[u8]) -> DetectedFileKind {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return DetectedFileKind::Txt;
    }

    let snippet = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&bytes[3..]).to_string()
    } else {
        String::from_utf8_lossy(bytes).to_string()
    };
    let trimmed = snippet.trim_start();
    let lower = trimmed.to_ascii_lowercase();

    if lower.starts_with("{\\rtf") {
        return DetectedFileKind::Rtf;
    }
    if lower.starts_with('{') || lower.starts_with('[') {
        return DetectedFileKind::Json;
    }
    if lower.starts_with("<svg") || lower.contains("<svg") {
        return DetectedFileKind::Svg;
    }
    if lower.starts_with("<?xml")
        || (lower.starts_with('<')
            && !lower.starts_with("<!doctype html")
            && !lower.starts_with("<html"))
    {
        return DetectedFileKind::Xml;
    }

    DetectedFileKind::Txt
}

fn format_size(value: Option<u64>) -> String {
    match value {
        Some(bytes) => {
            let units = ["B", "KB", "MB", "GB", "TB"];
            let mut size = bytes as f64;
            let mut index = 0usize;
            while size >= 1024.0 && index < units.len() - 1 {
                size /= 1024.0;
                index += 1;
            }
            if index == 0 {
                format!("{bytes} {}", units[index])
            } else {
                format!("{size:.1} {}", units[index])
            }
        }
        None => "-".to_string(),
    }
}

fn format_transfer_rate(bytes_per_second: f64) -> String {
    if !bytes_per_second.is_finite() || bytes_per_second <= 0.0 {
        return "--".to_string();
    }

    format!("{}/s", format_size(Some(bytes_per_second.round() as u64)))
}

fn format_duration_compact(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

fn apply_template(template: &str, replacements: &[(&str, String)]) -> String {
    let mut rendered = template.to_string();
    for (placeholder, value) in replacements {
        rendered = rendered.replace(placeholder, value);
    }
    rendered
}

fn localize_format(
    english: &'static str,
    chinese: &'static str,
    replacements: &[(&str, String)],
) -> String {
    apply_template(localize_message(english, chinese), replacements)
}

fn suggest_available_conflict_path(
    destination: &Path,
    plan: &ExtractPathPlan,
    reserved_paths: &BTreeSet<PathBuf>,
) -> PathBuf {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "file".to_string());
    let stem = destination
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| file_name.clone());
    let extension = destination
        .extension()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty());

    for index in 1.. {
        let candidate_name = if let Some(extension) = &extension {
            format!("{stem} ({index}).{extension}")
        } else {
            format!("{stem} ({index})")
        };
        let candidate = parent.join(candidate_name);
        if !candidate.exists()
            && !reserved_paths.contains(&candidate)
            && !plan.renamed_paths.values().any(|path| path == &candidate)
        {
            return candidate;
        }
    }

    destination.to_path_buf()
}

fn real_task_progress(task: &TaskQueueItem) -> f32 {
    match task.state {
        TaskState::Completed => 1.0,
        TaskState::Queued | TaskState::Failed | TaskState::Canceled if task.total_bytes == 0 => 0.0,
        TaskState::Queued
        | TaskState::Running
        | TaskState::Canceling
        | TaskState::Failed
        | TaskState::Canceled => {
            if task.total_bytes == 0 {
                0.0
            } else {
                let progress =
                    (task.processed_bytes as f32 / task.total_bytes as f32).clamp(0.0, 1.0);
                if matches!(task.state, TaskState::Running | TaskState::Canceling) {
                    progress.min(0.99)
                } else {
                    progress
                }
            }
        }
    }
}

fn task_is_finalizing(task: &TaskQueueItem) -> bool {
    task.total_bytes > 0
        && task.processed_bytes >= task.total_bytes
        && matches!(task.state, TaskState::Running | TaskState::Canceling)
}

fn running_task_eta(task: &TaskQueueItem, _now: Instant) -> Option<Duration> {
    let speed = task.current_bytes_per_second?;
    if speed <= 0.0 || task.total_bytes == 0 {
        return None;
    }

    let remaining = task.total_bytes.saturating_sub(task.processed_bytes);
    Some(Duration::from_secs_f64((remaining as f64 / speed).max(0.0)))
}

fn completed_task_rate(task: &TaskQueueItem) -> Option<f64> {
    let started_at = task.started_at?;
    let finished_at = task.finished_at?;
    let elapsed = finished_at
        .saturating_duration_since(started_at)
        .as_secs_f64();
    if elapsed <= 0.0 || task.total_bytes == 0 {
        return None;
    }

    Some(task.total_bytes as f64 / elapsed)
}

fn completed_shell_task_rate(
    total_bytes: u64,
    started_at: Instant,
    finished_at: Instant,
) -> Option<f64> {
    let elapsed = finished_at
        .saturating_duration_since(started_at)
        .as_secs_f64();
    if elapsed <= 0.0 || total_bytes == 0 {
        return None;
    }

    Some(total_bytes as f64 / elapsed)
}

fn total_bytes_for_sources(sources: &[PathBuf], excluded_paths: &[PathBuf]) -> Result<u64> {
    let mut total = 0u64;
    for source in sources {
        total += total_bytes_for_path(source, excluded_paths)?;
    }
    Ok(total)
}

fn total_bytes_for_path(path: &Path, excluded_paths: &[PathBuf]) -> Result<u64> {
    if excluded_paths
        .iter()
        .any(|excluded| path == excluded || path.starts_with(excluded))
    {
        return Ok(0);
    }

    let metadata = fs::metadata(path).with_context(|| {
        localize_format(
            "Failed to read metadata for {path}",
            "读取 {path} 的元数据失败",
            &[("{path}", path.display().to_string())],
        )
    })?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }

    let mut total = 0u64;
    for entry in fs::read_dir(path).with_context(|| {
        localize_format(
            "Failed to enumerate {path}",
            "枚举 {path} 失败",
            &[("{path}", path.display().to_string())],
        )
    })? {
        let entry = entry.with_context(|| {
            localize_format(
                "Failed to read an entry in {path}",
                "读取 {path} 中的项目失败",
                &[("{path}", path.display().to_string())],
            )
        })?;
        total += total_bytes_for_path(&entry.path(), excluded_paths)?;
    }
    Ok(total)
}

fn ensure_archive_extension(path: PathBuf, format: CompressionFormat) -> PathBuf {
    let Some(file_name) = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
    else {
        return path;
    };
    if format.matches_file_name(&file_name) {
        return path;
    }

    let lower = file_name.to_ascii_lowercase();
    let base_name = known_archive_suffixes_for_output()
        .iter()
        .find_map(|suffix| {
            lower
                .ends_with(suffix)
                .then_some(file_name[..file_name.len() - suffix.len()].to_string())
        })
        .unwrap_or(file_name);

    path.with_file_name(format!("{base_name}{}", format.primary_suffix()))
}

fn suggested_archive_output_path(
    sources: &[PathBuf],
    format: CompressionFormat,
) -> Option<PathBuf> {
    let [first, ..] = sources else {
        return None;
    };

    if sources.len() == 1 {
        let parent = first.parent().unwrap_or_else(|| Path::new("."));
        let base_name = if first.is_file() {
            if format.is_single_file_stream() {
                first
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "archive".to_string())
            } else {
                first
                    .file_stem()
                    .map(|value| value.to_string_lossy().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "archive".to_string())
            }
        } else {
            first
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "archive".to_string())
        };
        return Some(parent.join(format!("{base_name}{}", format.primary_suffix())));
    }

    let output_dir = common_source_parent(sources).unwrap_or_else(|| {
        first
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    });
    Some(output_dir.join(format!("fastzip_bundle{}", format.primary_suffix())))
}

fn known_archive_suffixes_for_output() -> &'static [&'static str] {
    &[
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tgz", ".tbz2", ".txz", ".7z", ".zip", ".tar", ".bzip2",
        ".gz", ".bz2", ".xz", ".zst", ".zstd", ".tar.zst", ".tzst", ".lz4", ".tar.lz4", ".tlz4",
    ]
}

fn common_source_parent(sources: &[PathBuf]) -> Option<PathBuf> {
    let mut parents = sources
        .iter()
        .filter_map(|source| source.parent().map(Path::to_path_buf));
    let mut shared = parents.next()?;

    for parent in parents {
        while !parent.starts_with(&shared) {
            if !shared.pop() {
                return None;
            }
        }
    }

    Some(shared)
}

fn open_path_with_system_default(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let wide_path = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                wide_path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if (result as usize) <= 32 {
            bail!(
                "{}",
                localize_format(
                    "Failed to open {path}",
                    "打开 {path} 失败",
                    &[("{path}", path.display().to_string())],
                )
            );
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open").arg(path).status().with_context(|| {
            localize_format(
                "Failed to open {path}",
                "打开 {path} 失败",
                &[("{path}", path.display().to_string())],
            )
        })?;
        if !status.success() {
            bail!(
                "{}",
                localize_format(
                    "System open command failed for {path}",
                    "{path} 的系统打开命令执行失败",
                    &[("{path}", path.display().to_string())],
                )
            );
        }
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = Command::new("xdg-open")
            .arg(path)
            .status()
            .with_context(|| {
                localize_format(
                    "Failed to open {path}",
                    "打开 {path} 失败",
                    &[("{path}", path.display().to_string())],
                )
            })?;
        if !status.success() {
            bail!(
                "{}",
                localize_format(
                    "System open command failed for {path}",
                    "{path} 的系统打开命令执行失败",
                    &[("{path}", path.display().to_string())],
                )
            );
        }
        return Ok(());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        bail!(
            "{}",
            localize_message(
                "System preview is not supported on this platform",
                "当前平台不支持系统预览",
            )
        )
    }
}

fn default_file_manager_dir() -> PathBuf {
    for candidate in [
        env::var_os("USERPROFILE").map(PathBuf::from),
        env::var_os("HOME").map(PathBuf::from),
        env::current_dir().ok(),
    ]
    .into_iter()
    .flatten()
    {
        if candidate.is_dir() {
            return candidate;
        }
    }

    PathBuf::from(".")
}

fn resolve_file_manager_directory(path: &Path) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let normalized = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());
    let normalized = normalize_windows_user_path(&normalized);

    if !normalized.exists() {
        bail!(
            "{}",
            localize_format(
                "Folder path not found: {path}",
                "未找到文件夹路径：{path}",
                &[("{path}", normalized.display().to_string())],
            )
        );
    }
    if !normalized.is_dir() {
        bail!(
            "{}",
            localize_format(
                "The selected path is not a folder: {path}",
                "所选路径不是文件夹：{path}",
                &[("{path}", normalized.display().to_string())],
            )
        );
    }

    Ok(normalized)
}

fn normalize_file_manager_compression_paths(selected_paths: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
    let mut ordered = selected_paths
        .iter()
        .filter(|path| path.exists())
        .map(|path| normalize_windows_user_path(path))
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });

    let mut normalized = Vec::new();
    for path in ordered {
        if normalized.iter().any(|existing| path.starts_with(existing)) {
            continue;
        }
        normalized.push(path);
    }
    normalized.sort();
    normalized
}

#[cfg(target_os = "windows")]
fn normalize_windows_user_path(path: &Path) -> PathBuf {
    match path.components().next() {
        Some(Component::Prefix(prefix_component)) => match prefix_component.kind() {
            Prefix::VerbatimDisk(letter) => {
                let mut normalized = PathBuf::from(format!("{}:\\", letter as char));
                for component in path.components().skip(1) {
                    normalized.push(component.as_os_str());
                }
                normalized
            }
            Prefix::VerbatimUNC(server, share) => {
                let mut normalized = PathBuf::from(format!(
                    r"\\{}\{}",
                    server.to_string_lossy(),
                    share.to_string_lossy()
                ));
                for component in path.components().skip(1) {
                    normalized.push(component.as_os_str());
                }
                normalized
            }
            _ => path.to_path_buf(),
        },
        _ => path.to_path_buf(),
    }
}

#[cfg(not(target_os = "windows"))]
fn normalize_windows_user_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn paint_background(ui: &mut egui::Ui, palette: ThemePalette) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, palette.background);
    let time = ui.ctx().input(|input| input.time) as f32;
    let drift_x1 = (time * 0.15).sin() * 12.0;
    let drift_y1 = (time * 0.18).cos() * 10.0;
    let drift_r1 = (time * 0.12).sin() * 8.0;
    let drift_x2 = (time * 0.17).cos() * 14.0;
    let drift_y2 = (time * 0.13).sin() * 10.0;
    let drift_r2 = (time * 0.14).cos() * 6.0;
    painter.circle_filled(
        rect.left_top() + Vec2::new(160.0 + drift_x1, 80.0 + drift_y1),
        170.0 + drift_r1,
        palette.background_glow_primary,
    );
    painter.circle_filled(
        rect.right_top() - Vec2::new(140.0 - drift_x2, -30.0 + drift_y2),
        180.0 + drift_r2,
        palette.background_glow_secondary,
    );
}

fn initial_ready_log(language: Language) -> String {
    language
        .text("Native formats loaded successfully.", "原生格式已加载。")
        .to_string()
}

fn initial_backend_log(language: Language) -> String {
    language
        .text(
            "Engine initialized. Multi-threading enabled.",
            "引擎已初始化，多线程已启用。",
        )
        .to_string()
}

fn load_multilingual_fonts() -> Vec<(String, Vec<u8>)> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(custom_path) = env::var("FASTZIP_UI_FONT") {
        candidates.push(PathBuf::from(custom_path));
    }

    candidates.extend([
        PathBuf::from(r"C:\Windows\Fonts\NotoSansSC-VF.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\NotoSerifSC-VF.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\msyh.ttc"),
        PathBuf::from(r"C:\Windows\Fonts\Deng.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\YuGothM.ttc"),
        PathBuf::from(r"C:\Windows\Fonts\meiryo.ttc"),
        PathBuf::from(r"C:\Windows\Fonts\malgun.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\segoeui.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\tahoma.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\msyhbd.ttc"),
        PathBuf::from(r"C:\Windows\Fonts\msyhl.ttc"),
        PathBuf::from(r"C:\Windows\Fonts\simhei.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\simsun.ttc"),
        PathBuf::from(r"C:\Windows\Fonts\simsunb.ttf"),
    ]);

    let mut loaded_fonts = Vec::new();

    for path in candidates {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };

        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "fastzip-ui-font".to_string());
        if loaded_fonts
            .iter()
            .any(|(existing_name, _)| existing_name == &name)
        {
            continue;
        }
        loaded_fonts.push((name, bytes));
    }

    loaded_fonts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    #[test]
    fn detects_png_from_signature_without_relying_on_extension() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("image.data");
        fs::write(
            &path,
            [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00],
        )
        .unwrap();

        assert_eq!(detect_file_kind(&path, false), DetectedFileKind::Png);
    }

    #[test]
    fn keeps_unknown_binary_files_as_other_even_with_popular_extensions() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("fake.png");
        fs::write(&path, [0x00, 0x01, 0x02, 0x03, 0x04]).unwrap();

        assert_eq!(detect_file_kind(&path, false), DetectedFileKind::Other);
    }

    #[test]
    fn distinguishes_docx_packages_from_plain_zip_archives() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("package.bin");
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>"#)
            .unwrap();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(br#"<w:document xmlns:w="urn:test"></w:document>"#)
            .unwrap();
        zip.finish().unwrap();

        let kind = detect_file_kind(&path, false);
        assert_eq!(kind, DetectedFileKind::Docx);
        assert!(!kind.show_extract_action());
    }

    #[test]
    fn detects_raw_zip_archives_without_zip_extensions() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("archive.data");
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.start_file("hello.txt", options).unwrap();
        zip.write_all(b"hello world").unwrap();
        zip.finish().unwrap();

        let kind = detect_file_kind(&path, false);
        assert_eq!(kind, DetectedFileKind::Zip);
        assert!(kind.show_extract_action());
    }

    #[test]
    fn falls_back_to_known_name_mapping_only_when_content_is_unavailable() {
        assert_eq!(
            detect_file_kind(Path::new("report.pdf"), false),
            DetectedFileKind::Pdf
        );
        assert_eq!(
            detect_file_kind(Path::new("unknown.custom"), false),
            DetectedFileKind::Other
        );
    }
}
