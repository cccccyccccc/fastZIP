use std::env;
#[cfg(target_os = "windows")]
use std::ffi::OsStr;
use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use crate::archive::{CompressionFormat, CompressionLevel, ZipCompressionMethod};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
    RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};

const SETTINGS_SECTION: &str = "ui";
const LANGUAGE_KEY: &str = "language";
const THEME_KEY: &str = "theme";
const SETTINGS_FILE_NAME: &str = "settings.ini";
const SETTINGS_DIR_NAME: &str = "FastZIP";
#[cfg(target_os = "windows")]
const AUTOSTART_RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(target_os = "windows")]
const AUTOSTART_VALUE_NAME: &str = "FastZIP";

pub fn load_preferred_language_value() -> Option<String> {
    for path in settings_search_paths() {
        let Some(contents) = read_settings_file(&path) else {
            continue;
        };
        if let Some(value) = parse_ini_value(&contents, SETTINGS_SECTION, LANGUAGE_KEY) {
            return Some(value);
        }
    }

    None
}

pub fn save_preferred_language_value(value: &str) -> Result<PathBuf> {
    let path = user_settings_path()
        .ok_or_else(|| anyhow!("LOCALAPPDATA is not available for FastZIP settings"))?;
    let theme = read_settings_key(&path, THEME_KEY);
    write_settings_file(&path, Some(value), theme.as_deref())?;
    Ok(path)
}

pub fn load_preferred_theme() -> Option<String> {
    for path in settings_search_paths() {
        if let Some(value) = read_settings_key(&path, THEME_KEY) {
            return Some(value);
        }
    }
    None
}

pub fn save_preferred_theme_value(value: &str) -> Result<PathBuf> {
    let path = user_settings_path()
        .ok_or_else(|| anyhow!("LOCALAPPDATA is not available for FastZIP settings"))?;
    let language = read_settings_key(&path, LANGUAGE_KEY);
    write_settings_file(&path, language.as_deref(), Some(value))?;
    Ok(path)
}

#[cfg(target_os = "windows")]
pub fn load_autostart_enabled() -> Result<bool> {
    if autostart_value_in_key(HKEY_CURRENT_USER)? {
        return Ok(true);
    }
    if autostart_value_in_key(HKEY_LOCAL_MACHINE)? {
        return Ok(true);
    }
    Ok(false)
}

#[cfg(not(target_os = "windows"))]
pub fn load_autostart_enabled() -> Result<bool> {
    Ok(false)
}

#[cfg(target_os = "windows")]
pub fn save_autostart_enabled(executable_path: &Path, enabled: bool) -> Result<()> {
    if enabled {
        let key = create_current_user_run_key(KEY_SET_VALUE)?;
        let resolved_path = executable_path
            .canonicalize()
            .unwrap_or_else(|_| executable_path.to_path_buf());
        let command = format!("\"{}\" gui", resolved_path.display());
        let value_name = wide_null(OsStr::new(AUTOSTART_VALUE_NAME));
        let value = wide_null(OsStr::new(command.as_str()));
        let data_len = (value.len() * std::mem::size_of::<u16>()) as u32;
        let status = unsafe {
            RegSetValueExW(
                key.0,
                value_name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast::<u8>(),
                data_len,
            )
        };
        if status != 0 {
            return Err(anyhow!(
                "Failed to enable FastZIP autostart (Windows error code {status})"
            ));
        }
        return Ok(());
    }

    delete_autostart_value(HKEY_CURRENT_USER)?;
    delete_autostart_value(HKEY_LOCAL_MACHINE)?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn save_autostart_enabled(_executable_path: &Path, _enabled: bool) -> Result<()> {
    Err(anyhow!("Autostart is not supported on this platform"))
}

#[cfg(target_os = "windows")]
pub fn delete_autostart_hklm_value() -> Result<()> {
    delete_autostart_value(HKEY_LOCAL_MACHINE)
}

fn settings_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(path) = user_settings_path() {
        paths.push(path);
    }

    if let Some(path) = common_settings_path() {
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }

    paths
}

fn user_settings_path() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|root| root.join(SETTINGS_DIR_NAME).join(SETTINGS_FILE_NAME))
}

