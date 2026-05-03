#![windows_subsystem = "windows"]
#![allow(dead_code, unused_imports)]

use std::io::{Read, Seek, SeekFrom};
use std::process::Command;

#[path = "../archive/mod.rs"]
mod archive;
#[path = "../amsi.rs"]
mod amsi;
#[path = "../encoding.rs"]
mod encoding;

use archive::{ArchiveService, ExtractOptions, ExtractPathPlan, OverwriteMode};

/// Magic bytes marking the start of the SFX footer.
const SFX_MAGIC: &[u8; 8] = b"FSTZPSFX";

/// Footer: [MAGIC:8][format_tag:u32 LE:4][archive_size:u64 LE:8] = 20 bytes
const FOOTER_SIZE: u64 = 20;

fn map_format_tag(tag: u32) -> Option<&'static str> {
    match tag {
        0 => Some(".zip"),
        1 => Some(".7z"),
        2 => Some(".tar"),
        3 => Some(".tar.gz"),
        4 => Some(".tar.bz2"),
        5 => Some(".tar.xz"),
        6 => Some(".tar.zst"),
        7 => Some(".tar.lz4"),
        8 => Some(".gz"),
        9 => Some(".bz2"),
        10 => Some(".xz"),
        11 => Some(".zst"),
        12 => Some(".lz4"),
        _ => None,
    }
}

fn main() {
    if let Err(e) = run() {
        show_error(&format!("{e:#}"));
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    let mut exe = std::fs::File::open(&exe_path)?;

    let exe_len = exe.metadata()?.len();
    if exe_len < FOOTER_SIZE {
        return Err("Invalid SFX executable (too small)".into());
    }

    // Read footer at end of file
    exe.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))?;
    let mut footer = [0u8; FOOTER_SIZE as usize];
    exe.read_exact(&mut footer)?;

    if &footer[0..8] != SFX_MAGIC {
        return Err("Invalid SFX executable (magic not found)".into());
    }

    let format_tag = u32::from_le_bytes(footer[8..12].try_into().unwrap());
    let archive_size = u64::from_le_bytes(footer[12..20].try_into().unwrap()) as usize;

    let ext = map_format_tag(format_tag).ok_or("Unknown SFX format tag")?;

    // Read archive data
    let archive_offset = exe_len - FOOTER_SIZE - archive_size as u64;
    exe.seek(SeekFrom::Start(archive_offset))?;
    let mut archive_data = vec![0u8; archive_size];
    exe.read_exact(&mut archive_data)?;

    // Write archive to temp directory with correct extension
    let temp_dir = std::env::temp_dir().join(format!("fastzip_sfx_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    let archive_path = temp_dir.join(format!("archive{ext}"));
    std::fs::write(&archive_path, &archive_data)?;

    // Extract
    let output_dir = temp_dir.join("extracted");
    std::fs::create_dir_all(&output_dir)?;

    let service = ArchiveService::new();
    let options = ExtractOptions {
        output_dir: output_dir.clone(),
        overwrite_mode: OverwriteMode::Overwrite,
        keep_paths: true,
        password: None,
        filename_encoding: encoding::FilenameEncoding::Utf8,
        scan_files: false,
    };

    service.extract_archive_with_progress_and_cancel_with_plan(
        &archive_path,
        &options,
        &ExtractPathPlan::default(),
        &mut |_| {},
        &mut || false,
    )?;

    // Open Explorer
    let _ = Command::new("explorer")
        .arg(output_dir.as_os_str())
        .spawn();

    Ok(())
}

fn show_error(msg: &str) {
    let _ = native_windows_dialog(msg);
}

#[cfg(windows)]
fn native_windows_dialog(msg: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    let wide_msg: Vec<u16> = OsStr::new(msg)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let wide_title: Vec<u16> = OsStr::new("FastZIP SFX Error")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            std::ptr::null_mut(),
            wide_msg.as_ptr(),
            wide_title.as_ptr(),
            0x00000010, // MB_ICONERROR
        );
    }
}

#[cfg(not(windows))]
fn native_windows_dialog(msg: &str) {
    eprintln!("{msg}");
}
