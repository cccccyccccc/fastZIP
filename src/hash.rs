use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Crc32,
    Sha256,
    Blake3,
}

impl HashAlgorithm {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Crc32 => "CRC-32",
            Self::Sha256 => "SHA-256",
            Self::Blake3 => "BLAKE3",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Sha256, Self::Blake3, Self::Crc32]
    }
}

#[derive(Debug, Clone)]
pub struct ChecksumResult {
    pub algorithm: HashAlgorithm,
    pub hex_digest: String,
    pub file_size: u64,
    pub elapsed: std::time::Duration,
}

pub fn file_checksum(path: &Path, algorithm: HashAlgorithm) -> Result<ChecksumResult> {
    let started = Instant::now();
    let mut file = File::open(path).with_context(|| format!("Cannot open: {}", path.display()))?;
    let mut buffer = [0u8; 65536];
    let mut total_bytes: u64 = 0;

    match algorithm {
        HashAlgorithm::Crc32 => {
            let mut hasher = crc32fast::Hasher::new();
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
                total_bytes += n as u64;
            }
            Ok(ChecksumResult {
                algorithm,
                hex_digest: format!("{:08x}", hasher.finalize()),
                file_size: total_bytes,
                elapsed: started.elapsed(),
            })
        }
        HashAlgorithm::Sha256 => {
            use sha2::Digest;
            let mut hasher = sha2::Sha256::new();
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
                total_bytes += n as u64;
            }
            let digest = hasher.finalize();
            let hex = digest.iter().map(|b| format!("{:02x}", b)).collect::<String>();
            Ok(ChecksumResult {
                algorithm,
                hex_digest: hex,
                file_size: total_bytes,
                elapsed: started.elapsed(),
            })
        }
        HashAlgorithm::Blake3 => {
            let mut hasher = blake3::Hasher::new();
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
                total_bytes += n as u64;
            }
            Ok(ChecksumResult {
                algorithm,
                hex_digest: hasher.finalize().to_hex().to_string(),
                file_size: total_bytes,
                elapsed: started.elapsed(),
            })
        }
    }
}

pub fn file_checksums(path: &Path) -> Result<Vec<ChecksumResult>> {
    HashAlgorithm::all()
        .iter()
        .map(|&algo| file_checksum(path, algo))
        .collect()
}
