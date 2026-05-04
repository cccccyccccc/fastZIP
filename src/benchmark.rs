use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::archive::{
    ArchiveService, CompressionFormat, CompressionLevel, CompressionOptions, ZipCompressionMethod,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEntry {
    pub format: CompressionFormat,
    pub level: CompressionLevel,
    pub input_bytes: u64,
    pub output_bytes: u64,
    #[serde(with = "crate::serde_helpers::serde_duration")]
    pub elapsed: std::time::Duration,
}

impl BenchmarkEntry {
    pub fn compression_ratio(&self) -> f64 {
        if self.input_bytes == 0 {
            return 0.0;
        }
        self.output_bytes as f64 / self.input_bytes as f64
    }

    pub fn throughput_mbps(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs == 0.0 {
            return 0.0;
        }
        (self.input_bytes as f64 / secs) / (1024.0 * 1024.0)
    }
}

pub enum DataKind {
    Compressible,
    Incompressible,
}

/// Run a full benchmark suite across all format/level combinations.
pub fn run_benchmark(out_dir: &std::path::Path) -> Result<Vec<BenchmarkEntry>> {
    let service = ArchiveService::new();
    let mut results = Vec::new();

    let compressible = generate_compressible_data(1024 * 1024); // 1 MB
    let incompressible = generate_incompressible_data(1024 * 1024); // 1 MB

    let formats: &[(CompressionFormat, &str, DataKind)] = &[
        (
            CompressionFormat::Zip,
            "compressible.zip",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::Zip,
            "incompressible.zip",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::SevenZip,
            "compressible.7z",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::SevenZip,
            "incompressible.7z",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::TarGz,
            "compressible.tar.gz",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::TarGz,
            "incompressible.tar.gz",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::TarBz2,
            "compressible.tar.bz2",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::TarBz2,
            "incompressible.tar.bz2",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::TarXz,
            "compressible.tar.xz",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::TarXz,
            "incompressible.tar.xz",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::TarZst,
            "compressible.tar.zst",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::TarZst,
            "incompressible.tar.zst",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::TarLz4,
            "compressible.tar.lz4",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::TarLz4,
            "incompressible.tar.lz4",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::Gz,
            "compressible.gz",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::Gz,
            "incompressible.gz",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::Bz2,
            "compressible.bz2",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::Bz2,
            "incompressible.bz2",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::Xz,
            "compressible.xz",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::Xz,
            "incompressible.xz",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::Zst,
            "compressible.zst",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::Zst,
            "incompressible.zst",
            DataKind::Incompressible,
        ),
        (
            CompressionFormat::Lz4,
            "compressible.lz4",
            DataKind::Compressible,
        ),
        (
            CompressionFormat::Lz4,
            "incompressible.lz4",
            DataKind::Incompressible,
        ),
    ];

    let levels = &[
        CompressionLevel::Fastest,
        CompressionLevel::Normal,
        CompressionLevel::Maximum,
    ];

    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("Failed to create benchmark dir: {}", out_dir.display()))?;

    let source_dir = out_dir.join("source");
    std::fs::create_dir_all(&source_dir)?;

    // Use a single temp directory for all benchmark source data
    let temp_dir = tempfile::tempdir().context("Failed to create benchmark temp dir")?;

    for (format, filename, data_kind) in formats {
        let data = match data_kind {
            DataKind::Compressible => &compressible,
            DataKind::Incompressible => &incompressible,
        };

        let src = temp_dir.path().join("input.dat");
        std::fs::write(&src, data)?;

        for &level in levels {
            // Skip levels that don't make sense for certain formats
            if matches!(format, CompressionFormat::Lz4) && level == CompressionLevel::Maximum {
                continue;
            }

            let output = out_dir.join(format!("{}_{}", level_name(level), filename));
            let _ = std::fs::remove_file(&output);

            let options = CompressionOptions {
                format: *format,
                level,
                zip_method: ZipCompressionMethod::Deflate,
                thread_count: 1,
                password: None,
                encrypt_file_names: false,
                split_volume_size: None,
                sfx: false,
            };

            let started = Instant::now();
            let report = service.compress_with_options(&[src.clone()], &output, options)?;
            let elapsed = started.elapsed();

            results.push(BenchmarkEntry {
                format: *format,
                level,
                input_bytes: report.input_bytes,
                output_bytes: report.output_bytes,
                elapsed,
            });
        }
    }

    Ok(results)
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

fn generate_compressible_data(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let pattern =
        b"Hello, FastZIP benchmark! This text repeats to produce highly compressible data.\n";
    while data.len() < size {
        let remaining = size - data.len();
        let chunk = pattern.len().min(remaining);
        data.extend_from_slice(&pattern[..chunk]);
    }
    data
}

fn generate_incompressible_data(size: usize) -> Vec<u8> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut data = Vec::with_capacity(size);
    let mut state = 42u64;
    while data.len() < size {
        let mut hasher = DefaultHasher::new();
        state.hash(&mut hasher);
        let hash = hasher.finish();
        data.extend_from_slice(&hash.to_le_bytes());
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
    }
    data.truncate(size);
    data
}
