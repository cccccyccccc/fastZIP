use std::env;

use super::*;

impl FastZipGui {
    pub(super) fn new(
        cc: &eframe::CreationContext<'_>,
        launch_request: Option<GuiLaunchRequest>,
    ) -> Self {
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
        let auto_update_enabled = load_auto_update_enabled();
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
            auto_update_enabled,
            show_update_dialog: false,
            update_info: None,
            update_check_pending: false,
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
            update_receiver: None,
            requested_viewport_size: None,
            pending_launch_request: launch_request,
            #[cfg(target_os = "windows")]
            root_hwnd,
        }
    }

    pub(super) fn is_scanning_archive(&self) -> bool {
        self.scan_receiver.is_some()
    }

    pub(super) fn has_active_task(&self) -> bool {
        self.task_receiver.is_some()
    }

    pub(super) fn t(&self, english: &'static str, chinese: &'static str) -> &'static str {
        self.language.text(english, chinese)
    }

    pub(super) fn palette(&self) -> ThemePalette {
        theme_palette(self.theme_mode)
    }

    pub(super) fn switch_locale(&mut self, locale: &'static AppLocale) {
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
            locale.display_name()
        ));
        if let Err(error) = save_preferred_language_value(locale.code) {
            self.push_log(format!(
                "{}: {error:#}",
                self.t("Failed to save language preference", "保存语言偏好失败")
            ));
        }
    }

    pub(super) fn switch_theme(&mut self, theme_mode: ThemeMode) {
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

    pub(super) fn switch_autostart(&mut self, enabled: bool) {
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
    pub(super) fn try_elevated_delete_autostart(&mut self) {
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
            let message = self.t("Autostart disabled successfully.", "已成功关闭开机自启动。");
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
    pub(super) fn try_elevated_delete_autostart(&mut self) {}

    #[cfg(target_os = "windows")]
    pub(super) fn try_open_default_apps_dialog(&mut self) {
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
    pub(super) fn try_open_default_apps_dialog(&mut self) {}
    pub(super) fn activate_workspace(&mut self, workspace_mode: WorkspaceMode) {
        self.workspace_mode = workspace_mode;
        self.side_nav = self.current_workspace_nav_item();
    }

    pub(super) fn current_workspace_nav_item(&self) -> SideNavItem {
        match self.workspace_mode {
            WorkspaceMode::Compress => SideNavItem::Compress,
            WorkspaceMode::Extract => SideNavItem::Extract,
        }
    }

    pub(super) fn set_side_nav(&mut self, item: SideNavItem) {
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

    pub(super) fn reset_extract_browser(&mut self) {
        self.extract_browser_path.clear();
        self.preview_archive_path = None;
        self.preview_output_dir = None;
    }

    pub(super) fn reset_compress_browser(&mut self) {
        self.compress_browser_path = None;
    }

    pub(super) fn set_file_manager_directory(&mut self, path: PathBuf) {
        let normalized = normalize_windows_user_path(&path);
        self.file_manager_current_dir = normalized.clone();
        self.file_manager_path_input = normalized.display().to_string();
    }

    pub(super) fn open_file_manager_directory(&mut self, path: PathBuf) -> Result<()> {
        let resolved = resolve_file_manager_directory(&path)?;
        self.set_side_nav(SideNavItem::FileManager);
        self.set_file_manager_directory(resolved);
        Ok(())
    }

    pub(super) fn open_file_manager_typed_path(&mut self) {
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

    pub(super) fn browse_file_manager_folder(&mut self) {
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

    pub(super) fn open_file_manager_parent_directory(&mut self) {
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

    pub(super) fn view_file_manager_entry(&mut self, path: &Path, is_dir: bool) {
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

    pub(super) fn extract_file_manager_zip(&mut self, path: &Path) {
        if let Err(error) = self.apply_archive_selection(path.to_path_buf()) {
            self.push_log(self.language.format(
                "Archive selection failed: {error}",
                "选择压缩包失败：{error}",
                &[("{error}", format!("{error:#}"))],
            ));
            self.show_toast(FeedbackTone::Error, format!("{error:#}"));
        }
    }

    pub(super) fn compress_selected_file_manager_paths(&mut self) {
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

    pub(super) fn schedule_dialog_action(&mut self, ctx: &Context, action: PendingDialogAction) {
        if self.dialog_click_guard {
            return;
        }

        self.pending_dialog_action = Some(action);
        self.dialog_click_guard = true;
        ctx.request_repaint();
    }

    pub(super) fn refresh_dialog_click_guard(&mut self, ctx: &Context) {
        if ctx.input(|input| input.pointer.any_down()) {
            self.dialog_click_guard = false;
        }
    }

    pub(super) fn process_launch_request(&mut self) {
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

    pub(super) fn process_pending_dialog_action(&mut self) {
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

    pub(super) fn enforce_viewport_aspect_ratio(&mut self, ctx: &Context) {
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

    pub(super) fn browse_archive(&mut self) {
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

    pub(super) fn apply_archive_selection(&mut self, path: PathBuf) -> Result<()> {
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

    pub(super) fn browse_compress_files(&mut self) {
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

    pub(super) fn browse_compress_folders(&mut self) {
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

    pub(super) fn apply_compress_sources(&mut self, sources: Vec<PathBuf>) {
        self.compress_sources = sources;
        self.compress_excluded_paths.clear();
        self.reset_compress_browser();
        if let Some(suggested_output) = suggested_archive_output_path(
            &self.compress_sources,
            self.compression_options.format,
            self.language,
        ) {
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

    pub(super) fn append_compress_sources(&mut self, mut dropped_sources: Vec<PathBuf>) {
        let previous_sources = self.compress_sources.clone();
        let previous_suggested_output = suggested_archive_output_path(
            &previous_sources,
            self.compression_options.format,
            self.language,
        )
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
                self.language,
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

    pub(super) fn retain_valid_compress_exclusions(&mut self) {
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

    pub(super) fn exclude_compress_path(&mut self, path: &Path) {
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

    pub(super) fn remove_compress_source(&mut self, path: &Path) {
        let previous_sources = self.compress_sources.clone();
        let previous_count = previous_sources.len();
        let previous_suggested_output = suggested_archive_output_path(
            &previous_sources,
            self.compression_options.format,
            self.language,
        )
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
                    self.language,
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

    pub(super) fn open_compress_source_picker(&mut self) {
        self.activate_workspace(WorkspaceMode::Compress);
        self.show_compress_source_picker = true;
    }

    pub(super) fn draw_compress_source_picker(&mut self, ctx: &Context) {
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

    pub(super) fn browse_compress_output_path(&mut self) {
        self.activate_workspace(WorkspaceMode::Compress);

        let mut dialog = FileDialog::new();
        if let Some(default_output) = suggested_archive_output_path(
            &self.compress_sources,
            self.compression_options.format,
            self.language,
        ) {
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

    pub(super) fn browse_output_dir(&mut self) {
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

    pub(super) fn handle_dropped_files(&mut self, ctx: &Context) {
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

    pub(super) fn handle_compress_drop(&mut self, dropped_paths: Vec<PathBuf>) {
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

    pub(super) fn handle_extract_drop(&mut self, dropped_paths: Vec<PathBuf>) {
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

    pub(super) fn scan_archive(&mut self) {
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

    pub(super) fn test_current_archive(&mut self) {
        let archive_path = match self.archive_path_buf() {
            Ok(path) => path,
            Err(message) => {
                self.push_log(message);
                return;
            }
        };
        let password: Option<String> =
            (!self.extract_password.is_empty()).then_some(self.extract_password.clone());
        match self
            .service
            .test_archive_with_password(&archive_path, password.as_deref())
        {
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

    pub(super) fn extract_archive(&mut self) {
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

    pub(super) fn enqueue_extract_task(&mut self, pending_task: PendingExtractTask) {
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

    pub(super) fn prepare_extract_conflict_dialog(
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

    pub(super) fn loaded_or_fetch_archive_entries(
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

    pub(super) fn collect_extract_conflicts(
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

    pub(super) fn reserved_extract_conflict_paths(
        dialog: &ExtractConflictDialogState,
    ) -> BTreeSet<PathBuf> {
        dialog.task.plan.renamed_paths.values().cloned().collect()
    }

    pub(super) fn default_extract_conflict_rename_value(
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

    pub(super) fn compress_archive(&mut self) {
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

    pub(super) fn estimate_extract_total_bytes(&self, archive_path: &Path) -> Result<u64> {
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

    pub(super) fn enqueue_task(&mut self, kind: TaskKind, spec: TaskSpec, total_bytes: u64) -> u64 {
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

    pub(super) fn start_next_task(&mut self) {
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

    pub(super) fn can_prioritize_task(&self, task_id: u64) -> bool {
        let Some(first_queued_index) = self
            .task_queue
            .iter()
            .position(|task| task.state == TaskState::Queued)
        else {
            return false;
        };

        self.task_queue[first_queued_index].id != task_id
    }

    pub(super) fn prioritize_task(&mut self, task_id: u64) {
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

    pub(super) fn cancel_task(&mut self, task_id: u64) {
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

    pub(super) fn rerun_task(&mut self, task_id: u64) {
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

    pub(super) fn delete_task(&mut self, task_id: u64) {
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

    pub(super) fn open_task_output_dir(&mut self, task_id: u64) {
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

    pub(super) fn handle_task_queue_row_event(&mut self, event: TaskQueueRowEvent) {
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

    pub(super) fn archive_path_buf(&self) -> std::result::Result<PathBuf, String> {
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

    pub(super) fn compress_sources_buf(&self) -> std::result::Result<Vec<PathBuf>, String> {
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

    pub(super) fn compress_output_path_buf(&self) -> std::result::Result<PathBuf, String> {
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

    pub(super) fn output_dir_buf(&self) -> std::result::Result<PathBuf, String> {
        let path = self.output_dir.trim();
        if path.is_empty() {
            return Err(self
                .t("Choose an output folder first.", "请先选择输出目录。")
                .to_string());
        }

        Ok(PathBuf::from(path))
    }

    pub(super) fn push_log(&mut self, line: impl Into<String>) {
        self.logs.push(line.into());
        if self.logs.len() > 200 {
            let drain = self.logs.len() - 200;
            self.logs.drain(0..drain);
        }
    }

    pub(super) fn show_toast(&mut self, tone: FeedbackTone, text: impl Into<String>) {
        self.toast = Some(ToastMessage {
            text: text.into(),
            tone,
            created_at: Instant::now(),
            duration: Duration::from_millis(2100),
        });
    }

    pub(super) fn running_task_count(&self) -> usize {
        self.task_queue
            .iter()
            .filter(|task| matches!(task.state, TaskState::Running | TaskState::Canceling))
            .count()
    }

    pub(super) fn queued_task_count(&self) -> usize {
        self.task_queue
            .iter()
            .filter(|task| task.state == TaskState::Queued)
            .count()
    }

    pub(super) fn workspace_action_busy(&self) -> bool {
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

    pub(super) fn live_status(&self) -> (String, bool, FeedbackTone) {
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

    pub(super) fn needs_live_animation(&self) -> bool {
        self.toast.is_some()
            || self.show_compress_source_picker
            || self.pending_dialog_action.is_some()
            || self.is_scanning_archive()
            || self.has_active_task()
    }

    pub(super) fn backend_label(&self, kind: BackendKind) -> &'static str {
        match kind {
            BackendKind::Native => self.t("Native Rust core", "原生 Rust 内核"),
            BackendKind::RarAdapter => self.t("RAR adapter", "RAR 适配层"),
        }
    }

    pub(super) fn poll_scan_jobs(&mut self, ctx: &Context) {
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

    pub(super) fn poll_task_jobs(&mut self, ctx: &Context) {
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

    pub(super) fn finish_task(&mut self, job: TaskJobResult) {
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

    pub(super) fn draw_top_bar(&mut self, ui: &mut egui::Ui) {
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
    pub(super) fn minimize_window(&self, ctx: &Context) {
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
    pub(super) fn minimize_window(&self, ctx: &Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }

    pub(super) fn draw_side_nav(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn side_nav_item(&mut self, ui: &mut egui::Ui, item: SideNavItem, label: &str) {
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

    pub(super) fn draw_workspace_dashboard(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn draw_file_manager_page(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn draw_file_manager_toolbar(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn checksum_selected_file_manager_path(&mut self) {
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

    pub(super) fn draw_checksum_dialog(&mut self, ctx: &Context) {
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
                                    RichText::new(copy_label).size(11.0).color(palette.primary),
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
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui
                                            .add(
                                                Button::new(
                                                    RichText::new(self.t("Copy", "复制"))
                                                        .size(11.0)
                                                        .color(palette.primary),
                                                )
                                                .fill(palette.surface_high)
                                                .stroke(Stroke::new(1.0, palette.outline_variant))
                                                .corner_radius(6.0)
                                                .min_size(Vec2::new(64.0, 26.0)),
                                            )
                                            .clicked()
                                        {
                                            ui.ctx().copy_text(result.hex_digest.clone());
                                        }
                                    });
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
                        .add(Button::new(self.t("Close", "关闭")).min_size(Vec2::new(96.0, 36.0)))
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

    pub(super) fn draw_file_manager_panel(&mut self, ui: &mut egui::Ui) {
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
                                        view_target = Some((entry.path.clone(), entry.is_dir));
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

    pub(super) fn draw_task_queue_page(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();

        paint_background(ui, palette);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                self.draw_task_queue_panel(ui);
            });
    }

    pub(super) fn draw_current_page(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn draw_workspace_action(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn draw_path_selectors(&mut self, ui: &mut egui::Ui) {
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
            let extra_test_button = if is_compress {
                0.0
            } else {
                button_width + field_gap
            };
            let field_width =
                ((ui.available_width() - button_width - extra_test_button - field_gap * 2.0) / 2.0)
                    .max(160.0);

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
                                    RichText::new(test_label).size(11.0).color(palette.text),
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
                        let test_button =
                            Button::new(RichText::new(test_label).size(11.0).color(palette.text))
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

    pub(super) fn apply_compression_format(&mut self, format: CompressionFormat) {
        if self.compression_options.format == format {
            return;
        }

        let previous_format = self.compression_options.format;
        let previous_suggested_output =
            suggested_archive_output_path(&self.compress_sources, previous_format, self.language)
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
                suggested_archive_output_path(&self.compress_sources, format, self.language)
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

    pub(super) fn compression_method_description_for(
        &self,
        options: &CompressionOptions,
    ) -> String {
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
            CompressionFormat::TarZst => self.t("Zstd stream", "Zstd 压缩流").to_string(),
            CompressionFormat::Zst => self.t("Zstd stream", "Zstd 压缩流").to_string(),
            CompressionFormat::TarLz4 => self.t("LZ4 stream", "LZ4 压缩流").to_string(),
            CompressionFormat::Lz4 => self.t("LZ4 stream", "LZ4 压缩流").to_string(),
        }
    }

    pub(super) fn compression_dictionary_text_for(&self, options: &CompressionOptions) -> String {
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

    pub(super) fn compression_word_size_text_for(&self, options: &CompressionOptions) -> String {
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

    pub(super) fn compression_solid_text_for(&self, options: &CompressionOptions) -> String {
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

    pub(super) fn compression_memory_text_for(&self, options: &CompressionOptions) -> String {
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
            (CompressionFormat::TarZst, _, _) => self.t("Medium (~32 MB)", "中（约 32 MB）"),
            (CompressionFormat::Lz4, _, _) => self.t("Very low (< 8 MB)", "很低（< 8 MB）"),
            (CompressionFormat::TarLz4, _, _) => self.t("Very low (< 8 MB)", "很低（< 8 MB）"),
            (CompressionFormat::Zip, _, _) => unreachable!("ZIP handled above"),
        };
        memory.to_string()
    }

    pub(super) fn draw_compression_options_panel(&mut self, ui: &mut egui::Ui) {
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
                                        self.t("AES-256", "AES-256")
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

    pub(super) fn draw_extract_options_panel(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn draw_queue_panel(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn draw_task_queue_panel(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn draw_task_output_path_dialog(&mut self, ctx: &Context) {
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

    pub(super) fn draw_selected_file_manager_paths_dialog(&mut self, ctx: &Context) {
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

    pub(super) fn draw_save_preset_dialog(&mut self, ctx: &Context) {
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
                        .add(Button::new(self.t("Cancel", "取消")).min_size(Vec2::new(80.0, 32.0)))
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
                                RichText::new(self.t("Save", "保存")).color(palette.on_primary),
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

    pub(super) fn draw_manage_presets_dialog(&mut self, ctx: &Context) {
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
                        RichText::new(self.t("No presets saved yet.", "尚未保存任何预设。"))
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
                                    ui.label(RichText::new(name).size(13.0).color(palette.text));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui
                                            .add(
                                                Button::new(
                                                    RichText::new(self.t("Delete", "删除"))
                                                        .size(11.0)
                                                        .color(palette.error),
                                                )
                                                .fill(palette.surface_high)
                                                .stroke(Stroke::new(1.0, palette.outline_variant))
                                                .corner_radius(6.0)
                                                .min_size(Vec2::new(64.0, 26.0)),
                                            )
                                            .clicked()
                                        {
                                            if let Err(e) = crate::settings::delete_preset(name) {
                                                self.show_toast(
                                                    FeedbackTone::Error,
                                                    self.language.format(
                                                        "Failed to delete preset: {error}",
                                                        "删除预设失败：{error}",
                                                        &[("{error}", format!("{e:#}"))],
                                                    ),
                                                );
                                            }
                                            // Force close/reopen to refresh list
                                            close_requested = true;
                                            ui.ctx().request_repaint();
                                        }
                                    });
                                });
                                ui.add_space(4.0);
                            }
                        });
                }

                ui.add_space(10.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(Button::new(self.t("Close", "关闭")).min_size(Vec2::new(96.0, 36.0)))
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

    pub(super) fn draw_test_result_dialog(&mut self, ctx: &Context) {
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
                        .add(Button::new(self.t("Close", "关闭")).min_size(Vec2::new(96.0, 36.0)))
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

    pub(super) fn draw_extract_conflict_dialog(&mut self, ctx: &Context) {
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

    pub(super) fn apply_extract_conflict_action(&mut self, action: ExtractConflictAction) {
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

    pub(super) fn validate_extract_conflict_rename(
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

    pub(super) fn draw_logs_page(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn trigger_startup_update_check(&mut self) {
        if !self.auto_update_enabled || self.update_check_pending {
            return;
        }
        self.update_check_pending = true;
        let (tx, rx) = std::sync::mpsc::channel();
        self.update_receiver = Some(rx);
        std::thread::spawn(move || {
            let result = crate::update::check_latest_release();
            if let Ok(Some(info)) = result {
                let _ = tx.send(info);
            }
        });
    }

    pub(super) fn poll_update_check(&mut self) {
        if let Some(ref rx) = self.update_receiver {
            if let Ok(info) = rx.try_recv() {
                self.update_info = Some(info);
                self.show_update_dialog = true;
                self.update_receiver = None;
            }
        }
    }

    pub(super) fn draw_update_dialog(&mut self, ctx: &Context) {
        if !self.show_update_dialog {
            return;
        }
        let palette = self.palette();
        let title = self.t("New Version Available", "发现新版本");
        let close_text = self.t("Later", "以后再说");
        let download_text = self.t("Download", "下载");

        let mut close = false;
        let mut open_download = false;

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(420.0, 280.0))
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                ui.add_space(12.0);

                if let Some(ref info) = self.update_info {
                    let version_label = if self.language == Language::ChineseSimplified {
                        format!("FastZIP {} 可供下载。", info.version)
                    } else {
                        format!("FastZIP {} is available for download.", info.version)
                    };
                    ui.label(RichText::new(version_label).size(14.0).color(palette.text));
                    ui.add_space(8.0);
                    if !info.body.is_empty() {
                        ui.label(
                            RichText::new(self.t("Release Notes:", "更新内容："))
                                .size(12.0)
                                .strong()
                                .color(palette.text_secondary),
                        );
                        ui.add_space(4.0);
                        let body = if info.body.len() > 500 {
                            format!("{}...", &info.body[..500])
                        } else {
                            info.body.clone()
                        };
                        ui.label(RichText::new(body).size(11.0).color(palette.text_muted));
                    }
                }

                ui.add_space(20.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let download_btn = Button::new(
                        RichText::new(download_text)
                            .size(14.0)
                            .strong()
                            .color(palette.text_secondary),
                    )
                    .fill(palette.primary)
                    .corner_radius(8.0)
                    .min_size(Vec2::new(100.0, 36.0));
                    if ui.add(download_btn).clicked() {
                        open_download = true;
                        close = true;
                    }

                    ui.add_space(12.0);

                    let later_btn = Button::new(
                        RichText::new(close_text)
                            .size(14.0)
                            .color(palette.text_muted),
                    )
                    .fill(palette.subtle_fill)
                    .stroke(Stroke::new(1.0, palette.subtle_stroke))
                    .corner_radius(8.0)
                    .min_size(Vec2::new(80.0, 36.0));
                    if ui.add(later_btn).clicked() {
                        close = true;
                    }
                });
            });

        if close {
            self.show_update_dialog = false;
        }
        if open_download {
            if let Some(ref info) = self.update_info {
                #[cfg(target_os = "windows")]
                {
                    let wide_url: Vec<u16> = std::ffi::OsStr::new(&info.download_url)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    unsafe {
                        ShellExecuteW(
                            std::ptr::null_mut(),
                            std::ptr::null(),
                            wide_url.as_ptr(),
                            std::ptr::null(),
                            std::ptr::null(),
                            1, // SW_SHOWNORMAL
                        );
                    }
                }
            }
        }
    }

    pub(super) fn draw_settings_page(&mut self, ui: &mut egui::Ui) {
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
                                .selected_text(self.locale.display_name())
                                .width(128.0)
                                .show_ui(ui, |ui| {
                                    for locale in supported_locales() {
                                        if ui
                                            .selectable_label(
                                                self.locale == locale,
                                                locale.display_name(),
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
                                RichText::new(self.t("Auto Update", "自动更新"))
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.t(
                                    "Check for new versions on startup.",
                                    "启动时检查新版本。",
                                ))
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(if self.auto_update_enabled {
                                    self.t("Enabled", "已开启")
                                } else {
                                    self.t("Disabled", "已关闭")
                                })
                                .size(11.0)
                                .strong()
                                .color(if self.auto_update_enabled {
                                    palette.primary
                                } else {
                                    palette.text_muted
                                }),
                            );
                            ui.add_space(12.0);

                            if theme_switch(ui, "auto-update", self.auto_update_enabled, palette) {
                                self.auto_update_enabled = !self.auto_update_enabled;
                                if let Err(e) = save_auto_update_enabled(self.auto_update_enabled) {
                                    self.push_log(format!(
                                        "Failed to update auto-update setting: {e}",
                                    ));
                                }
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

                });
            });
    }

    pub(super) fn handle_queue_target(&mut self, target: QueueRowTarget) {
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

    pub(super) fn is_compress_root_path(&self, path: &Path) -> bool {
        self.compress_sources.iter().any(|source| source == path)
    }

    pub(super) fn is_path_within_any_compress_source(&self, path: &Path) -> bool {
        self.compress_sources
            .iter()
            .any(|source| path == source || path.starts_with(source))
    }

    pub(super) fn is_path_excluded_from_compress(&self, path: &Path) -> bool {
        self.compress_excluded_paths
            .iter()
            .any(|excluded| path == excluded || path.starts_with(excluded))
    }

    pub(super) fn preview_archive_entry(&mut self, relative_path: &Path) -> Result<()> {
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
                    .unwrap_or_else(|| self.t("archive", "归档").to_string()),
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

    pub(super) fn compress_sources_display(&self) -> String {
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

    pub(super) fn collect_file_manager_entries(&self) -> Result<Vec<FileManagerEntry>> {
        let mut entries = fs::read_dir(&self.file_manager_current_dir)
            .with_context(|| {
                self.language.format(
                    "Failed to open file manager directory {path}",
                    "无法打开文件管理器目录 {path}",
                    &[(
                        "{path}",
                        self.file_manager_current_dir.display().to_string(),
                    )],
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
                        format_size(Some(metadata.len()), self.language)
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

    pub(super) fn queue_rows(&self) -> Vec<QueueRowData> {
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

    pub(super) fn compress_rows(&self) -> Vec<QueueRowData> {
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

    pub(super) fn extract_rows(&self) -> Vec<QueueRowData> {
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
                        format_size(entry.uncompressed_size, self.language)
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

    pub(super) fn compress_root_rows(&self) -> Vec<QueueRowData> {
        self.compress_sources
            .iter()
            .filter(|source| !self.is_path_excluded_from_compress(source))
            .map(|source| self.compress_path_row(source))
            .collect()
    }

    pub(super) fn compress_child_rows(&self, current_dir: &Path) -> Vec<QueueRowData> {
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

    pub(super) fn compress_path_row(&self, path: &Path) -> QueueRowData {
        let is_dir = path.is_dir();
        let size = if is_dir {
            "-".to_string()
        } else {
            format_size(path.metadata().ok().map(|meta| meta.len()), self.language)
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

    pub(super) fn task_metrics(
        &self,
        _index: usize,
        task: &TaskQueueItem,
        now: Instant,
    ) -> TaskQueueMetrics {
        let progress = real_task_progress(task);
        let speed_text = task
            .current_bytes_per_second
            .map(|v| format_transfer_rate(v, self.language))
            .unwrap_or_else(|| self.t("--", "--").to_string());
        let is_finalizing = task_is_finalizing(task);
        let eta_text = match task.state {
            TaskState::Queued => self.t("--", "--").to_string(),
            TaskState::Running if is_finalizing => self.t("Finalizing", "收尾中").to_string(),
            TaskState::Running => running_task_eta(task, now)
                .map(|d| format_duration_compact(d, self.language))
                .unwrap_or_else(|| self.t("--", "--").to_string()),
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
                self.language.format(
                    "{progress}%",
                    "{progress}%",
                    &[("{progress}", format!("{:.0}", progress * 100.0))],
                )
            },
            speed_text,
            eta_text,
            status_text: task.state.label(self.language).to_string(),
        }
    }

    pub(super) fn draw_footer(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let version_label = self
            .t("v{version}", "v{version}")
            .replace("{version}", env!("CARGO_PKG_VERSION"));

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
                    .selected_text(self.locale.display_name())
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for locale in supported_locales() {
                            if ui
                                .selectable_label(self.locale == locale, locale.display_name())
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

    pub(super) fn draw_feedback_toast(&mut self, ctx: &Context) {
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

    pub(super) fn draw_drop_overlay(&self, ctx: &Context) {
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
