use crate::update::{self, ReleaseInfo};

#[tauri::command]
pub fn check_for_updates() -> Result<Option<ReleaseInfo>, String> {
    update::check_latest_release().map_err(|e| e.to_string())
}