fn common_settings_path() -> Option<PathBuf> {
    env::var_os("PROGRAMDATA")
        .map(PathBuf::from)
        .map(|root| root.join(SETTINGS_DIR_NAME).join(SETTINGS_FILE_NAME))
}

fn read_settings_file(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn read_settings_key(path: &Path, key: &str) -> Option<String> {
    read_settings_file(path).and_then(|contents| parse_ini_value(&contents, SETTINGS_SECTION, key))
}

fn write_settings_file(path: &Path, language_value: Option<&str>, theme_value: Option<&str>) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Settings file path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create settings directory {}", parent.display()))?;

    let mut lines = vec![
        "; FastZIP user preferences".to_string(),
        format!("[{}]", SETTINGS_SECTION),
    ];
    if let Some(lang) = language_value {
        lines.push(format!("{}={}", LANGUAGE_KEY, lang));
    }
    if let Some(theme) = theme_value {
        lines.push(format!("{}={}", THEME_KEY, theme));
    }
    let contents = lines.join("\r\n") + "\r\n";
    fs::write(path, contents)
        .with_context(|| format!("Failed to write settings file {}", path.display()))?;
    Ok(())
}

fn parse_ini_value(contents: &str, target_section: &str, target_key: &str) -> Option<String> {
    let mut current_section = String::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if let Some(section_name) = line
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .map(str::trim)
        {
            current_section.clear();
            current_section.push_str(section_name);
            continue;
        }

        if !current_section.eq_ignore_ascii_case(target_section) {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case(target_key) {
            continue;
        }

        let normalized = value.trim().trim_matches('"').trim_matches('\'');
        if normalized.is_empty() {
            return None;
        }
        return Some(normalized.to_string());
    }

    None
}

#[cfg(target_os = "windows")]
struct RegistryKey(HKEY);

#[cfg(target_os = "windows")]
impl Drop for RegistryKey {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn create_current_user_run_key(_access: u32) -> Result<RegistryKey> {
    let subkey = wide_null(OsStr::new(AUTOSTART_RUN_SUBKEY));
    let mut key: HKEY = std::ptr::null_mut();
    let status = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, subkey.as_ptr(), &mut key) };
    if status != 0 {
        return Err(anyhow!(
            "Failed to create FastZIP autostart registry key (Windows error code {status})"
        ));
    }
    Ok(RegistryKey(key))
}

