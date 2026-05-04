use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use super::{ArchiveEntry, ExtractOptions, ExtractPathPlan, ExtractionReport};

/// Wrapper to adapt `std::io::Read + Seek` to `iso9660_simple::Read`.
struct IsoReader<R: Read + Seek> {
    inner: R,
}

impl<R: Read + Seek> iso9660_simple::Read for IsoReader<R> {
    fn read(&mut self, position: usize, buffer: &mut [u8]) -> Option<()> {
        self.inner.seek(SeekFrom::Start(position as u64)).ok()?;
        self.inner.read_exact(buffer).ok()
    }
}

#[derive(Clone)]
struct StackFrame {
    path: PathBuf,
    lba: usize,
}

/// Visit one ISO directory, adding entries and returning child stack frames.
fn visit_iso_dir(
    iso: &mut iso9660_simple::ISO9660,
    lba: Option<usize>,
    parent_path: &Path,
    entries: &mut Vec<ArchiveEntry>,
) -> Vec<StackFrame> {
    let dir_iter = match lba {
        Some(pos) => iso.read_directory(pos),
        None => iso.read_root(),
    };

    let mut children = Vec::new();
    for entry in &dir_iter {
        let name = &entry.name;
        if name == "\0" || name == "\x01" || name.is_empty() {
            continue;
        }
        let entry_path = parent_path.join(name);
        if entry.is_folder() {
            entries.push(ArchiveEntry {
                path: entry_path.clone(),
                is_dir: true,
                uncompressed_size: None,
                compressed_size: None,
            });
            children.push(StackFrame {
                path: entry_path,
                lba: entry.lsb_position() as usize,
            });
        } else {
            entries.push(ArchiveEntry {
                path: entry_path,
                is_dir: false,
                uncompressed_size: Some(entry.file_size() as u64),
                compressed_size: None,
            });
        }
    }
    children
}

/// List all files and directories in an ISO 9660 image.
pub fn list_iso(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let file =
        File::open(path).with_context(|| format!("Failed to open ISO file: {}", path.display()))?;
    let reader = IsoReader { inner: file };
    let mut iso = iso9660_simple::ISO9660::from_device(reader)
        .ok_or_else(|| anyhow!("Failed to read ISO 9660 filesystem from {}", path.display()))?;

    let mut entries = Vec::new();
    let mut stack = visit_iso_dir(&mut iso, None, Path::new(""), &mut entries);

    while let Some(frame) = stack.pop() {
        let children = visit_iso_dir(&mut iso, Some(frame.lba), &frame.path, &mut entries);
        stack.extend(children);
    }

    Ok(entries)
}

/// ISO 9660 sector size in bytes.
const SECTOR_SIZE: usize = 2048;

/// Metadata for a file to extract, collected while the directory iterator lives.
struct FileToExtract {
    path: PathBuf,
    lba: usize,
    file_size: usize,
}

/// Extract all files from an ISO 9660 image.
pub fn extract_iso<F>(
    path: &Path,
    options: &ExtractOptions,
    plan: &ExtractPathPlan,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<ExtractionReport>
where
    F: FnMut(u64),
{
    let file =
        File::open(path).with_context(|| format!("Failed to open ISO file: {}", path.display()))?;
    let reader = IsoReader { inner: file };
    let mut iso = iso9660_simple::ISO9660::from_device(reader)
        .ok_or_else(|| anyhow!("Failed to read ISO 9660 filesystem from {}", path.display()))?;

    // Open a second handle for raw file data reads, since iso.read_file()
    // requires an entry reference that conflicts with directory traversal.
    let mut data_reader = File::open(path)
        .with_context(|| format!("Failed to open ISO file for data reads: {}", path.display()))?;

    let mut report = ExtractionReport::new(options.output_dir.clone());
    let mut stack: Vec<StackFrame> = Vec::new();

    // Process root: collect files and child directories
    let mut pending_files: Vec<FileToExtract> = Vec::new();
    {
        let root_iter = iso.read_root();
        for entry in &root_iter {
            if should_cancel() {
                return Err(anyhow!("Extraction cancelled"));
            }
            let name = &entry.name;
            if name == "\0" || name == "\x01" || name.is_empty() {
                continue;
            }
            let entry_path = PathBuf::from(name);
            if plan.should_skip(&entry_path) {
                continue;
            }
            if entry.is_folder() {
                let destination = super::resolve_output_path(&entry_path, options, plan)?;
                super::create_directory(&destination, &mut report)?;
                stack.push(StackFrame {
                    path: entry_path,
                    lba: entry.lsb_position() as usize,
                });
            } else {
                pending_files.push(FileToExtract {
                    path: entry_path,
                    lba: entry.lsb_position() as usize,
                    file_size: entry.file_size() as usize,
                });
            }
        }
    }
    for f in &pending_files {
        extract_one_iso_file(&mut data_reader, f, options, plan, progress, &mut report)?;
    }

    while let Some(frame) = stack.pop() {
        if should_cancel() {
            return Err(anyhow!("Extraction cancelled"));
        }
        let mut pending_files: Vec<FileToExtract> = Vec::new();
        {
            let dir_iter = iso.read_directory(frame.lba);
            for entry in &dir_iter {
                if should_cancel() {
                    return Err(anyhow!("Extraction cancelled"));
                }
                let name = &entry.name;
                if name == "\0" || name == "\x01" || name.is_empty() {
                    continue;
                }
                let entry_path = frame.path.join(name);
                if plan.should_skip(&entry_path) {
                    continue;
                }
                if entry.is_folder() {
                    let destination = super::resolve_output_path(&entry_path, options, plan)?;
                    super::create_directory(&destination, &mut report)?;
                    stack.push(StackFrame {
                        path: entry_path,
                        lba: entry.lsb_position() as usize,
                    });
                } else {
                    pending_files.push(FileToExtract {
                        path: entry_path,
                        lba: entry.lsb_position() as usize,
                        file_size: entry.file_size() as usize,
                    });
                }
            }
        }
        for f in &pending_files {
            extract_one_iso_file(&mut data_reader, f, options, plan, progress, &mut report)?;
        }
    }

    Ok(report)
}

fn extract_one_iso_file<F>(
    reader: &mut File,
    file_info: &FileToExtract,
    options: &ExtractOptions,
    plan: &ExtractPathPlan,
    progress: &mut F,
    report: &mut ExtractionReport,
) -> Result<()>
where
    F: FnMut(u64),
{
    let destination = super::resolve_output_path(&file_info.path, options, plan)?;
    if !super::prepare_output_file(&destination, options.overwrite_mode, report)? {
        return Ok(());
    }
    let byte_offset = file_info.lba * SECTOR_SIZE;
    let mut data = vec![0u8; file_info.file_size];
    reader.seek(SeekFrom::Start(byte_offset as u64))?;
    reader.read_exact(&mut data)?;
    if options.scan_files {
        super::amsi_scan_data(&file_info.path.to_string_lossy(), &data, options)?;
    }
    std::fs::write(&destination, &data)
        .with_context(|| format!("Failed to write extracted file: {}", destination.display()))?;
    report.files_written += 1;
    progress(data.len() as u64);
    Ok(())
}
