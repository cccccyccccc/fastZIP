use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::ArchiveEntry;

/// List the images contained in a WIM archive.
/// wim-parser only provides metadata (image names, descriptions, counts),
/// not individual file listings within each image.
pub fn list_wim(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let mut parser = wim_parser::WimParser::new(path)
        .with_context(|| format!("Failed to open WIM file: {}", path.display()))?;
    parser
        .parse_full()
        .with_context(|| format!("Failed to parse WIM file: {}", path.display()))?;

    let images = parser.get_images();
    let mut entries = Vec::with_capacity(images.len());

    for image in images {
        let name = format!(
            "[Image {}] {}",
            image.index,
            if image.name.is_empty() {
                "Unnamed"
            } else {
                &image.name
            }
        );
        let desc = if !image.description.is_empty() {
            format!(
                " | {} files, {} dirs, {} MB{}",
                image.file_count,
                image.dir_count,
                image.total_bytes / (1024 * 1024),
                if let Some(ref arch) = image.architecture {
                    format!(", {arch}")
                } else {
                    String::new()
                },
            )
        } else {
            String::new()
        };

        entries.push(ArchiveEntry {
            path: PathBuf::from(format!("{name}{desc}")),
            is_dir: true,
            uncompressed_size: Some(image.total_bytes),
            compressed_size: None,
        });
    }

    Ok(entries)
}

/// Inspect a WIM archive for basic metadata.
#[allow(dead_code)]
pub fn inspect_wim(path: &Path) -> Result<WimInspection> {
    let mut parser = wim_parser::WimParser::new(path)
        .with_context(|| format!("Failed to open WIM file: {}", path.display()))?;
    parser
        .parse_full()
        .with_context(|| format!("Failed to parse WIM file: {}", path.display()))?;

    let images = parser.get_images().to_vec();
    let windows_info = parser.get_windows_info();

    Ok(WimInspection {
        image_count: images.len(),
        images,
        windows_info,
    })
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WimInspection {
    pub image_count: usize,
    pub images: Vec<wim_parser::ImageInfo>,
    pub windows_info: Option<wim_parser::WindowsInfo>,
}

/// WIM extraction is not supported — wim-parser only parses metadata.
/// Full WIM extraction requires LZX/XPRESS decompression and
/// file system traversal which is beyond the current scope.
pub fn extract_wim(
    _path: &Path,
    _options: &super::ExtractOptions,
    _plan: &super::ExtractPathPlan,
    _progress: &mut impl FnMut(u64),
    _should_cancel: &mut impl FnMut() -> bool,
) -> Result<super::ExtractionReport> {
    anyhow::bail!(
        "WIM extraction is not yet supported. Use the 'list' command to view image metadata."
    );
}
