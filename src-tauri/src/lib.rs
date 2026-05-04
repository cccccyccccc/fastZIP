use fastzip::tauri_commands::{
    archive_commands, benchmark_commands, file_manager_commands, hash_commands, settings_commands,
    update_commands,
};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            archive_commands::init_app_state(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            archive_commands::inspect_archive,
            archive_commands::list_archive,
            archive_commands::test_archive,
            archive_commands::get_backend_statuses,
            archive_commands::start_extract,
            archive_commands::start_compress,
            archive_commands::cancel_archive_task,
            settings_commands::get_language,
            settings_commands::set_language,
            settings_commands::get_supported_locales,
            settings_commands::get_theme,
            settings_commands::set_theme,
            settings_commands::get_autostart_enabled,
            settings_commands::set_autostart_enabled,
            settings_commands::get_auto_update_enabled,
            settings_commands::set_auto_update_enabled,
            settings_commands::list_presets,
            settings_commands::load_preset,
            settings_commands::save_preset,
            settings_commands::delete_preset,
            settings_commands::encode_preset_value,
            settings_commands::get_translations,
            hash_commands::calculate_checksum,
            hash_commands::calculate_all_checksums,
            update_commands::check_for_updates,
            benchmark_commands::run_benchmark,
            file_manager_commands::list_directory,
            file_manager_commands::get_file_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FastZIP");
}
