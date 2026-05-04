use std::env;

use crate::localization;
use crate::settings;

#[tauri::command]
pub fn get_language() -> Result<String, String> {
    Ok(localization::current_locale().code.to_string())
}

#[tauri::command]
pub fn set_language(code: String) -> Result<(), String> {
    localization::set_current_locale_by_code(&code);
    settings::save_preferred_language_value(&code).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_supported_locales() -> Vec<serde_json::Value> {
    localization::supported_locales()
        .iter()
        .map(|locale| {
            serde_json::json!({
                "code": locale.code,
                "name_en": locale.name_en,
                "name_zh": locale.name_zh,
            })
        })
        .collect()
}

#[tauri::command]
pub fn get_theme() -> String {
    settings::load_preferred_theme().unwrap_or_else(|| "system".to_string())
}

#[tauri::command]
pub fn set_theme(theme: String) -> Result<(), String> {
    settings::save_preferred_theme_value(&theme).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_autostart_enabled() -> bool {
    settings::load_autostart_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
    let exe_path = env::current_exe().map_err(|e| e.to_string())?;
    settings::save_autostart_enabled(&exe_path, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_auto_update_enabled() -> bool {
    settings::load_auto_update_enabled()
}

#[tauri::command]
pub fn set_auto_update_enabled(enabled: bool) -> Result<(), String> {
    settings::save_auto_update_enabled(enabled).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Presets ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_presets() -> Vec<String> {
    settings::list_preset_names()
}

#[tauri::command]
pub fn load_preset(name: String) -> Result<String, String> {
    settings::load_preset(&name).ok_or_else(|| format!("Preset '{name}' not found"))
}

#[tauri::command]
pub fn save_preset(name: String, value: String) -> Result<(), String> {
    settings::save_preset(&name, &value).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_preset(name: String) -> Result<(), String> {
    settings::delete_preset(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_translations(code: String) -> std::collections::HashMap<String, String> {
    crate::localization::translations_for(&code)
}

#[tauri::command]
pub fn encode_preset_value(
    format: crate::archive::CompressionFormat,
    level: crate::archive::CompressionLevel,
    method: crate::archive::ZipCompressionMethod,
    threads: u32,
    encrypt_file_names: bool,
) -> String {
    settings::encode_preset_value(format, level, method, threads, encrypt_file_names)
}