#[cfg(target_os = "windows")]
fn autostart_value_in_key(root: HKEY) -> Result<bool> {
    let subkey = wide_null(OsStr::new(AUTOSTART_RUN_SUBKEY));
    let mut key: HKEY = std::ptr::null_mut();
    let status = unsafe { RegOpenKeyExW(root, subkey.as_ptr(), 0, KEY_QUERY_VALUE, &mut key) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    if status != 0 {
        return Ok(false);
    }
    let _guard = RegistryKey(key);

    let value_name = wide_null(OsStr::new(AUTOSTART_VALUE_NAME));
    let mut value_type = 0u32;
    let mut data_len = 0u32;
    let status = unsafe {
        RegQueryValueExW(
            key,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &mut value_type,
            std::ptr::null_mut(),
            &mut data_len,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    if status != 0 {
        return Ok(false);
    }
    Ok(value_type == REG_SZ && data_len >= 2)
}

#[cfg(target_os = "windows")]
fn delete_autostart_value(root: HKEY) -> Result<()> {
    let subkey = wide_null(OsStr::new(AUTOSTART_RUN_SUBKEY));
    let mut key: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(root, subkey.as_ptr(), 0, KEY_SET_VALUE | KEY_QUERY_VALUE, &mut key)
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    if status == ERROR_ACCESS_DENIED {
        if root == HKEY_LOCAL_MACHINE {
            return Err(anyhow!(
                "FastZIP autostart was installed for all users (HKLM) and cannot be disabled \
                 without administrator privileges.\n\n\
                 Run FastZIP as administrator once to turn off autostart, \
                 or delete the 'FastZIP' value manually from:\n\
                 HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Run"
            ));
        }
        return Err(anyhow!(
            "Cannot access the Windows Run registry key. \
             Try running FastZIP as administrator to change autostart."
        ));
    }
    if status != 0 {
        return Ok(());
    }
    let _guard = RegistryKey(key);

    let value_name = wide_null(OsStr::new(AUTOSTART_VALUE_NAME));
    let status = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    if status == 0 {
        return Ok(());
    }
    if status == ERROR_ACCESS_DENIED {
        if root == HKEY_LOCAL_MACHINE {
            return Err(anyhow!(
                "FastZIP autostart was installed for all users (HKLM) and cannot be disabled \
                 without administrator privileges.\n\n\
                 Run FastZIP as administrator once to turn off autostart, \
                 or delete the 'FastZIP' value manually from:\n\
                 HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Run"
            ));
        }
        return Err(anyhow!(
            "Cannot remove the FastZIP autostart value from the registry. \
             Try running FastZIP as administrator."
        ));
    }
    Err(anyhow!(
        "Failed to disable FastZIP autostart (Windows error code {status})"
    ))
}

#[cfg(target_os = "windows")]
fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

// --- Presets ---

const PRESETS_SECTION: &str = "presets";

pub fn load_preset(name: &str) -> Option<String> {
    for path in settings_search_paths() {
        let Some(contents) = read_settings_file(&path) else {
            continue;
        };
        if let Some(value) = parse_ini_value(&contents, PRESETS_SECTION, name) {
            return Some(value);
        }
    }
    None
}

pub fn save_preset(name: &str, value: &str) -> Result<PathBuf> {
    let path = user_settings_path()
        .ok_or_else(|| anyhow!("LOCALAPPDATA is not available for FastZIP settings"))?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Settings file path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create settings directory {}", parent.display()))?;

    let mut content = String::new();
    let mut found_section = false;
    let mut replaced = false;

    if let Ok(existing) = fs::read_to_string(&path) {
        let mut in_presets = false;
        for raw_line in existing.lines() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
                content.push_str(raw_line);
                content.push_str("\r\n");
                if in_presets && !found_section {
                    content.push_str(&format!("{}={}\r\n", name, value));
                    replaced = true;
                    found_section = true;
                }
                continue;
            }
            if let Some(section_name) = trimmed
                .strip_prefix('[')
                .and_then(|v| v.strip_suffix(']'))
                .map(str::trim)
            {
                if in_presets && !found_section {
                    content.push_str(&format!("{}={}\r\n", name, value));
                    replaced = true;
                    found_section = true;
                }
                in_presets = section_name.eq_ignore_ascii_case(PRESETS_SECTION);
                found_section = found_section || in_presets;
                content.push_str(raw_line);
                content.push_str("\r\n");
                continue;
            }
            if in_presets && !replaced {
                if let Some((key, _existing_value)) = trimmed.split_once('=') {
                    if key.trim().eq_ignore_ascii_case(name) {
                        content.push_str(&format!("{}={}\r\n", name, value));
                        replaced = true;
                        continue;
                    }
                }
            }
            content.push_str(raw_line);
            content.push_str("\r\n");
        }
    }

    if !found_section {
        if !content.is_empty() && !content.ends_with("\r\n") {
            content.push_str("\r\n");
        }
        content.push_str(&format!("[{}]\r\n", PRESETS_SECTION));
        content.push_str(&format!("{}={}\r\n", name, value));
    } else if !replaced {
        if !content.ends_with("\r\n") {
            content.push_str("\r\n");
        }
        content.push_str(&format!("{}={}\r\n", name, value));
    }

    fs::write(&path, content)
        .with_context(|| format!("Failed to write settings file {}", path.display()))?;
    Ok(path)
}

pub fn list_preset_names() -> Vec<String> {
    let mut names = Vec::new();
    for path in settings_search_paths() {
        let Some(contents) = read_settings_file(&path) else {
            continue;
        };
        let mut in_presets = false;
        for raw_line in contents.lines() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
                continue;
            }
            if let Some(section_name) = trimmed
                .strip_prefix('[')
                .and_then(|v| v.strip_suffix(']'))
                .map(str::trim)
            {
                in_presets = section_name.eq_ignore_ascii_case(PRESETS_SECTION);
                continue;
            }
            if in_presets {
                if let Some((key, _value)) = trimmed.split_once('=') {
                    let name = key.trim().to_string();
                    if !name.is_empty() && !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
    }
    names.sort();
    names
}

pub fn encode_preset_value(
    format: CompressionFormat,
    level: CompressionLevel,
    method: ZipCompressionMethod,
    threads: u32,
    encrypt_file_names: bool,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        format.primary_suffix(),
        level_name(level),
        method_name(method),
        threads,
        if encrypt_file_names { "yes" } else { "no" }
    )
}

pub fn decode_preset_value(value: &str) -> Result<(CompressionFormat, CompressionLevel, ZipCompressionMethod, u32, bool)> {
    let parts: Vec<&str> = value.split('|').collect();
    if parts.len() != 5 {
        bail!("Invalid preset value: expected 5 fields, got {}", parts.len());
    }
    let format = CompressionFormat::detect(&std::path::PathBuf::from(format!("dummy{}", parts[0])))?;
    let level = parse_level_name(parts[1])?;
    let method = parse_method_name(parts[2])?;
    let threads: u32 = parts[3].parse().unwrap_or(1);
    let encrypt = parts[4] == "yes";
    Ok((format, level, method, threads, encrypt))
}

fn level_name(level: CompressionLevel) -> &'static str {
    match level {
        CompressionLevel::Fastest => "fastest",
        CompressionLevel::Fast => "fast",
        CompressionLevel::Normal => "normal",
        CompressionLevel::Maximum => "maximum",
        CompressionLevel::Ultra => "ultra",
    }
}

