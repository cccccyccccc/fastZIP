use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use lz4_flex::frame::FrameDecoder as Lz4FrameDecoder;
use sevenz_rust2::{ArchiveReader as SevenZReader, Password as SevenZPassword};
use tar::Archive as TarArchive;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::Decoder as ZstdDecoder;

use super::{ArchiveFormat, native::open_archive, rar::RarBackend};

#[derive(Debug, Clone)]
pub struct TestReport {
    pub archive_path: PathBuf,
    pub format: ArchiveFormat,
    pub entries_tested: u64,
    pub entries_failed: u64,
    pub bytes_read: u64,
    pub elapsed: std::time::Duration,
    pub errors: Vec<String>,
}

pub(crate) fn test_archive(
    path: &Path,
    password: Option<&str>,
) -> Result<TestReport> {
    let format = ArchiveFormat::detect(path)?;
    let started = Instant::now();

    match format {
        ArchiveFormat::Rar => test_rar(path, &format, started),
        ArchiveFormat::SevenZip => test_7z(path, password, &format, started),
        ArchiveFormat::Zip => test_zip(path, password, &format, started),
        ArchiveFormat::Tar => test_tar(path, &format, started, || open_archive(path)),
        ArchiveFormat::TarGz => {
            let reader = GzDecoder::new(open_archive(path)?);
            test_tar(path, &format, started, || Ok(reader))
        }
        ArchiveFormat::TarBz2 => {
            let reader = BzDecoder::new(open_archive(path)?);
            test_tar(path, &format, started, || Ok(reader))
        }
        ArchiveFormat::TarXz => {
            let reader = XzDecoder::new(open_archive(path)?);
            test_tar(path, &format, started, || Ok(reader))
        }
        ArchiveFormat::TarZst => {
            let reader = ZstdDecoder::new(open_archive(path)?)?;
            test_tar(path, &format, started, || Ok(reader))
        }
        ArchiveFormat::TarLz4 => {
            let reader = Lz4FrameDecoder::new(open_archive(path)?);
            test_tar(path, &format, started, || Ok(reader))
        }
        ArchiveFormat::Gz => test_stream(path, &format, started, || {
            Ok(GzDecoder::new(open_archive(path)?))
        }),
        ArchiveFormat::Bz2 => test_stream(path, &format, started, || {
            Ok(BzDecoder::new(open_archive(path)?))
        }),
        ArchiveFormat::Xz => test_stream(path, &format, started, || {
            Ok(XzDecoder::new(open_archive(path)?))
        }),
        ArchiveFormat::Zst => test_stream(path, &format, started, || {
            ZstdDecoder::new(open_archive(path)?).map_err(Into::into)
        }),
        ArchiveFormat::Lz4 => test_stream(path, &format, started, || {
            Ok(Lz4FrameDecoder::new(open_archive(path)?))
        }),
        ArchiveFormat::Wim | ArchiveFormat::Iso => {
            bail!("Integrity testing is not supported for WIM/ISO archives")
        }
    }
}

fn test_zip(
    path: &Path,
    password: Option<&str>,
    format: &ArchiveFormat,
    started: Instant,
) -> Result<TestReport> {
    let file =
        File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read ZIP archive {}", path.display()))?;
    let mut report = TestReport::new_empty(path, *format, started);
    let mut buffer = [0u8; 65536];

    for index in 0..archive.len() {
        let password_ref = password.filter(|v| !v.is_empty());
        let mut entry = match password_ref {
            Some(pw) => archive.by_index_decrypt(index, pw.as_bytes()),
            None => archive.by_index(index),
        }
        .map_err(|e| anyhow!("Failed to open entry {}: {e}", index))?;

        if entry.is_dir() {
            continue;
        }

        report.entries_tested += 1;
        match read_all(&mut entry, &mut buffer) {
            Ok(bytes) => report.bytes_read += bytes,
            Err(e) => {
                report.entries_failed += 1;
                report
                    .errors
                    .push(format!("{}: {e:#}", entry.name().to_string()));
            }
        }
    }

    report.elapsed = started.elapsed();
    Ok(report)
}

