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
    self, Align, Button, CentralPanel, Color32, Context, Frame, Layout, Margin, ProgressBar,
    RichText, ScrollArea, SidePanel, Stroke, TextEdit, TopBottomPanel, Vec2,
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
use crate::hash::{ChecksumResult, file_checksums};
use crate::localization::{
    AppLocale, detect_app_locale, locale_is_chinese, localize_message, set_current_locale,
    supported_locales,
};
use crate::settings::{
    load_auto_update_enabled, load_autostart_enabled, load_preferred_theme,
    save_auto_update_enabled, save_autostart_enabled, save_preferred_language_value,
    save_preferred_theme_value,
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
    let value =
        ctx.animate_bool_with_time_and_easing(id, target, duration, egui::emath::easing::cubic_out);
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
        .or_else(|| suggested_archive_output_path(&normalized_sources, format, Language::detect()))
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
    auto_update_enabled: bool,
    show_update_dialog: bool,
    update_info: Option<crate::update::ReleaseInfo>,
    update_check_pending: bool,
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
    update_receiver: Option<Receiver<crate::update::ReleaseInfo>>,
    requested_viewport_size: Option<Vec2>,
    pending_launch_request: Option<GuiLaunchRequest>,
    #[cfg(target_os = "windows")]
    root_hwnd: Option<HWND>,
}

mod app;

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
                        format_size(Some(self.processed_bytes), self.language),
                        format_size(Some(self.total_bytes), self.language)
                    )
                }
            }
            ShellCompressionState::Completed => self
                .report
                .as_ref()
                .map(|report| {
                    format!(
                        "{} {}, {} {}  ·  {} → {}",
                        report.files_added,
                        self.t("files", "个文件"),
                        report.directories_added,
                        self.t("dirs", "个目录"),
                        format_size(Some(report.input_bytes), self.language),
                        format_size(Some(report.output_bytes), self.language),
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
                                            .map(|v| format_transfer_rate(v, self.language))
                                            .unwrap_or_else(|| { self.t("--", "--").to_string() })
                                    ))
                                    .size(12.0)
                                    .color(palette.text_secondary),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{}: {}",
                                        self.t("Elapsed", "耗时"),
                                        format_duration_compact(elapsed, self.language)
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
        self.trigger_startup_update_check();
        self.poll_update_check();
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
        self.draw_update_dialog(ctx);
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
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/fastzip-icon.png"))
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
    let active = anim_bool(
        ui.ctx(),
        egui::Id::new("status-chip-pulse"),
        busy,
        ANIM_NORMAL,
    );
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

fn format_size(value: Option<u64>, language: Language) -> String {
    match value {
        Some(bytes) => {
            let units = [
                language.text("B", "B"),
                language.text("KB", "KB"),
                language.text("MB", "MB"),
                language.text("GB", "GB"),
                language.text("TB", "TB"),
            ];
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
        None => language.text("-", "-").to_string(),
    }
}

fn format_transfer_rate(bytes_per_second: f64, language: Language) -> String {
    if !bytes_per_second.is_finite() || bytes_per_second <= 0.0 {
        return language.text("--", "--").to_string();
    }

    format!(
        "{}{}",
        format_size(Some(bytes_per_second.round() as u64), language),
        language.text("/s", "/秒"),
    )
}

fn format_duration_compact(duration: Duration, language: Language) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    let h_label = language.text("h", "时");
    let m_label = language.text("m", "分");
    let s_label = language.text("s", "秒");

    if hours > 0 {
        format!("{hours}{h_label} {minutes:02}{m_label}")
    } else if minutes > 0 {
        format!("{minutes}{m_label} {seconds:02}{s_label}")
    } else {
        format!("{seconds}{s_label}")
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
    language: Language,
) -> Option<PathBuf> {
    let [first, ..] = sources else {
        return None;
    };

    if sources.len() == 1 {
        let parent = first.parent().unwrap_or_else(|| Path::new("."));
        let default_name = language.text("archive", "归档");
        let base_name = if first.is_file() {
            if format.is_single_file_stream() {
                first
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| default_name.to_string())
            } else {
                first
                    .file_stem()
                    .map(|value| value.to_string_lossy().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| default_name.to_string())
            }
        } else {
            first
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| default_name.to_string())
        };
        return Some(parent.join(format!("{base_name}{}", format.primary_suffix())));
    }

    let output_dir = common_source_parent(sources).unwrap_or_else(|| {
        first
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    });
    Some(output_dir.join(format!(
        "{}{}",
        language.text("fastzip_bundle", "fastzip压缩包"),
        format.primary_suffix()
    )))
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