fn parse_level_name(name: &str) -> Result<CompressionLevel> {
    match name.to_lowercase().as_str() {
        "fastest" => Ok(CompressionLevel::Fastest),
        "fast" => Ok(CompressionLevel::Fast),
        "normal" => Ok(CompressionLevel::Normal),
        "maximum" => Ok(CompressionLevel::Maximum),
        "ultra" => Ok(CompressionLevel::Ultra),
        other => bail!("Unknown compression level: {other}"),
    }
}

fn method_name(method: ZipCompressionMethod) -> &'static str {
    match method {
        ZipCompressionMethod::Deflate => "deflate",
        ZipCompressionMethod::Stored => "stored",
        ZipCompressionMethod::Bzip2 => "bzip2",
        ZipCompressionMethod::Zstd => "zstd",
        ZipCompressionMethod::Xz => "xz",
    }
}

fn parse_method_name(name: &str) -> Result<ZipCompressionMethod> {
    match name.to_lowercase().as_str() {
        "deflate" => Ok(ZipCompressionMethod::Deflate),
        "stored" => Ok(ZipCompressionMethod::Stored),
        "bzip2" => Ok(ZipCompressionMethod::Bzip2),
        "zstd" => Ok(ZipCompressionMethod::Zstd),
        "xz" => Ok(ZipCompressionMethod::Xz),
        other => bail!("Unknown ZIP compression method: {other}"),
    }
}

pub fn delete_preset(name: &str) -> Result<()> {
    let path = user_settings_path()
        .ok_or_else(|| anyhow!("LOCALAPPDATA is not available for FastZIP settings"))?;
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(_) => return Ok(()),
    };

    let mut output = String::new();
    let mut in_presets = false;
    for raw_line in contents.lines() {
        let trimmed = raw_line.trim();
        if in_presets
            && !trimmed.is_empty()
            && !trimmed.starts_with(';')
            && !trimmed.starts_with('#')
            && !trimmed.starts_with('[')
        {
            if let Some((key, _value)) = trimmed.split_once('=') {
                if key.trim().eq_ignore_ascii_case(name) {
                    continue;
                }
            }
        }
        if let Some(section_name) = trimmed
            .strip_prefix('[')
            .and_then(|v| v.strip_suffix(']'))
            .map(str::trim)
        {
            in_presets = section_name.eq_ignore_ascii_case(PRESETS_SECTION);
        }
        output.push_str(raw_line);
        output.push_str("\r\n");
    }

    fs::write(&path, output)
        .with_context(|| format!("Failed to write settings file {}", path.display()))?;
    Ok(())
}