fn test_7z(
    path: &Path,
    password: Option<&str>,
    format: &ArchiveFormat,
    started: Instant,
) -> Result<TestReport> {
    let sevenz_password = password
        .filter(|v| !v.is_empty())
        .map(SevenZPassword::new)
        .unwrap_or_else(SevenZPassword::empty);

    let mut archive = SevenZReader::open(path, sevenz_password)
        .map_err(|e| anyhow!("Failed to open 7z archive {}: {e}", path.display()))?;
    let mut report = TestReport::new_empty(path, *format, started);
    let mut buffer = [0u8; 65536];

    archive
        .for_each_entries(|entry, reader| {
            if entry.is_directory() {
                return Ok(true);
            }
            report.entries_tested += 1;
            match read_all(reader, &mut buffer) {
                Ok(bytes) => report.bytes_read += bytes,
                Err(e) => {
                    report.entries_failed += 1;
                    report
                        .errors
                        .push(format!("{}: {e:#}", entry.name().to_string()));
                }
            }
            Ok(true)
        })
        .map_err(|e| anyhow!("7z integrity check failed for {}: {e}", path.display()))?;

    report.elapsed = started.elapsed();
    Ok(report)
}

fn test_tar<R: Read>(
    path: &Path,
    format: &ArchiveFormat,
    started: Instant,
    reader_fn: impl FnOnce() -> Result<R>,
) -> Result<TestReport> {
    let reader = reader_fn()?;
    let mut archive = TarArchive::new(reader);
    let mut report = TestReport::new_empty(path, *format, started);
    let mut buffer = [0u8; 65536];

    for entry_result in archive.entries()? {
        let mut entry = entry_result.map_err(|e| anyhow!("Tar entry error: {e}"))?;
        let entry_path = entry.path()?.display().to_string();
        if entry.header().entry_type().is_dir() {
            continue;
        }
        report.entries_tested += 1;
        match read_all(&mut entry, &mut buffer) {
            Ok(bytes) => report.bytes_read += bytes,
            Err(e) => {
                report.entries_failed += 1;
                report
                    .errors
                    .push(format!("{entry_path}: {e:#}"));
            }
        }
    }

    report.elapsed = started.elapsed();
    Ok(report)
}

fn test_stream<R: Read>(
    path: &Path,
    format: &ArchiveFormat,
    started: Instant,
    reader_fn: impl FnOnce() -> Result<R>,
) -> Result<TestReport> {
    let mut reader = reader_fn()?;
    let mut report = TestReport::new_empty(path, *format, started);
    let mut buffer = [0u8; 65536];

    report.entries_tested = 1;
    match read_all(&mut reader, &mut buffer) {
        Ok(bytes) => report.bytes_read = bytes,
        Err(e) => {
            report.entries_failed = 1;
            report.errors.push(format!("{e:#}"));
        }
    }

    report.elapsed = started.elapsed();
    Ok(report)
}

fn test_rar(path: &Path, format: &ArchiveFormat, started: Instant) -> Result<TestReport> {
    let backend = RarBackend::new();
    let executable = match backend.require_executable() {
        Ok(exe) => exe.to_path_buf(),
        Err(e) => bail!("{e:#}"),
    };

    let output = std::process::Command::new(executable)
        .arg("t")
        .arg("-idq")
        .arg(path.as_os_str())
        .output()
        .with_context(|| format!("Failed to invoke RAR adapter for {}", path.display()))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "RAR test failed for {}.\n{}{}",
            path.display(),
            stdout.trim(),
            stderr.trim(),
        );
    }

    Ok(TestReport {
        archive_path: path.to_path_buf(),
        format: *format,
        entries_tested: 0,
        entries_failed: 0,
        bytes_read: 0,
        elapsed: started.elapsed(),
        errors: Vec::new(),
    })
}

fn read_all(reader: &mut (impl Read + ?Sized), buffer: &mut [u8]) -> Result<u64> {
    let mut total: u64 = 0;
    loop {
        let n = reader.read(buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
    }
    Ok(total)
}

impl TestReport {
    fn new_empty(path: &Path, format: ArchiveFormat, started: Instant) -> Self {
        Self {
            archive_path: path.to_path_buf(),
            format,
            entries_tested: 0,
            entries_failed: 0,
            bytes_read: 0,
            elapsed: started.elapsed(),
            errors: Vec::new(),
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.entries_failed == 0 && self.errors.is_empty()
    }
}
