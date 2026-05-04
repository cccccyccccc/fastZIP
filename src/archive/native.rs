use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use lz4_flex::frame::FrameDecoder as Lz4FrameDecoder;
use xz2::read::XzDecoder;
use zstd::stream::Decoder as ZstdDecoder;

use super::{
    ArchiveEntry, ArchiveFormat, BackendKind, BackendStatus, CompressionOptions, CompressionReport,
    ExtractOptions, ExtractPathPlan, ExtractionReport, FilenameEncoding,
    collect_existing_split_volume_paths, create_archive_from_sources, extract_7z,
    extract_single_file, extract_tar, extract_zip, iso, list_7z, list_single_file, list_tar,
    list_zip, split_volume_base_path, wim,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NativeBackend;

struct ResolvedArchiveInput {
    physical_path: PathBuf,
    logical_path: PathBuf,
    temporary: bool,
}

impl ResolvedArchiveInput {
    fn cleanup(&self) {
        if self.temporary {
            let _ = fs::remove_file(&self.physical_path);
        }
    }
}

impl NativeBackend {
    pub(crate) fn status(&self) -> BackendStatus {
        BackendStatus {
            kind: BackendKind::Native,
            label: BackendKind::Native.label().to_string(),
            available: true,
            detail: "Ready. Using built-in Rust codecs.".to_string(),
            formats: vec![
                ".7z".to_string(),
                ".zip".to_string(),
                ".tar".to_string(),
                ".tar.gz".to_string(),
                ".tgz".to_string(),
                ".tar.bz2".to_string(),
                ".tbz2".to_string(),
                ".tar.xz".to_string(),
                ".txz".to_string(),
                ".gz".to_string(),
                ".bz2".to_string(),
                ".bzip2".to_string(),
                ".xz".to_string(),
                ".zst".to_string(),
                ".zstd".to_string(),
                ".tar.zst".to_string(),
                ".tzst".to_string(),
                ".lz4".to_string(),
                ".tar.lz4".to_string(),
                ".tlz4".to_string(),
            ],
        }
    }

    pub(crate) fn list_archive_with_password(
        &self,
        path: &Path,
        format: ArchiveFormat,
        password: Option<&str>,
        encoding: FilenameEncoding,
    ) -> Result<Vec<ArchiveEntry>> {
        let resolved = resolve_archive_input(path)?;
        let result = match format {
            ArchiveFormat::SevenZip => list_7z(&resolved.physical_path, password),
            ArchiveFormat::Zip => list_zip(&resolved.physical_path, password, encoding),
            ArchiveFormat::Tar => list_tar(open_archive(&resolved.physical_path)?),
            ArchiveFormat::TarGz => {
                list_tar(GzDecoder::new(open_archive(&resolved.physical_path)?))
            }
            ArchiveFormat::TarBz2 => {
                list_tar(BzDecoder::new(open_archive(&resolved.physical_path)?))
            }
            ArchiveFormat::TarXz => {
                list_tar(XzDecoder::new(open_archive(&resolved.physical_path)?))
            }
            ArchiveFormat::Gz => list_single_file(&resolved.logical_path, ".gz"),
            ArchiveFormat::Bz2 => list_single_file(&resolved.logical_path, ".bz2"),
            ArchiveFormat::Xz => list_single_file(&resolved.logical_path, ".xz"),
            ArchiveFormat::Zst => list_single_file(&resolved.logical_path, ".zst"),
            ArchiveFormat::Lz4 => list_single_file(&resolved.logical_path, ".lz4"),
            ArchiveFormat::TarZst => {
                list_tar(ZstdDecoder::new(open_archive(&resolved.physical_path)?)?)
            }
            ArchiveFormat::TarLz4 => {
                list_tar(Lz4FrameDecoder::new(open_archive(&resolved.physical_path)?))
            }
            ArchiveFormat::Rar => bail!("RAR archives are handled by the RAR adapter"),
            ArchiveFormat::Wim => wim::list_wim(&resolved.physical_path),
            ArchiveFormat::Iso => iso::list_iso(&resolved.physical_path),
        };
        resolved.cleanup();
        result
    }

    pub(crate) fn extract_archive_with_progress_and_cancel<F, C>(
        &self,
        path: &Path,
        format: ArchiveFormat,
        options: &ExtractOptions,
        plan: &ExtractPathPlan,
        progress: &mut F,
        should_cancel: &mut C,
    ) -> Result<ExtractionReport>
    where
        F: FnMut(u64),
        C: FnMut() -> bool,
    {
        let resolved = resolve_archive_input(path)?;
        let result = match format {
            ArchiveFormat::SevenZip => extract_7z(
                &resolved.physical_path,
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::Zip => extract_zip(
                &resolved.physical_path,
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::Tar => extract_tar(
                open_archive(&resolved.physical_path)?,
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::TarGz => extract_tar(
                GzDecoder::new(open_archive(&resolved.physical_path)?),
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::TarBz2 => extract_tar(
                BzDecoder::new(open_archive(&resolved.physical_path)?),
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::TarXz => extract_tar(
                XzDecoder::new(open_archive(&resolved.physical_path)?),
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::Gz => extract_single_file(
                GzDecoder::new(open_archive(&resolved.physical_path)?),
                &resolved.logical_path,
                ".gz",
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::Bz2 => extract_single_file(
                BzDecoder::new(open_archive(&resolved.physical_path)?),
                &resolved.logical_path,
                ".bz2",
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::Xz => extract_single_file(
                XzDecoder::new(open_archive(&resolved.physical_path)?),
                &resolved.logical_path,
                ".xz",
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::Zst => extract_single_file(
                ZstdDecoder::new(open_archive(&resolved.physical_path)?)?,
                &resolved.logical_path,
                ".zst",
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::Lz4 => extract_single_file(
                Lz4FrameDecoder::new(open_archive(&resolved.physical_path)?),
                &resolved.logical_path,
                ".lz4",
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::TarZst => extract_tar(
                ZstdDecoder::new(open_archive(&resolved.physical_path)?)?,
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::TarLz4 => extract_tar(
                Lz4FrameDecoder::new(open_archive(&resolved.physical_path)?),
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::Rar => bail!("RAR archives are handled by the RAR adapter"),
            ArchiveFormat::Wim => wim::extract_wim(
                &resolved.physical_path,
                options,
                plan,
                progress,
                should_cancel,
            ),
            ArchiveFormat::Iso => iso::extract_iso(
                &resolved.physical_path,
                options,
                plan,
                progress,
                should_cancel,
            ),
        };
        resolved.cleanup();
        result
    }

    pub(crate) fn compress_with_options_and_exclusions_and_progress_and_cancel<F, C>(
        &self,
        sources: &[PathBuf],
        excluded_paths: &[PathBuf],
        output_path: &Path,
        options: CompressionOptions,
        progress: &mut F,
        should_cancel: &mut C,
    ) -> Result<CompressionReport>
    where
        F: FnMut(u64),
        C: FnMut() -> bool,
    {
        create_archive_from_sources(
            sources,
            excluded_paths,
            output_path,
            options,
            progress,
            should_cancel,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn compress_to_zip_with_progress_and_cancel<F, C>(
        &self,
        sources: &[PathBuf],
        excluded_paths: &[PathBuf],
        output_path: &Path,
        progress: &mut F,
        should_cancel: &mut C,
    ) -> Result<CompressionReport>
    where
        F: FnMut(u64),
        C: FnMut() -> bool,
    {
        self.compress_with_options_and_exclusions_and_progress_and_cancel(
            sources,
            excluded_paths,
            output_path,
            CompressionOptions::default(),
            progress,
            should_cancel,
        )
    }
}

pub(crate) fn open_archive(path: &Path) -> Result<File> {
    File::open(path).with_context(|| format!("Failed to open {}", path.display()))
}

fn resolve_archive_input(path: &Path) -> Result<ResolvedArchiveInput> {
    let Some((base_path, part_index, _width)) = split_volume_base_path(path) else {
        return Ok(ResolvedArchiveInput {
            physical_path: path.to_path_buf(),
            logical_path: path.to_path_buf(),
            temporary: false,
        });
    };

    if part_index != 1 {
        bail!(
            "Open the first split volume (*.001) instead of {}",
            path.display()
        );
    }

    let volume_paths = collect_existing_split_volume_paths(&base_path)?;
    if volume_paths.is_empty() {
        bail!("No split archive volumes were found for {}", path.display());
    }

    let temp_path = env::temp_dir().join(format!(
        "fastzip-joined-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = (|| -> Result<()> {
        let temp_file = File::create(&temp_path)
            .with_context(|| format!("Failed to create {}", temp_path.display()))?;
        let mut output = BufWriter::with_capacity(4 * 1024 * 1024, temp_file);

        for (expected_index, volume_path) in volume_paths.iter().enumerate() {
            let expected_part = expected_index as u32 + 1;
            let (_, actual_part, _) = split_volume_base_path(volume_path)
                .ok_or_else(|| anyhow!("Invalid split volume name: {}", volume_path.display()))?;
            if actual_part != expected_part {
                bail!(
                    "Split archive volume set is incomplete. Missing part {:03}.",
                    expected_part
                );
            }

            let mut input = File::open(volume_path)
                .with_context(|| format!("Failed to open {}", volume_path.display()))?;
            std::io::copy(&mut input, &mut output)
                .with_context(|| format!("Failed to read {}", volume_path.display()))?;
        }

        output
            .flush()
            .with_context(|| format!("Failed to flush {}", temp_path.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result?;

    Ok(ResolvedArchiveInput {
        physical_path: temp_path,
        logical_path: base_path,
        temporary: true,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use bzip2::Compression as BzCompression;
    use bzip2::write::BzEncoder;
    use flate2::Compression as GzCompression;
    use flate2::write::GzEncoder;
    use tar::Builder;
    use tempfile::tempdir;
    use xz2::write::XzEncoder;
    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    use super::*;
    use crate::archive::{
        CompressionFormat, CompressionLevel, ExtractOptions, OverwriteMode, ZipCompressionMethod,
    };

    #[test]
    fn detects_common_archive_kinds() {
        assert_eq!(
            ArchiveFormat::detect(Path::new("bundle.7z")).unwrap(),
            ArchiveFormat::SevenZip
        );
        assert_eq!(
            ArchiveFormat::detect(Path::new("bundle.tar.gz")).unwrap(),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            ArchiveFormat::detect(Path::new("bundle.tbz2")).unwrap(),
            ArchiveFormat::TarBz2
        );
        assert_eq!(
            ArchiveFormat::detect(Path::new("bundle.txz")).unwrap(),
            ArchiveFormat::TarXz
        );
        assert_eq!(
            ArchiveFormat::detect(Path::new("bundle.zip")).unwrap(),
            ArchiveFormat::Zip
        );
        assert_eq!(
            ArchiveFormat::detect(Path::new("bundle.zip.001")).unwrap(),
            ArchiveFormat::Zip
        );
        assert_eq!(
            ArchiveFormat::detect(Path::new("bundle.7z.001")).unwrap(),
            ArchiveFormat::SevenZip
        );
        assert_eq!(
            ArchiveFormat::detect(Path::new("bundle.gz")).unwrap(),
            ArchiveFormat::Gz
        );
    }

    #[test]
    fn lists_and_extracts_zip_archives() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let archive_path = temp.path().join("sample.zip");
        let source_name = "folder/hello.txt";
        let source_bytes = b"hello from zip";

        create_zip_archive(&archive_path, source_name, source_bytes);

        let entries = backend
            .list_archive_with_password(
                &archive_path,
                ArchiveFormat::Zip,
                None,
                FilenameEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, PathBuf::from(source_name));

        let output_dir = temp.path().join("zip-out");
        let report = backend
            .extract_archive_with_progress_and_cancel(
                &archive_path,
                ArchiveFormat::Zip,
                &ExtractOptions {
                    output_dir: output_dir.clone(),
                    overwrite_mode: OverwriteMode::Overwrite,
                    keep_paths: true,
                    password: None,
                    filename_encoding: FilenameEncoding::Utf8,
                    scan_files: false,
                },
                &ExtractPathPlan::default(),
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();
        assert_eq!(report.files_written, 1);
        assert_eq!(
            fs::read(output_dir.join(source_name)).unwrap(),
            source_bytes
        );
    }

    #[test]
    fn compresses_files_and_directories_into_zip_archives() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let folder = temp.path().join("project");
        fs::create_dir_all(folder.join("nested")).unwrap();
        fs::write(folder.join("nested").join("hello.txt"), b"hello zip").unwrap();
        fs::write(temp.path().join("readme.md"), b"top level").unwrap();

        let archive_path = temp.path().join("bundle.zip");
        let report = backend
            .compress_with_options_and_exclusions_and_progress_and_cancel(
                &[folder.clone(), temp.path().join("readme.md")],
                &[],
                &archive_path,
                CompressionOptions::default(),
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();

        assert_eq!(report.files_added, 2);

        let entries = backend
            .list_archive_with_password(
                &archive_path,
                ArchiveFormat::Zip,
                None,
                FilenameEncoding::Utf8,
            )
            .unwrap();
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("project/nested/hello.txt"))
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("readme.md"))
        );
    }

    #[test]
    fn compresses_zip_archives_with_password_and_requires_it_for_listing_and_extracting() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let folder = temp.path().join("secret");
        let password = "zip-secret";
        fs::create_dir_all(&folder).unwrap();
        for index in 0..4 {
            let name = format!("note-{index:02}.txt");
            let payload = vec![b'A' + index as u8; 384 * 1024];
            fs::write(folder.join(name), payload).unwrap();
        }

        let archive_path = temp.path().join("secret.zip");
        let report = backend
            .compress_with_options_and_exclusions_and_progress_and_cancel(
                std::slice::from_ref(&folder),
                &[],
                &archive_path,
                CompressionOptions {
                    format: CompressionFormat::Zip,
                    level: CompressionLevel::Normal,
                    zip_method: ZipCompressionMethod::Deflate,
                    thread_count: 4,
                    password: Some(password.to_string()),
                    encrypt_file_names: false,
                    ..CompressionOptions::default()
                },
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();
        assert_eq!(report.files_added, 4);

        let missing_password = backend.list_archive_with_password(
            &archive_path,
            ArchiveFormat::Zip,
            None,
            FilenameEncoding::Utf8,
        );
        assert!(missing_password.is_err());

        let wrong_password = backend.list_archive_with_password(
            &archive_path,
            ArchiveFormat::Zip,
            Some("wrong"),
            FilenameEncoding::Utf8,
        );
        assert!(wrong_password.is_err());

        let entries = backend
            .list_archive_with_password(
                &archive_path,
                ArchiveFormat::Zip,
                Some(password),
                FilenameEncoding::Utf8,
            )
            .unwrap();
        for index in 0..4 {
            assert!(entries
                .iter()
                .any(|entry| entry.path == PathBuf::from(format!("secret/note-{index:02}.txt"))));
        }

        let extract_without_password = backend.extract_archive_with_progress_and_cancel(
            &archive_path,
            ArchiveFormat::Zip,
            &ExtractOptions {
                output_dir: temp.path().join("zip-no-password"),
                overwrite_mode: OverwriteMode::Overwrite,
                keep_paths: true,
                password: None,
                filename_encoding: FilenameEncoding::Utf8,
                scan_files: false,
            },
            &ExtractPathPlan::default(),
            &mut |_delta| {},
            &mut || false,
        );
        assert!(extract_without_password.is_err());

        let output_dir = temp.path().join("zip-secret-out");
        let extraction = backend
            .extract_archive_with_progress_and_cancel(
                &archive_path,
                ArchiveFormat::Zip,
                &ExtractOptions {
                    output_dir: output_dir.clone(),
                    overwrite_mode: OverwriteMode::Overwrite,
                    keep_paths: true,
                    password: Some(password.to_string()),
                    filename_encoding: FilenameEncoding::Utf8,
                    scan_files: false,
                },
                &ExtractPathPlan::default(),
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();
        assert_eq!(extraction.files_written, 4);
        for index in 0..4 {
            assert_eq!(
                fs::read(
                    output_dir
                        .join("secret")
                        .join(format!("note-{index:02}.txt"))
                )
                .unwrap(),
                vec![b'A' + index as u8; 384 * 1024]
            );
        }
    }

    #[test]
    fn compresses_and_extracts_7z_archives_with_native_backend() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let folder = temp.path().join("project");
        fs::create_dir_all(folder.join("nested")).unwrap();
        fs::write(folder.join("keep.txt"), b"keep sevenzip").unwrap();
        fs::write(folder.join("photo.jpg"), b"fake-jpeg-payload").unwrap();
        fs::write(folder.join("nested").join("skip.txt"), b"skip sevenzip").unwrap();

        let archive_path = temp.path().join("bundle.7z");
        let report = backend
            .compress_with_options_and_exclusions_and_progress_and_cancel(
                std::slice::from_ref(&folder),
                &[folder.join("nested").join("skip.txt")],
                &archive_path,
                CompressionOptions {
                    format: CompressionFormat::SevenZip,
                    level: CompressionLevel::Maximum,
                    zip_method: ZipCompressionMethod::Deflate,
                    thread_count: 2,
                    ..CompressionOptions::default()
                },
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();

        assert_eq!(report.files_added, 2);
        let entries = backend
            .list_archive_with_password(
                &archive_path,
                ArchiveFormat::SevenZip,
                None,
                FilenameEncoding::Utf8,
            )
            .unwrap();
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("project/keep.txt"))
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("project/photo.jpg"))
        );
        assert!(
            !entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("project/nested/skip.txt"))
        );

        let output_dir = temp.path().join("sevenzip-out");
        let extraction = backend
            .extract_archive_with_progress_and_cancel(
                &archive_path,
                ArchiveFormat::SevenZip,
                &ExtractOptions {
                    output_dir: output_dir.clone(),
                    overwrite_mode: OverwriteMode::Overwrite,
                    keep_paths: true,
                    password: None,
                    filename_encoding: FilenameEncoding::Utf8,
                    scan_files: false,
                },
                &ExtractPathPlan::default(),
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();
        assert_eq!(extraction.files_written, 2);
        assert_eq!(
            fs::read(output_dir.join("project").join("keep.txt")).unwrap(),
            b"keep sevenzip"
        );
        assert_eq!(
            fs::read(output_dir.join("project").join("photo.jpg")).unwrap(),
            b"fake-jpeg-payload"
        );
    }

    #[test]
    fn compresses_split_zip_archives_and_reads_them_from_the_first_volume() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let folder = temp.path().join("split-project");
        fs::create_dir_all(&folder).unwrap();

        for index in 0..3 {
            let payload = (0..(96 * 1024))
                .map(|offset| ((offset + index * 31) % 251) as u8)
                .collect::<Vec<_>>();
            fs::write(folder.join(format!("chunk-{index:02}.bin")), payload).unwrap();
        }

        let archive_path = temp.path().join("bundle.zip");
        let report = backend
            .compress_with_options_and_exclusions_and_progress_and_cancel(
                std::slice::from_ref(&folder),
                &[],
                &archive_path,
                CompressionOptions {
                    format: CompressionFormat::Zip,
                    level: CompressionLevel::Fastest,
                    zip_method: ZipCompressionMethod::Stored,
                    thread_count: 2,
                    split_volume_size: Some(64 * 1024),
                    ..CompressionOptions::default()
                },
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();

        let first_volume = temp.path().join("bundle.zip.001");
        let second_volume = temp.path().join("bundle.zip.002");
        assert_eq!(report.archive_path, first_volume);
        assert!(!archive_path.exists());
        assert!(first_volume.exists());
        assert!(second_volume.exists());

        let entries = backend
            .list_archive_with_password(
                &first_volume,
                ArchiveFormat::Zip,
                None,
                FilenameEncoding::Utf8,
            )
            .unwrap();
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("split-project/chunk-00.bin"))
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("split-project/chunk-01.bin"))
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("split-project/chunk-02.bin"))
        );

        let output_dir = temp.path().join("split-zip-out");
        let extraction = backend
            .extract_archive_with_progress_and_cancel(
                &first_volume,
                ArchiveFormat::Zip,
                &ExtractOptions {
                    output_dir: output_dir.clone(),
                    overwrite_mode: OverwriteMode::Overwrite,
                    keep_paths: true,
                    password: None,
                    filename_encoding: FilenameEncoding::Utf8,
                    scan_files: false,
                },
                &ExtractPathPlan::default(),
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();
        assert_eq!(extraction.files_written, 3);

        for index in 0..3 {
            let expected = (0..(96 * 1024))
                .map(|offset| ((offset + index * 31) % 251) as u8)
                .collect::<Vec<_>>();
            assert_eq!(
                fs::read(
                    output_dir
                        .join("split-project")
                        .join(format!("chunk-{index:02}.bin"))
                )
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn compresses_split_7z_archives_and_reads_them_from_the_first_volume() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let folder = temp.path().join("split-vault");
        fs::create_dir_all(folder.join("nested")).unwrap();
        fs::write(folder.join("alpha.txt"), b"alpha split sevenzip payload").unwrap();
        let beta_payload = (0..(160 * 1024))
            .scan(0x1234_5678u32, |state, _| {
                *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                Some((*state >> 16) as u8)
            })
            .collect::<Vec<_>>();
        fs::write(folder.join("nested").join("beta.bin"), &beta_payload).unwrap();

        let archive_path = temp.path().join("vault.7z");
        let report = backend
            .compress_with_options_and_exclusions_and_progress_and_cancel(
                std::slice::from_ref(&folder),
                &[],
                &archive_path,
                CompressionOptions {
                    format: CompressionFormat::SevenZip,
                    level: CompressionLevel::Fast,
                    zip_method: ZipCompressionMethod::Deflate,
                    thread_count: 2,
                    split_volume_size: Some(48 * 1024),
                    ..CompressionOptions::default()
                },
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();

        let first_volume = temp.path().join("vault.7z.001");
        assert_eq!(report.archive_path, first_volume);
        assert!(!archive_path.exists());
        assert!(first_volume.exists());
        assert!(temp.path().join("vault.7z.002").exists());

        let entries = backend
            .list_archive_with_password(
                &first_volume,
                ArchiveFormat::SevenZip,
                None,
                FilenameEncoding::Utf8,
            )
            .unwrap();
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("split-vault/alpha.txt"))
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("split-vault/nested/beta.bin"))
        );

        let output_dir = temp.path().join("split-7z-out");
        let extraction = backend
            .extract_archive_with_progress_and_cancel(
                &first_volume,
                ArchiveFormat::SevenZip,
                &ExtractOptions {
                    output_dir: output_dir.clone(),
                    overwrite_mode: OverwriteMode::Overwrite,
                    keep_paths: true,
                    password: None,
                    filename_encoding: FilenameEncoding::Utf8,
                    scan_files: false,
                },
                &ExtractPathPlan::default(),
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();
        assert_eq!(extraction.files_written, 2);
        assert_eq!(
            fs::read(output_dir.join("split-vault").join("alpha.txt")).unwrap(),
            b"alpha split sevenzip payload"
        );
        assert_eq!(
            fs::read(
                output_dir
                    .join("split-vault")
                    .join("nested")
                    .join("beta.bin")
            )
            .unwrap(),
            beta_payload
        );
    }

    #[test]
    fn compresses_7z_archives_with_password_and_encrypted_headers() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let folder = temp.path().join("vault");
        let password = "7z-secret";
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("hidden.txt"), b"hidden sevenzip payload").unwrap();

        let archive_path = temp.path().join("vault.7z");
        let report = backend
            .compress_with_options_and_exclusions_and_progress_and_cancel(
                std::slice::from_ref(&folder),
                &[],
                &archive_path,
                CompressionOptions {
                    format: CompressionFormat::SevenZip,
                    level: CompressionLevel::Maximum,
                    zip_method: ZipCompressionMethod::Deflate,
                    thread_count: 2,
                    password: Some(password.to_string()),
                    encrypt_file_names: true,
                    ..CompressionOptions::default()
                },
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();
        assert_eq!(report.files_added, 1);

        assert!(
            backend
                .list_archive_with_password(
                    &archive_path,
                    ArchiveFormat::SevenZip,
                    None,
                    FilenameEncoding::Utf8
                )
                .is_err()
        );
        assert!(
            backend
                .list_archive_with_password(
                    &archive_path,
                    ArchiveFormat::SevenZip,
                    Some("wrong"),
                    FilenameEncoding::Utf8
                )
                .is_err()
        );

        let entries = backend
            .list_archive_with_password(
                &archive_path,
                ArchiveFormat::SevenZip,
                Some(password),
                FilenameEncoding::Utf8,
            )
            .unwrap();
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == PathBuf::from("vault/hidden.txt"))
        );

        assert!(
            backend
                .extract_archive_with_progress_and_cancel(
                    &archive_path,
                    ArchiveFormat::SevenZip,
                    &ExtractOptions {
                        output_dir: temp.path().join("sevenzip-no-password"),
                        overwrite_mode: OverwriteMode::Overwrite,
                        keep_paths: true,
                        password: None,
                        filename_encoding: FilenameEncoding::Utf8,
                        scan_files: false,
                    },
                    &ExtractPathPlan::default(),
                    &mut |_delta| {},
                    &mut || false,
                )
                .is_err()
        );

        let output_dir = temp.path().join("sevenzip-secret-out");
        let extraction = backend
            .extract_archive_with_progress_and_cancel(
                &archive_path,
                ArchiveFormat::SevenZip,
                &ExtractOptions {
                    output_dir: output_dir.clone(),
                    overwrite_mode: OverwriteMode::Overwrite,
                    keep_paths: true,
                    password: Some(password.to_string()),
                    filename_encoding: FilenameEncoding::Utf8,
                    scan_files: false,
                },
                &ExtractPathPlan::default(),
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();
        assert_eq!(extraction.files_written, 1);
        assert_eq!(
            fs::read(output_dir.join("vault").join("hidden.txt")).unwrap(),
            b"hidden sevenzip payload"
        );
    }

    #[test]
    fn compresses_single_files_into_native_stream_archives() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("notes.txt");
        fs::write(&source_path, b"native stream payload").unwrap();

        for (archive_name, compression_format, archive_format, thread_count) in [
            ("notes.txt.gz", CompressionFormat::Gz, ArchiveFormat::Gz, 1),
            (
                "notes.txt.bz2",
                CompressionFormat::Bz2,
                ArchiveFormat::Bz2,
                1,
            ),
            ("notes.txt.xz", CompressionFormat::Xz, ArchiveFormat::Xz, 2),
        ] {
            let archive_path = temp.path().join(archive_name);
            let report = backend
                .compress_with_options_and_exclusions_and_progress_and_cancel(
                    std::slice::from_ref(&source_path),
                    &[],
                    &archive_path,
                    CompressionOptions {
                        format: compression_format,
                        level: CompressionLevel::Maximum,
                        zip_method: ZipCompressionMethod::Deflate,
                        thread_count,
                        ..CompressionOptions::default()
                    },
                    &mut |_delta| {},
                    &mut || false,
                )
                .unwrap();

            assert_eq!(report.files_added, 1);
            let output_dir = temp
                .path()
                .join(format!("extract-{}", archive_name.replace('.', "-")));
            let extraction = backend
                .extract_archive_with_progress_and_cancel(
                    &archive_path,
                    archive_format,
                    &ExtractOptions {
                        output_dir: output_dir.clone(),
                        overwrite_mode: OverwriteMode::Overwrite,
                        keep_paths: true,
                        password: None,
                        filename_encoding: FilenameEncoding::Utf8,
                        scan_files: false,
                    },
                    &ExtractPathPlan::default(),
                    &mut |_delta| {},
                    &mut || false,
                )
                .unwrap();
            assert_eq!(extraction.files_written, 1);
            assert_eq!(
                fs::read(output_dir.join("notes.txt")).unwrap(),
                b"native stream payload"
            );
        }
    }

    #[test]
    fn extracts_tar_gz_archives() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let archive_path = temp.path().join("sample.tar.gz");
        let source_name = "nested/readme.txt";
        let source_bytes = b"hello from tar.gz";

        create_tar_gz_archive(&archive_path, source_name, source_bytes);

        let entries = backend
            .list_archive_with_password(
                &archive_path,
                ArchiveFormat::TarGz,
                None,
                FilenameEncoding::Utf8,
            )
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, PathBuf::from(source_name));

        let output_dir = temp.path().join("tar-out");
        backend
            .extract_archive_with_progress_and_cancel(
                &archive_path,
                ArchiveFormat::TarGz,
                &ExtractOptions {
                    output_dir: output_dir.clone(),
                    overwrite_mode: OverwriteMode::Overwrite,
                    keep_paths: true,
                    password: None,
                    filename_encoding: FilenameEncoding::Utf8,
                    scan_files: false,
                },
                &ExtractPathPlan::default(),
                &mut |_delta| {},
                &mut || false,
            )
            .unwrap();

        assert_eq!(
            fs::read(output_dir.join(source_name)).unwrap(),
            source_bytes
        );
    }

    #[test]
    fn extracts_gz_bz2_and_xz_single_files() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let source_name = "notes.txt";
        let source_bytes = b"single file payload";

        let gz_path = temp.path().join("notes.txt.gz");
        let bz2_path = temp.path().join("notes.txt.bz2");
        let xz_path = temp.path().join("notes.txt.xz");

        create_gz_file(&gz_path, source_bytes);
        create_bz2_file(&bz2_path, source_bytes);
        create_xz_file(&xz_path, source_bytes);

        for (archive_path, format) in [
            (&gz_path, ArchiveFormat::Gz),
            (&bz2_path, ArchiveFormat::Bz2),
            (&xz_path, ArchiveFormat::Xz),
        ] {
            let output_dir = temp.path().join(
                archive_path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
            );
            let report = backend
                .extract_archive_with_progress_and_cancel(
                    archive_path,
                    format,
                    &ExtractOptions {
                        output_dir: output_dir.clone(),
                        overwrite_mode: OverwriteMode::Overwrite,
                        keep_paths: true,
                        password: None,
                        filename_encoding: FilenameEncoding::Utf8,
                        scan_files: false,
                    },
                    &ExtractPathPlan::default(),
                    &mut |_delta| {},
                    &mut || false,
                )
                .unwrap();
            assert_eq!(report.files_written, 1);
            assert_eq!(
                fs::read(output_dir.join(source_name)).unwrap(),
                source_bytes
            );
        }
    }

    #[test]
    fn canceled_compression_removes_partial_archive_and_preserves_existing_output() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("payload.bin");
        fs::write(&source_path, vec![42u8; 512 * 1024]).unwrap();

        let archive_path = temp.path().join("bundle.zip");
        let existing_bytes = b"existing archive bytes".to_vec();
        fs::write(&archive_path, &existing_bytes).unwrap();

        let processed = std::cell::Cell::new(0u64);
        let result = backend.compress_to_zip_with_progress_and_cancel(
            &[source_path],
            &[],
            &archive_path,
            &mut |delta| processed.set(processed.get().saturating_add(delta)),
            &mut || processed.get() >= 64 * 1024,
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&archive_path).unwrap(), existing_bytes);
        assert!(
            fs::read_dir(temp.path())
                .unwrap()
                .filter_map(|entry| entry.ok())
                .all(|entry| {
                    !entry
                        .file_name()
                        .to_string_lossy()
                        .contains(".fastzip-part-")
                })
        );
    }

    #[test]
    fn canceled_extraction_preserves_existing_destination_file() {
        let backend = NativeBackend;
        let temp = tempdir().unwrap();
        let archive_path = temp.path().join("sample.zip");
        let entry_name = "folder/hello.txt";
        create_zip_archive(&archive_path, entry_name, &vec![7u8; 512 * 1024]);

        let output_dir = temp.path().join("zip-out");
        fs::create_dir_all(output_dir.join("folder")).unwrap();
        let destination = output_dir.join(entry_name);
        let existing_bytes = b"keep me".to_vec();
        fs::write(&destination, &existing_bytes).unwrap();

        let processed = std::cell::Cell::new(0u64);
        let plan = ExtractPathPlan::default();
        let result = backend.extract_archive_with_progress_and_cancel(
            &archive_path,
            ArchiveFormat::Zip,
            &ExtractOptions {
                output_dir: output_dir.clone(),
                overwrite_mode: OverwriteMode::Overwrite,
                keep_paths: true,
                password: None,
                filename_encoding: FilenameEncoding::Utf8,
                scan_files: false,
            },
            &plan,
            &mut |delta| processed.set(processed.get().saturating_add(delta)),
            &mut || processed.get() >= 64 * 1024,
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&destination).unwrap(), existing_bytes);
        assert!(
            fs::read_dir(destination.parent().unwrap())
                .unwrap()
                .filter_map(|entry| entry.ok())
                .all(|entry| {
                    !entry
                        .file_name()
                        .to_string_lossy()
                        .contains(".fastzip-part-")
                })
        );
    }

    fn create_zip_archive(path: &Path, entry_name: &str, bytes: &[u8]) {
        let file = File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file(entry_name, options).unwrap();
        writer.write_all(bytes).unwrap();
        writer.finish().unwrap();
    }

    fn create_tar_gz_archive(path: &Path, entry_name: &str, bytes: &[u8]) {
        let file = File::create(path).unwrap();
        let encoder = GzEncoder::new(file, GzCompression::default());
        let mut builder = Builder::new(encoder);

        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, entry_name, bytes).unwrap();
        builder.finish().unwrap();
    }

    fn create_gz_file(path: &Path, bytes: &[u8]) {
        let file = File::create(path).unwrap();
        let mut encoder = GzEncoder::new(file, GzCompression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap();
    }

    fn create_bz2_file(path: &Path, bytes: &[u8]) {
        let file = File::create(path).unwrap();
        let mut encoder = BzEncoder::new(file, BzCompression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap();
    }

    fn create_xz_file(path: &Path, bytes: &[u8]) {
        let file = File::create(path).unwrap();
        let mut encoder = XzEncoder::new(file, 6);
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap();
    }
}
