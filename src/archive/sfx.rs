use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use super::CompressionFormat;

/// SFX footer: [MAGIC:8][format_tag:u32 LE:4][archive_size:u64 LE:8] = 20 bytes
const SFX_MAGIC: &[u8; 8] = b"FSTZPSFX";
const FOOTER_SIZE: usize = 20;

fn format_tag(format: CompressionFormat) -> u32 {
    match format {
        CompressionFormat::Zip => 0,
        CompressionFormat::SevenZip => 1,
        CompressionFormat::Tar => 2,
        CompressionFormat::TarGz => 3,
        CompressionFormat::TarBz2 => 4,
        CompressionFormat::TarXz => 5,
        CompressionFormat::TarZst => 6,
        CompressionFormat::TarLz4 => 7,
        CompressionFormat::Gz => 8,
        CompressionFormat::Bz2 => 9,
        CompressionFormat::Xz => 10,
        CompressionFormat::Zst => 11,
        CompressionFormat::Lz4 => 12,
    }
}

/// Find the sfx-stub.exe binary. Searches next to the current executable first,
/// then falls back to a compile-time embedded copy.
fn locate_sfx_stub() -> Result<Vec<u8>> {
    // Try runtime lookup next to the current exe
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let stub_path = exe_dir.join("sfx-stub.exe");
            if stub_path.exists() {
                return fs::read(&stub_path)
                    .with_context(|| format!("Failed to read sfx-stub: {}", stub_path.display()));
            }
        }
    }

    // Fall back to the compile-time embedded stub
    embed_sfx_stub()
}

/// Embed the sfx-stub binary at compile time (release builds).
fn embed_sfx_stub() -> Result<Vec<u8>> {
    let candidates = [
        // Release build layout
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/sfx-stub.exe"),
        // Debug build layout (for development)
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/sfx-stub.exe"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return fs::read(candidate).with_context(|| {
                format!("Failed to read embedded sfx-stub: {}", candidate.display())
            });
        }
    }

    Err(anyhow!(
        "sfx-stub.exe not found. Build it first with: cargo build --bin sfx-stub --release"
    ))
}

/// Wrap an existing archive file as an SFX executable.
/// Reads the archive, appends it to the sfx-stub along with a footer,
/// and writes the result to `output_path`.
pub fn wrap_sfx(archive_path: &Path, output_path: &Path, format: CompressionFormat) -> Result<()> {
    let stub_bytes = locate_sfx_stub()?;
    let archive_bytes = fs::read(archive_path)
        .with_context(|| format!("Failed to read archive: {}", archive_path.display()))?;

    let archive_size = archive_bytes.len() as u64;
    let tag = format_tag(format);

    let mut footer = [0u8; FOOTER_SIZE];
    footer[0..8].copy_from_slice(SFX_MAGIC);
    footer[8..12].copy_from_slice(&tag.to_le_bytes());
    footer[12..20].copy_from_slice(&archive_size.to_le_bytes());

    let mut output = fs::File::create(output_path)
        .with_context(|| format!("Failed to create SFX output: {}", output_path.display()))?;

    output.write_all(&stub_bytes)?;
    output.write_all(&archive_bytes)?;
    output.write_all(&footer)?;

    Ok(())
}
