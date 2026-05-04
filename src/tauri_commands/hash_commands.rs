use std::path::PathBuf;

use crate::hash::{self, ChecksumResult, HashAlgorithm};

#[tauri::command]
pub fn calculate_checksum(path: PathBuf, algorithm: String) -> Result<ChecksumResult, String> {
    let algo = match algorithm.to_lowercase().as_str() {
        "sha256" => HashAlgorithm::Sha256,
        "blake3" => HashAlgorithm::Blake3,
        "crc32" => HashAlgorithm::Crc32,
        other => return Err(format!("Unknown hash algorithm: {other}")),
    };
    hash::file_checksum(&path, algo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn calculate_all_checksums(path: PathBuf) -> Result<Vec<ChecksumResult>, String> {
    hash::file_checksums(&path).map_err(|e| e.to_string())
}
