mod iso;
mod native;
mod rar;
mod service;
mod sfx;
pub mod test;
mod wim;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};

use crate::encoding::{FilenameEncoding, decode_zip_filename};
use bzip2::Compression as BzCompression;
use bzip2::write::BzEncoder;
use crc32fast::Hasher as Crc32Hasher;
use flate2::Compress as RawDeflateCompressor;
use flate2::Compression as GzCompression;
use flate2::FlushCompress;
use flate2::Status as FlateStatus;
use flate2::write::GzEncoder;
use libdeflater::CompressionLvl as WholeBufferDeflateLevel;
use libdeflater::Compressor as WholeBufferDeflateCompressor;
use sevenz_rust2::encoder_options::{
    AesEncoderOptions as SevenZAesEncoderOptions, Lzma2Options as SevenZLzma2Options,
};
use sevenz_rust2::{
    Archive as SevenZArchive, ArchiveEntry as SevenZArchiveEntry, ArchiveReader as SevenZReader,
    ArchiveWriter as SevenZWriter, EncoderConfiguration as SevenZEncoderConfiguration,
    EncoderMethod as SevenZEncoderMethod, Password as SevenZPassword,
    SourceReader as SevenZSourceReader,
};
use tar::Archive as TarArchive;
use tar::{Builder as TarBuilder, Header as TarHeader, HeaderMode as TarHeaderMode};
use xz2::stream::{Check as XzCheck, MtStreamBuilder};
use xz2::write::XzEncoder;
use zip::AesMode as ZipAesMode;
use zip::CompressionMethod;
use zip::DateTime;
use zip::ZipArchive;
use zip::write::{FileOptions as ZipFileOptions, SimpleFileOptions};
use zstd::stream::Encoder as ZstdEncoder;

pub use service::{ArchiveInspection, ArchiveService, BackendStatus};
pub use test::TestReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    SevenZip,
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    Gz,
    Bz2,
    Xz,
    Zst,
    TarZst,
    Lz4,
    TarLz4,
    Rar,
    Wim,
    Iso,
}

impl ArchiveFormat {
    pub fn detect(path: &Path) -> Result<Self> {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .ok_or_else(|| anyhow!("Archive path is missing a valid file name"))?;
        let name = split_volume_base_name(&name)
            .map(|(base_name, _, _)| base_name.to_string())
            .unwrap_or(name);

        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            return Ok(Self::TarGz);
        }
        if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") {
            return Ok(Self::TarBz2);
        }
        if name.ends_with(".tar.xz") || name.ends_with(".txz") {
            return Ok(Self::TarXz);
        }
        if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
            return Ok(Self::TarZst);
        }
        if name.ends_with(".tar.lz4") || name.ends_with(".tlz4") {
            return Ok(Self::TarLz4);
        }
        if name.ends_with(".7z") {
            return Ok(Self::SevenZip);
        }
        if name.ends_with(".zip") {
            return Ok(Self::Zip);
        }
        if name.ends_with(".tar") {
            return Ok(Self::Tar);
        }
        if name.ends_with(".rar") {
            return Ok(Self::Rar);
        }
        if name.ends_with(".wim") {
            return Ok(Self::Wim);
        }
        if name.ends_with(".iso") {
            return Ok(Self::Iso);
        }
        if name.ends_with(".zst") || name.ends_with(".zstd") {
            return Ok(Self::Zst);
        }
        if name.ends_with(".lz4") {
            return Ok(Self::Lz4);
        }
        if name.ends_with(".gz") {
            return Ok(Self::Gz);
        }
        if name.ends_with(".bz2") {
            return Ok(Self::Bz2);
        }
        if name.ends_with(".xz") {
            return Ok(Self::Xz);
        }

        bail!(
            "Unsupported archive format for {}. Supported formats: {}",
            path.display(),
            all_supported_formats().join(", ")
        )
    }

    pub fn default_extension(self) -> &'static str {
        match self {
            Self::SevenZip => ".7z",
            Self::Zip => ".zip",
            Self::Tar => ".tar",
            Self::TarGz => ".tar.gz",
            Self::TarBz2 => ".tar.bz2",
            Self::TarXz => ".tar.xz",
            Self::Gz => ".gz",
            Self::Bz2 => ".bz2",
            Self::Xz => ".xz",
            Self::Zst => ".zst",
            Self::TarZst => ".tar.zst",
            Self::Lz4 => ".lz4",
            Self::TarLz4 => ".tar.lz4",
            Self::Rar => ".rar",
            Self::Wim => ".wim",
            Self::Iso => ".iso",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SevenZip => "7Z",
            Self::Zip => "ZIP",
            Self::Tar => "TAR",
            Self::TarGz => "TAR.GZ",
            Self::TarBz2 => "TAR.BZ2",
            Self::TarXz => "TAR.XZ",
            Self::Gz => "GZ",
            Self::Bz2 => "BZ2",
            Self::Xz => "XZ",
            Self::Zst => "ZST",
            Self::TarZst => "TAR.ZST",
            Self::Lz4 => "LZ4",
            Self::TarLz4 => "TAR.LZ4",
            Self::Rar => "RAR",
            Self::Wim => "WIM",
            Self::Iso => "ISO",
        }
    }
}

impl fmt::Display for ArchiveFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Native,
    RarAdapter,
}

impl BackendKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "Native Rust core",
            Self::RarAdapter => "RAR adapter",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub uncompressed_size: Option<u64>,
    pub compressed_size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwriteMode {
    Overwrite,
    Skip,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtractPathPlan {
    pub skipped_paths: BTreeSet<PathBuf>,
    pub renamed_paths: BTreeMap<PathBuf, PathBuf>,
}

impl ExtractPathPlan {
    pub fn is_empty(&self) -> bool {
        self.skipped_paths.is_empty() && self.renamed_paths.is_empty()
    }

    pub fn should_skip(&self, relative: &Path) -> bool {
        self.skipped_paths.contains(relative)
    }

    pub fn renamed_path(&self, relative: &Path) -> Option<&PathBuf> {
        self.renamed_paths.get(relative)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractOptions {
    pub output_dir: PathBuf,
    pub overwrite_mode: OverwriteMode,
    pub keep_paths: bool,
    pub password: Option<String>,
    pub filename_encoding: FilenameEncoding,
    pub scan_files: bool,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("."),
            overwrite_mode: OverwriteMode::Overwrite,
            keep_paths: true,
            password: None,
            filename_encoding: FilenameEncoding::Utf8,
            scan_files: false,
        }
    }
}

impl ExtractOptions {
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref().filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionReport {
    pub output_dir: PathBuf,
    pub files_written: usize,
    pub directories_created: usize,
}

impl ExtractionReport {
    pub(crate) fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            files_written: 0,
            directories_created: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionReport {
    pub archive_path: PathBuf,
    pub files_added: usize,
    pub directories_added: usize,
    pub input_bytes: u64,
    pub output_bytes: u64,
}

impl CompressionReport {
    pub(crate) fn new(archive_path: PathBuf) -> Self {
        Self {
            archive_path,
            files_added: 0,
            directories_added: 0,
            input_bytes: 0,
            output_bytes: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionFormat {
    SevenZip,
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    Gz,
    Bz2,
    Xz,
    Zst,
    TarZst,
    Lz4,
    TarLz4,
}

impl CompressionFormat {
    pub fn default_extension(self) -> &'static str {
        match self {
            Self::SevenZip => ".7z",
            Self::Zip => ".zip",
            Self::Tar => ".tar",
            Self::TarGz => ".tar.gz",
            Self::TarBz2 => ".tar.bz2",
            Self::TarXz => ".tar.xz",
            Self::Gz => ".gz",
            Self::Bz2 => ".bz2",
            Self::Xz => ".xz",
            Self::Zst => ".zst",
            Self::TarZst => ".tar.zst",
            Self::Lz4 => ".lz4",
            Self::TarLz4 => ".tar.lz4",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SevenZip => "7Z",
            Self::Zip => "ZIP",
            Self::Tar => "TAR",
            Self::TarGz => "TAR.GZ",
            Self::TarBz2 => "TAR.BZ2",
            Self::TarXz => "TAR.XZ",
            Self::Gz => "GZ",
            Self::Bz2 => "BZ2",
            Self::Xz => "XZ",
            Self::Zst => "ZST",
            Self::TarZst => "TAR.ZST",
            Self::Lz4 => "LZ4",
            Self::TarLz4 => "TAR.LZ4",
        }
    }

    pub fn primary_suffix(self) -> &'static str {
        match self {
            Self::SevenZip => ".7z",
            Self::Zip => ".zip",
            Self::Tar => ".tar",
            Self::TarGz => ".tar.gz",
            Self::TarBz2 => ".tar.bz2",
            Self::TarXz => ".tar.xz",
            Self::Gz => ".gz",
            Self::Bz2 => ".bz2",
            Self::Xz => ".xz",
            Self::Zst => ".zst",
            Self::TarZst => ".tar.zst",
            Self::Lz4 => ".lz4",
            Self::TarLz4 => ".tar.lz4",
        }
    }

    pub fn matches_file_name(self, file_name: &str) -> bool {
        let lower = file_name.to_ascii_lowercase();
        self.suffixes().iter().any(|suffix| lower.ends_with(suffix))
    }

    pub fn suffixes(self) -> &'static [&'static str] {
        match self {
            Self::SevenZip => &[".7z"],
            Self::Zip => &[".zip"],
            Self::Tar => &[".tar"],
            Self::TarGz => &[".tar.gz", ".tgz"],
            Self::TarBz2 => &[".tar.bz2", ".tbz2"],
            Self::TarXz => &[".tar.xz", ".txz"],
            Self::Gz => &[".gz"],
            Self::Bz2 => &[".bz2", ".bzip2"],
            Self::Xz => &[".xz"],
            Self::Zst => &[".zst", ".zstd"],
            Self::TarZst => &[".tar.zst", ".tzst"],
            Self::Lz4 => &[".lz4"],
            Self::TarLz4 => &[".tar.lz4", ".tlz4"],
        }
    }

    pub fn supports_zip_method(self) -> bool {
        matches!(self, Self::Zip)
    }

    pub fn supports_thread_count(self) -> bool {
        matches!(
            self,
            Self::SevenZip | Self::Zip | Self::TarXz | Self::Xz | Self::Zst | Self::TarZst
        )
    }

    pub fn supports_password_encryption(self) -> bool {
        matches!(self, Self::SevenZip | Self::Zip)
    }

    pub fn supports_encrypt_file_names(self) -> bool {
        matches!(self, Self::SevenZip)
    }

    pub fn is_single_file_stream(self) -> bool {
        matches!(
            self,
            Self::Gz | Self::Bz2 | Self::Xz | Self::Zst | Self::Lz4
        )
    }

    pub fn detect(path: &Path) -> Result<Self> {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .ok_or_else(|| anyhow!("Output path is missing a valid file name"))?;

        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            return Ok(Self::TarGz);
        }
        if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") {
            return Ok(Self::TarBz2);
        }
        if name.ends_with(".tar.xz") || name.ends_with(".txz") {
            return Ok(Self::TarXz);
        }
        if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
            return Ok(Self::TarZst);
        }
        if name.ends_with(".tar.lz4") || name.ends_with(".tlz4") {
            return Ok(Self::TarLz4);
        }
        if name.ends_with(".7z") {
            return Ok(Self::SevenZip);
        }
        if name.ends_with(".zip") {
            return Ok(Self::Zip);
        }
        if name.ends_with(".tar") {
            return Ok(Self::Tar);
        }
        if name.ends_with(".zst") || name.ends_with(".zstd") {
            return Ok(Self::Zst);
        }
        if name.ends_with(".lz4") {
            return Ok(Self::Lz4);
        }
        if name.ends_with(".gz") {
            return Ok(Self::Gz);
        }
        if name.ends_with(".bz2") {
            return Ok(Self::Bz2);
        }
        if name.ends_with(".xz") {
            return Ok(Self::Xz);
        }

        bail!(
            "Cannot detect compression format from {}. Use an extension like .7z, .zip, .tar.gz, .zst, .lz4, etc.",
            path.display()
        )
    }
}

impl fmt::Display for CompressionFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    Fastest,
    Fast,
    Normal,
    Maximum,
    Ultra,
}

impl CompressionLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fastest => "Fastest",
            Self::Fast => "Fast",
            Self::Normal => "Normal",
            Self::Maximum => "Maximum",
            Self::Ultra => "Ultra",
        }
    }

    pub fn zip_level(self) -> Option<i64> {
        Some(match self {
            Self::Fastest => 1,
            Self::Fast => 3,
            Self::Normal => 6,
            Self::Maximum => 8,
            Self::Ultra => 9,
        })
    }

    pub fn zip_deflate_level(self) -> u32 {
        match self {
            Self::Fastest => 3,
            Self::Fast => 5,
            Self::Normal => 6,
            Self::Maximum => 8,
            Self::Ultra => 9,
        }
    }

    pub fn gzip_level(self) -> u32 {
        match self {
            Self::Fastest => 1,
            Self::Fast => 3,
            Self::Normal => 6,
            Self::Maximum => 8,
            Self::Ultra => 9,
        }
    }

    pub fn bzip2_level(self) -> u32 {
        match self {
            Self::Fastest => 1,
            Self::Fast => 3,
            Self::Normal => 6,
            Self::Maximum => 8,
            Self::Ultra => 9,
        }
    }

    pub fn xz_level(self) -> u32 {
        match self {
            Self::Fastest => 1,
            Self::Fast => 3,
            Self::Normal => 6,
            Self::Maximum => 8,
            Self::Ultra => 9,
        }
    }

    pub fn lz4_level(self) -> u32 {
        match self {
            Self::Fastest => 1,
            Self::Fast => 3,
            Self::Normal => 6,
            Self::Maximum => 9,
            Self::Ultra => 12,
        }
    }

    pub fn zstd_level(self) -> i64 {
        match self {
            Self::Fastest => 1,
            Self::Fast => 3,
            Self::Normal => 9,
            Self::Maximum => 15,
            Self::Ultra => 19,
        }
    }

    pub fn sevenz_level(self) -> u32 {
        match self {
            Self::Fastest => 1,
            Self::Fast => 3,
            Self::Normal => 5,
            Self::Maximum => 7,
            Self::Ultra => 9,
        }
    }

    pub fn sevenz_dictionary_size(self) -> u32 {
        match self {
            Self::Fastest => 1 * 1024 * 1024,
            Self::Fast => 4 * 1024 * 1024,
            Self::Normal => 8 * 1024 * 1024,
            Self::Maximum => 16 * 1024 * 1024,
            Self::Ultra => 32 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipCompressionMethod {
    Deflate,
    Stored,
    Bzip2,
    Zstd,
    Xz,
}

impl ZipCompressionMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Deflate => "Deflate",
            Self::Stored => "Store",
            Self::Bzip2 => "BZip2",
            Self::Zstd => "Zstandard",
            Self::Xz => "XZ",
        }
    }

    pub fn to_zip_method(self) -> CompressionMethod {
        match self {
            Self::Deflate => CompressionMethod::Deflated,
            Self::Stored => CompressionMethod::Stored,
            Self::Bzip2 => CompressionMethod::Bzip2,
            Self::Zstd => CompressionMethod::Zstd,
            Self::Xz => CompressionMethod::Xz,
        }
    }

    pub fn supports_level(self) -> bool {
        !matches!(self, Self::Stored)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionOptions {
    pub format: CompressionFormat,
    pub level: CompressionLevel,
    pub zip_method: ZipCompressionMethod,
    pub thread_count: u32,
    pub password: Option<String>,
    pub encrypt_file_names: bool,
    pub split_volume_size: Option<u64>,
    pub sfx: bool,
}

impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            format: CompressionFormat::Zip,
            level: CompressionLevel::Normal,
            zip_method: ZipCompressionMethod::Deflate,
            thread_count: 1,
            password: None,
            encrypt_file_names: false,
            split_volume_size: None,
            sfx: false,
        }
    }
}

impl CompressionOptions {
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref().filter(|value| !value.is_empty())
    }

    pub fn split_volume_size(&self) -> Option<u64> {
        self.split_volume_size.filter(|value| *value > 0)
    }
}

fn split_volume_name_parts(file_name: &str) -> Option<(&str, u32, usize)> {
    let split_at = file_name.rfind('.')?;
    let suffix = &file_name[split_at + 1..];
    if suffix.len() < 3 || !suffix.chars().all(|value| value.is_ascii_digit()) {
        return None;
    }

    let part_index = suffix.parse::<u32>().ok()?;
    if part_index == 0 {
        return None;
    }

    Some((&file_name[..split_at], part_index, suffix.len()))
}

fn supported_archive_suffixes() -> &'static [&'static str] {
    &[
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tgz", ".tbz2", ".txz", ".7z", ".zip", ".tar", ".rar",
        ".bzip2", ".gz", ".bz2", ".xz",
    ]
}

fn split_volume_base_name(file_name: &str) -> Option<(&str, u32, usize)> {
    let (base_name, part_index, width) = split_volume_name_parts(file_name)?;
    let lower = base_name.to_ascii_lowercase();
    supported_archive_suffixes()
        .iter()
        .any(|suffix| lower.ends_with(suffix))
        .then_some((base_name, part_index, width))
}

pub(crate) fn split_volume_base_path(path: &Path) -> Option<(PathBuf, u32, usize)> {
    let file_name = path.file_name()?.to_str()?;
    let (base_name, part_index, width) = split_volume_base_name(file_name)?;
    Some((path.with_file_name(base_name), part_index, width))
}

fn split_volume_prefix(file_name: &str) -> String {
    format!("{file_name}.")
}

pub(crate) fn split_volume_output_path(
    destination: &Path,
    part_index: u32,
    width: usize,
) -> Result<PathBuf> {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "Invalid archive destination file name: {}",
                destination.display()
            )
        })?;
    Ok(destination.with_file_name(format!("{file_name}.{part_index:0width$}", width = width)))
}

pub(crate) fn collect_existing_split_volume_paths(destination: &Path) -> Result<Vec<PathBuf>> {
    let parent = destination.parent().ok_or_else(|| {
        anyhow!(
            "Cannot determine the parent folder for {}",
            destination.display()
        )
    })?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "Invalid archive destination file name: {}",
                destination.display()
            )
        })?;
    let prefix = split_volume_prefix(file_name);
    let mut matches = fs::read_dir(parent)
        .with_context(|| format!("Failed to enumerate {}", parent.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .and_then(|candidate| candidate.strip_prefix(&prefix))
                .is_some_and(|suffix| {
                    suffix.len() >= 3 && suffix.chars().all(|value| value.is_ascii_digit())
                })
        })
        .collect::<Vec<_>>();
    matches.sort();
    Ok(matches)
}

pub fn parse_volume_size_spec(spec: &str) -> Result<u64> {
    let normalized = spec.trim().replace(' ', "");
    if normalized.is_empty() {
        bail!("Split volume size is empty");
    }

    let digit_count = normalized
        .chars()
        .take_while(|value| value.is_ascii_digit())
        .count();
    if digit_count == 0 {
        bail!("Split volume size must start with a number");
    }

    let value = normalized[..digit_count]
        .parse::<u64>()
        .context("Invalid split volume size number")?;
    if value == 0 {
        bail!("Split volume size must be greater than zero");
    }

    let unit = normalized[digit_count..].to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "" | "b" => 1u64,
        "k" | "kb" => 1024u64,
        "m" | "mb" => 1024u64.pow(2),
        "g" | "gb" => 1024u64.pow(3),
        "t" | "tb" => 1024u64.pow(4),
        _ => bail!(
            "Unsupported split volume unit `{}`. Use B, K, M, G, or T.",
            unit
        ),
    };

    value
        .checked_mul(multiplier)
        .ok_or_else(|| anyhow!("Split volume size is too large"))
}

pub fn all_supported_formats() -> &'static [&'static str] {
    &[
        ".7z", ".zip", ".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz",
        ".tar.zst", ".tzst", ".tar.lz4", ".tlz4", ".gz", ".bz2", ".bzip2", ".xz", ".zst", ".zstd",
        ".lz4", ".rar", ".wim", ".iso",
    ]
}

pub fn file_dialog_extensions() -> &'static [&'static str] {
    &[
        "7z", "zip", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst", "tzst", "lz4", "tlz4",
        "rar",
    ]
}

pub fn default_output_dir(archive: &Path) -> PathBuf {
    let file_name = archive
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let file_name = split_volume_base_name(&file_name)
        .map(|(base_name, _, _)| base_name.to_string())
        .unwrap_or(file_name);
    let lower = file_name.to_ascii_lowercase();

    let stripped = [
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tar.lz4", ".tgz", ".tbz2", ".txz", ".tzst",
        ".tlz4", ".7z", ".zip", ".tar", ".rar", ".bzip2", ".gz", ".bz2", ".xz", ".zst", ".zstd",
        ".lz4",
    ]
    .iter()
    .find_map(|suffix| lower.ends_with(suffix).then_some(*suffix))
    .map(|suffix| file_name[..file_name.len() - suffix.len()].to_string())
    .filter(|value| !value.is_empty())
    .unwrap_or(file_name);

    archive.with_file_name(stripped)
}

pub fn suggested_extract_output_dir(
    archive: &Path,
    entries: &[ArchiveEntry],
    keep_paths: bool,
) -> PathBuf {
    let default_output = default_output_dir(archive);
    if !keep_paths {
        return default_output;
    }

    let Some(default_name) = default_output
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
    else {
        return default_output;
    };

    let mut common_root_name: Option<String> = None;
    let mut wrapped_by_root_directory = false;

    for entry in entries {
        let components = entry
            .path
            .components()
            .filter_map(|component| match component {
                Component::Normal(value) => Some(value.to_string_lossy().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();

        let Some(first_component) = components.first() else {
            continue;
        };

        if let Some(existing_root_name) = &common_root_name {
            if !path_name_eq(existing_root_name, first_component) {
                return default_output;
            }
        } else {
            common_root_name = Some(first_component.clone());
        }

        if components.len() > 1 || (entry.is_dir && components.len() == 1) {
            wrapped_by_root_directory = true;
        }
    }

    let Some(root_name) = common_root_name else {
        return default_output;
    };

    if wrapped_by_root_directory && path_name_eq(&root_name, &default_name) {
        return default_output
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
    }

    default_output
}

fn path_name_eq(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

pub(crate) fn list_tar<R>(reader: R) -> Result<Vec<ArchiveEntry>>
where
    R: Read,
{
    let mut archive = TarArchive::new(reader);
    let mut entries = Vec::new();

    for entry in archive
        .entries()
        .context("Failed to enumerate TAR entries")?
    {
        let entry = entry.context("Failed to read TAR entry")?;
        let safe_path = sanitize_relative_path(&entry.path().context("Invalid TAR path")?);
        if safe_path.as_os_str().is_empty() {
            continue;
        }

        let size = if entry.header().entry_type().is_dir() {
            None
        } else {
            Some(entry.header().size().context("Invalid TAR entry size")?)
        };

        entries.push(ArchiveEntry {
            path: safe_path,
            is_dir: entry.header().entry_type().is_dir(),
            uncompressed_size: size,
            compressed_size: None,
        });
    }

    Ok(entries)
}

pub(crate) fn extract_tar<R, F>(
    reader: R,
    options: &ExtractOptions,
    plan: &ExtractPathPlan,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<ExtractionReport>
where
    R: Read,
    F: FnMut(u64),
{
    let mut archive = TarArchive::new(reader);
    let mut report = ExtractionReport::new(options.output_dir.clone());

    for entry in archive
        .entries()
        .context("Failed to enumerate TAR entries")?
    {
        ensure_not_canceled(should_cancel)?;
        let mut entry = entry.context("Failed to read TAR entry")?;
        let safe_path = sanitize_relative_path(&entry.path().context("Invalid TAR path")?);
        if safe_path.as_os_str().is_empty() {
            continue;
        }

        if plan.should_skip(&safe_path) {
            continue;
        }

        let destination = resolve_output_path(&safe_path, options, plan)?;
        let is_dir = entry.header().entry_type().is_dir();
        if is_dir {
            create_directory(&destination, &mut report)?;
            continue;
        }

        if !prepare_output_file(&destination, options.overwrite_mode, &mut report)? {
            continue;
        }

        extract_stream_to_destination(
            &mut entry,
            &destination,
            &mut report,
            progress,
            should_cancel,
            options.scan_files,
        )?;
    }

    Ok(report)
}

fn sevenz_password_from_option(password: Option<&str>) -> SevenZPassword {
    password
        .filter(|value| !value.is_empty())
        .map(SevenZPassword::new)
        .unwrap_or_else(SevenZPassword::empty)
}

fn map_sevenz_password_error(
    path: &Path,
    error: sevenz_rust2::Error,
    action: &str,
) -> anyhow::Error {
    match error {
        sevenz_rust2::Error::PasswordRequired => anyhow!(
            "Failed to {action} 7Z archive {}: a password is required",
            path.display()
        ),
        sevenz_rust2::Error::MaybeBadPassword(_) => anyhow!(
            "Failed to {action} 7Z archive {}: the password is incorrect",
            path.display()
        ),
        other => anyhow!("Failed to {action} 7Z archive {}: {other}", path.display()),
    }
}

fn map_zip_password_error(
    path: &Path,
    error: zip::result::ZipError,
    action: &str,
) -> anyhow::Error {
    match error {
        zip::result::ZipError::UnsupportedArchive(message)
            if message == zip::result::ZipError::PASSWORD_REQUIRED =>
        {
            anyhow!(
                "Failed to {action} ZIP archive {}: a password is required",
                path.display()
            )
        }
        zip::result::ZipError::InvalidPassword => anyhow!(
            "Failed to {action} ZIP archive {}: the password is incorrect",
            path.display()
        ),
        other => anyhow!("Failed to {action} ZIP archive {}: {other}", path.display()),
    }
}

fn open_zip_entry_by_index<'a>(
    archive: &'a mut ZipArchive<File>,
    file_number: usize,
    password: Option<&str>,
    path: &Path,
    action: &str,
) -> Result<zip::read::ZipFile<'a>> {
    let password = password.filter(|value| !value.is_empty());
    let entry = match password {
        Some(password) => archive.by_index_decrypt(file_number, password.as_bytes()),
        None => archive.by_index(file_number),
    }
    .map_err(|error| map_zip_password_error(path, error, action))?;
    Ok(entry)
}

pub(crate) fn list_zip(
    path: &Path,
    password: Option<&str>,
    encoding: FilenameEncoding,
) -> Result<Vec<ArchiveEntry>> {
    let file = File::open(path).with_context(open_context(path))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read ZIP archive {}", path.display()))?;
    let mut entries = Vec::with_capacity(archive.len());

    for index in 0..archive.len() {
        let entry = open_zip_entry_by_index(&mut archive, index, password, path, "inspect")?;
        let decoded_name = decode_zip_filename(entry.name_raw(), encoding);
        let safe_path = entry
            .enclosed_name()
            .unwrap_or_else(|| sanitize_relative_path(Path::new(&decoded_name)));
        if safe_path.as_os_str().is_empty() {
            continue;
        }

        entries.push(ArchiveEntry {
            path: safe_path,
            is_dir: entry.is_dir(),
            uncompressed_size: Some(entry.size()),
            compressed_size: Some(entry.compressed_size()),
        });
    }

    Ok(entries)
}

pub(crate) fn extract_zip<F>(
    path: &Path,
    options: &ExtractOptions,
    plan: &ExtractPathPlan,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<ExtractionReport>
where
    F: FnMut(u64),
{
    let file = File::open(path).with_context(open_context(path))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read ZIP archive {}", path.display()))?;
    let mut report = ExtractionReport::new(options.output_dir.clone());

    for index in 0..archive.len() {
        ensure_not_canceled(should_cancel)?;
        let mut entry =
            open_zip_entry_by_index(&mut archive, index, options.password(), path, "extract")?;
        let decoded_name = decode_zip_filename(entry.name_raw(), options.filename_encoding);
        let safe_path = entry
            .enclosed_name()
            .unwrap_or_else(|| sanitize_relative_path(Path::new(&decoded_name)));
        if safe_path.as_os_str().is_empty() {
            continue;
        }

        if plan.should_skip(&safe_path) {
            continue;
        }

        let destination = resolve_output_path(&safe_path, options, plan)?;
        if entry.is_dir() {
            create_directory(&destination, &mut report)?;
            continue;
        }

        if !prepare_output_file(&destination, options.overwrite_mode, &mut report)? {
            continue;
        }

        extract_stream_to_destination(
            &mut entry,
            &destination,
            &mut report,
            progress,
            should_cancel,
            options.scan_files,
        )?;
    }

    Ok(report)
}

pub(crate) fn list_single_file(path: &Path, suffix: &str) -> Result<Vec<ArchiveEntry>> {
    let output_name = single_file_output_name(path, suffix)?;
    let compressed_size = path.metadata().ok().map(|meta| meta.len());

    Ok(vec![ArchiveEntry {
        path: output_name.into(),
        is_dir: false,
        uncompressed_size: None,
        compressed_size,
    }])
}

pub(crate) fn extract_single_file<R, F>(
    mut reader: R,
    archive_path: &Path,
    suffix: &str,
    options: &ExtractOptions,
    plan: &ExtractPathPlan,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<ExtractionReport>
where
    R: Read,
    F: FnMut(u64),
{
    let output_name = single_file_output_name(archive_path, suffix)?;
    let relative = PathBuf::from(output_name);
    let mut report = ExtractionReport::new(options.output_dir.clone());

    if plan.should_skip(&relative) {
        return Ok(report);
    }

    let destination = resolve_output_path(&relative, options, plan)?;

    if prepare_output_file(&destination, options.overwrite_mode, &mut report)? {
        extract_stream_to_destination(
            &mut reader,
            &destination,
            &mut report,
            progress,
            should_cancel,
            options.scan_files,
        )?;
    }

    Ok(report)
}

pub(crate) fn list_7z(path: &Path, password: Option<&str>) -> Result<Vec<ArchiveEntry>> {
    let password = sevenz_password_from_option(password);
    let archive = SevenZArchive::open_with_password(path, &password)
        .map_err(|error| map_sevenz_password_error(path, error, "read"))?;
    let mut entries = Vec::with_capacity(archive.files.len());

    for entry in archive.files {
        let raw_path = Path::new(entry.name());
        if contains_unsafe_components(raw_path) {
            bail!("7Z archive contains an unsafe entry path: {}", entry.name());
        }

        let safe_path = sanitize_relative_path(raw_path);
        if safe_path.as_os_str().is_empty() {
            continue;
        }

        entries.push(ArchiveEntry {
            path: safe_path,
            is_dir: entry.is_directory(),
            uncompressed_size: entry.has_stream().then_some(entry.size()),
            compressed_size: entry.has_stream().then_some(entry.compressed_size),
        });
    }

    Ok(entries)
}

pub(crate) fn extract_7z<F>(
    path: &Path,
    options: &ExtractOptions,
    plan: &ExtractPathPlan,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<ExtractionReport>
where
    F: FnMut(u64),
{
    let password = sevenz_password_from_option(options.password());
    let mut archive = SevenZReader::open(path, password)
        .map_err(|error| map_sevenz_password_error(path, error, "read"))?;
    let mut report = ExtractionReport::new(options.output_dir.clone());

    archive
        .for_each_entries(|entry, reader| {
            if should_cancel() {
                return Err(
                    io::Error::new(io::ErrorKind::Interrupted, "Operation canceled").into(),
                );
            }

            let raw_path = Path::new(entry.name());
            if contains_unsafe_components(raw_path) {
                return Err(io::Error::other(format!(
                    "7Z archive contains an unsafe entry path: {}",
                    entry.name()
                ))
                .into());
            }

            let safe_path = sanitize_relative_path(raw_path);
            if safe_path.as_os_str().is_empty() {
                return Ok(true);
            }

            if plan.should_skip(&safe_path) {
                return Ok(true);
            }

            let destination = resolve_output_path(&safe_path, options, plan)
                .map_err(|error| io::Error::other(error.to_string()))?;

            if entry.is_directory() {
                create_directory(&destination, &mut report)
                    .map_err(|error| io::Error::other(error.to_string()))?;
                return Ok(true);
            }

            if !prepare_output_file(&destination, options.overwrite_mode, &mut report)
                .map_err(|error| io::Error::other(error.to_string()))?
            {
                return Ok(true);
            }

            extract_stream_to_destination(
                reader,
                &destination,
                &mut report,
                progress,
                should_cancel,
                options.scan_files,
            )
            .map_err(|error| io::Error::other(error.to_string()))?;

            Ok(true)
        })
        .map_err(|error| map_sevenz_password_error(path, error, "extract"))?;

    Ok(report)
}

fn create_xz_encoder<W: Write>(writer: W, options: &CompressionOptions) -> Result<XzEncoder<W>> {
    let thread_count = options.thread_count.max(1);
    if thread_count > 1 {
        let mut builder = MtStreamBuilder::new();
        builder
            .threads(thread_count)
            .preset(options.level.xz_level())
            .check(XzCheck::Crc64);
        let stream = builder.encoder().with_context(|| {
            format!("Failed to initialize multithreaded XZ compression with {thread_count} threads")
        })?;
        Ok(XzEncoder::new_stream(writer, stream))
    } else {
        Ok(XzEncoder::new(writer, options.level.xz_level()))
    }
}

fn create_zstd_encoder<W: Write>(
    writer: W,
    options: &CompressionOptions,
) -> Result<ZstdEncoder<'static, W>> {
    let level = options.level.zstd_level() as i32;
    let thread_count = options.thread_count.max(1) as u32;
    let mut encoder =
        ZstdEncoder::new(writer, level).with_context(|| "Failed to initialize Zstd compression")?;
    if thread_count > 1 {
        encoder
            .multithread(thread_count)
            .with_context(|| format!("Failed to enable {thread_count} threads for Zstd"))?;
    }
    Ok(encoder)
}

fn create_tar_zst_archive<F>(
    output_file: File,
    sources: &[PathBuf],
    report: &mut CompressionReport,
    options: &CompressionOptions,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()>
where
    F: FnMut(u64),
{
    let encoder = create_zstd_encoder(output_file, options)?;
    let mut builder = TarBuilder::new(encoder);
    let mut seen_entries = HashSet::new();
    add_sources_to_tar(
        &mut builder,
        sources,
        report,
        &mut seen_entries,
        excluded_paths,
        excluded_outputs,
        progress,
        should_cancel,
    )?;
    builder
        .finish()
        .with_context(|| "Failed to finalize tar.zst archive")?;
    builder
        .into_inner()
        .with_context(|| "Failed to finalize tar.zst archive")?
        .finish()
        .with_context(|| "Failed to finish zstd stream")?;
    Ok(())
}

fn create_tar_lz4_archive<F>(
    output_file: File,
    sources: &[PathBuf],
    report: &mut CompressionReport,
    options: &CompressionOptions,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()>
where
    F: FnMut(u64),
{
    let _ = options;
    let encoder = lz4_flex::frame::FrameEncoder::new(output_file);
    let mut builder = TarBuilder::new(encoder);
    let mut seen_entries = HashSet::new();
    add_sources_to_tar(
        &mut builder,
        sources,
        report,
        &mut seen_entries,
        excluded_paths,
        excluded_outputs,
        progress,
        should_cancel,
    )?;
    builder
        .finish()
        .with_context(|| "Failed to finalize tar.lz4 archive")?;
    builder
        .into_inner()
        .with_context(|| "Failed to finalize tar.lz4 archive")?
        .finish()
        .with_context(|| "Failed to finish lz4 frame")?;
    Ok(())
}

const SEVENZ_SOLID_BLOCK_MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const ZIP_PROGRESS_EMIT_BYTES: u64 = 1024 * 1024;
const ZIP_PARALLEL_DEFLATE_MIN_FILE_BYTES: u64 = 32 * 1024 * 1024;
const ZIP_PARALLEL_DEFLATE_MIN_CHUNK_BYTES: u64 = 8 * 1024 * 1024;
const ZIP_PARALLEL_DEFLATE_MAX_CHUNK_BYTES: u64 = 16 * 1024 * 1024;
const ZIP_PARALLEL_DEFLATE_WINDOW_BYTES: u64 = 32 * 1024;
const ZIP_PARALLEL_DEFLATE_OUTPUT_BUFFER_BYTES: usize = 1024 * 1024;
const ZIP_PARALLEL_DIRECT_DEFLATE_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const ZIP_PARALLEL_DIRECT_DEFLATE_MAX_INFLIGHT_INPUT_BYTES: u64 = 1024 * 1024 * 1024;
const ZIP_PARALLEL_DIRECT_MIXED_MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
const ZIP_PARALLEL_DIRECT_MIXED_MAX_FILE_COUNT: usize = 1024;
const ZIP_LOCAL_FILE_HEADER_SIGNATURE: u32 = 0x0403_4B50;
const ZIP_CENTRAL_DIRECTORY_HEADER_SIGNATURE: u32 = 0x0201_4B50;
const ZIP_END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0605_4B50;
const ZIP64_END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0606_4B50;
const ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_SIGNATURE: u32 = 0x0706_4B50;
const ZIP_UTF8_GENERAL_PURPOSE_FLAG: u16 = 1 << 11;
const ZIP_STORED_METHOD_ID: u16 = 0;
const ZIP_DEFLATE_METHOD_ID: u16 = 8;
const ZIP_BZIP2_METHOD_ID: u16 = 12;
const ZIP_ZSTD_METHOD_ID: u16 = 93;
const ZIP_XZ_METHOD_ID: u16 = 95;
const ZIP64_U32_MAX: u64 = u32::MAX as u64;
const ZIP_REGULAR_FILE_UNIX_MODE: u32 = 0o100644;
const ZIP_DIRECTORY_UNIX_MODE: u32 = 0o040755;
const ZIP_DIRECTORY_DOS_ATTRIBUTE: u32 = 0x10;

#[derive(Debug, Clone)]
struct PreparedZipDirectory {
    archive_path: String,
}

#[derive(Debug, Clone)]
struct PreparedZipFile {
    source_path: PathBuf,
    archive_path: String,
    archive_index: usize,
    input_len: u64,
    zip_method: ZipCompressionMethod,
}

#[derive(Debug, Clone)]
struct ZipWorkerChunk {
    files: Vec<PreparedZipFile>,
    total_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct ParallelDeflateChunkSpec {
    index: usize,
    offset: u64,
    input_len: u64,
    is_last: bool,
}

#[derive(Debug)]
struct ParallelDeflateChunkOutput {
    index: usize,
    compressed: Vec<u8>,
    crc: Crc32Hasher,
}

#[derive(Debug)]
enum ParallelDeflateWorkerMessage {
    Progress(u64),
    Chunk(Result<ParallelDeflateChunkOutput>),
}

#[derive(Debug, Clone, Copy)]
struct SingleEntryZipHeaderLayout {
    version_needed: u16,
    version_made_by: u16,
    general_purpose_flag: u16,
    method: u16,
    last_mod_time: u16,
    last_mod_date: u16,
    use_zip64: bool,
}

#[derive(Debug, Clone)]
struct ManualZipCentralDirectoryEntry {
    header_layout: SingleEntryZipHeaderLayout,
    entry_name: Vec<u8>,
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
    local_header_offset: u64,
    external_attributes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParallelDeflateEntryOutput {
    crc32: u32,
    compressed_size: u64,
}

#[derive(Debug)]
struct ParallelZipFileEntryOutput {
    archive_index: usize,
    header_layout: SingleEntryZipHeaderLayout,
    entry_name: Vec<u8>,
    compressed: Vec<u8>,
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
    external_attributes: u32,
}

#[derive(Debug)]
struct ParallelZipFileInput {
    bytes: Vec<u8>,
    crc32: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZipParallelPlan {
    worker_count: usize,
    task_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZipCompressionStrategy {
    DirectSerial,
    ParallelWorkers(ZipParallelPlan),
}

#[derive(Debug)]
struct ZipWorkerTask {
    worker_path: PathBuf,
    chunk: ZipWorkerChunk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SevenZCompressionStrategy {
    SolidLzma2,
    Copy,
}

#[derive(Debug, Clone)]
struct PreparedSevenZDirectory {
    source_path: PathBuf,
    archive_path: String,
}

#[derive(Debug, Clone)]
struct PreparedSevenZFile {
    source_path: PathBuf,
    archive_path: String,
    input_len: u64,
    strategy: SevenZCompressionStrategy,
}

#[derive(Debug, Clone)]
enum PreparedSevenZEntry {
    Directory(PreparedSevenZDirectory),
    File(PreparedSevenZFile),
}

struct SharedProgressContext<'a, F, C> {
    progress: &'a mut F,
    should_cancel: &'a mut C,
}

struct SevenZProgressFileReader<'a, F, C> {
    path: PathBuf,
    reader: Option<File>,
    finished: bool,
    shared_context: Rc<RefCell<SharedProgressContext<'a, F, C>>>,
}

impl<'a, F, C> SevenZProgressFileReader<'a, F, C> {
    fn new(path: PathBuf, shared_context: Rc<RefCell<SharedProgressContext<'a, F, C>>>) -> Self {
        Self {
            path,
            reader: None,
            finished: false,
            shared_context,
        }
    }
}

impl<F, C> Read for SevenZProgressFileReader<'_, F, C>
where
    F: FnMut(u64),
    C: FnMut() -> bool,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.finished {
            return Ok(0);
        }

        {
            let mut context = self.shared_context.borrow_mut();
            if (context.should_cancel)() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Operation canceled",
                ));
            }
        }

        if self.reader.is_none() {
            self.reader = Some(File::open(&self.path)?);
        }

        let bytes_read = self.reader.as_mut().unwrap().read(buf)?;
        if bytes_read == 0 {
            self.finished = true;
            self.reader = None;
            return Ok(0);
        }

        let mut context = self.shared_context.borrow_mut();
        (context.progress)(bytes_read as u64);
        Ok(bytes_read)
    }
}

fn create_sevenz_archive<F>(
    output_file: File,
    sources: &[PathBuf],
    report: &mut CompressionReport,
    options: &CompressionOptions,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()>
where
    F: FnMut(u64),
{
    let mut writer = SevenZWriter::new(output_file).context("Failed to initialize 7Z writer")?;
    writer.set_encrypt_header(options.encrypt_file_names && options.password().is_some());
    let lzma2_methods = sevenz_lzma2_methods(options);
    let copy_methods = sevenz_copy_methods(options);
    writer.set_content_methods(lzma2_methods.clone());

    let mut prepared_entries = Vec::new();
    let mut seen_entries = HashSet::new();

    for source in sources {
        ensure_not_canceled(should_cancel)?;
        if !source.exists() {
            bail!("Compression source not found: {}", source.display());
        }

        let root_name = source
            .file_name()
            .ok_or_else(|| anyhow!("Failed to derive an archive name from {}", source.display()))?;
        collect_sevenz_entries(
            source,
            &PathBuf::from(root_name),
            &mut prepared_entries,
            &mut seen_entries,
            excluded_paths,
            excluded_outputs,
            should_cancel,
        )?;
    }

    let shared_context = Rc::new(RefCell::new(SharedProgressContext {
        progress,
        should_cancel,
    }));
    let mut solid_entries = Vec::new();
    let mut solid_readers = Vec::new();
    let mut solid_batch_bytes = 0u64;

    for entry in prepared_entries {
        ensure_not_canceled_shared(&shared_context)?;

        match entry {
            PreparedSevenZEntry::Directory(directory) => {
                writer
                    .push_archive_entry::<&[u8]>(
                        SevenZArchiveEntry::from_path(
                            directory.source_path,
                            directory.archive_path,
                        ),
                        None,
                    )
                    .with_context(|| format!("Failed to add directory to 7Z archive"))?;
                report.directories_added += 1;
            }
            PreparedSevenZEntry::File(file) => match file.strategy {
                SevenZCompressionStrategy::Copy => {
                    flush_sevenz_solid_batch(
                        &mut writer,
                        &mut solid_entries,
                        &mut solid_readers,
                        &mut solid_batch_bytes,
                        &lzma2_methods,
                    )?;
                    push_sevenz_non_solid_file(
                        &mut writer,
                        file,
                        &copy_methods,
                        Rc::clone(&shared_context),
                        report,
                    )?;
                    writer.set_content_methods(lzma2_methods.clone());
                }
                SevenZCompressionStrategy::SolidLzma2 => {
                    if file.input_len >= SEVENZ_SOLID_BLOCK_MAX_BYTES {
                        flush_sevenz_solid_batch(
                            &mut writer,
                            &mut solid_entries,
                            &mut solid_readers,
                            &mut solid_batch_bytes,
                            &lzma2_methods,
                        )?;
                        push_sevenz_non_solid_file(
                            &mut writer,
                            file,
                            &lzma2_methods,
                            Rc::clone(&shared_context),
                            report,
                        )?;
                        continue;
                    }

                    if solid_batch_bytes.saturating_add(file.input_len)
                        > SEVENZ_SOLID_BLOCK_MAX_BYTES
                        && !solid_entries.is_empty()
                    {
                        flush_sevenz_solid_batch(
                            &mut writer,
                            &mut solid_entries,
                            &mut solid_readers,
                            &mut solid_batch_bytes,
                            &lzma2_methods,
                        )?;
                    }

                    report.files_added += 1;
                    report.input_bytes += file.input_len;
                    solid_batch_bytes = solid_batch_bytes.saturating_add(file.input_len);
                    solid_entries.push(SevenZArchiveEntry::from_path(
                        &file.source_path,
                        file.archive_path,
                    ));
                    solid_readers.push(SevenZSourceReader::from(SevenZProgressFileReader::new(
                        file.source_path,
                        Rc::clone(&shared_context),
                    )));
                }
            },
        }
    }

    flush_sevenz_solid_batch(
        &mut writer,
        &mut solid_entries,
        &mut solid_readers,
        &mut solid_batch_bytes,
        &lzma2_methods,
    )?;

    writer
        .finish()
        .context("Failed to finalize 7Z archive output")?;
    Ok(())
}

fn sevenz_lzma2_methods(options: &CompressionOptions) -> Vec<SevenZEncoderConfiguration> {
    let mut lzma2_options = if options.thread_count > 1 {
        SevenZLzma2Options::from_level_mt(
            options.level.sevenz_level(),
            options.thread_count.max(1),
            (options.level.sevenz_dictionary_size() as u64).saturating_mul(4),
        )
    } else {
        SevenZLzma2Options::from_level(options.level.sevenz_level())
    };
    lzma2_options.set_dictionary_size(options.level.sevenz_dictionary_size());
    let mut methods = vec![lzma2_options.into()];
    if let Some(password) = options.password() {
        methods.insert(
            0,
            SevenZAesEncoderOptions::new(SevenZPassword::new(password)).into(),
        );
    }
    methods
}

fn sevenz_copy_methods(options: &CompressionOptions) -> Vec<SevenZEncoderConfiguration> {
    let mut methods = vec![SevenZEncoderMethod::COPY.into()];
    if let Some(password) = options.password() {
        methods.insert(
            0,
            SevenZAesEncoderOptions::new(SevenZPassword::new(password)).into(),
        );
    }
    methods
}

fn collect_sevenz_entries(
    source: &Path,
    archive_path: &Path,
    prepared_entries: &mut Vec<PreparedSevenZEntry>,
    seen_entries: &mut HashSet<String>,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()> {
    ensure_not_canceled(should_cancel)?;
    if should_skip_compression_path(source, excluded_paths, excluded_outputs) {
        return Ok(());
    }

    if source.is_dir() {
        let entry_name = normalize_tar_entry_path(archive_path)?;
        if seen_entries.insert(format!("dir:{entry_name}")) {
            prepared_entries.push(PreparedSevenZEntry::Directory(PreparedSevenZDirectory {
                source_path: source.to_path_buf(),
                archive_path: entry_name,
            }));
        }

        let mut children = fs::read_dir(source)
            .with_context(|| format!("Failed to enumerate {}", source.display()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("Failed to read {}", source.display()))?;
        children.sort_by_key(|entry| entry.path());

        for child in children {
            let child_path = child.path();
            let child_name = child.file_name();
            collect_sevenz_entries(
                &child_path,
                &archive_path.join(child_name),
                prepared_entries,
                seen_entries,
                excluded_paths,
                excluded_outputs,
                should_cancel,
            )?;
        }

        return Ok(());
    }

    if !source.is_file() {
        bail!("Unsupported compression source type: {}", source.display());
    }

    let entry_name = normalize_tar_entry_path(archive_path)?;
    if !seen_entries.insert(format!("file:{entry_name}")) {
        bail!("Duplicate archive entry name: {entry_name}");
    }

    let input_len = source
        .metadata()
        .with_context(|| format!("Failed to inspect {}", source.display()))?
        .len();
    prepared_entries.push(PreparedSevenZEntry::File(PreparedSevenZFile {
        source_path: source.to_path_buf(),
        archive_path: entry_name,
        input_len,
        strategy: if is_likely_incompressible_path(source) {
            SevenZCompressionStrategy::Copy
        } else {
            SevenZCompressionStrategy::SolidLzma2
        },
    }));
    Ok(())
}

fn flush_sevenz_solid_batch<'a, F, C>(
    writer: &mut SevenZWriter<File>,
    solid_entries: &mut Vec<SevenZArchiveEntry>,
    solid_readers: &mut Vec<SevenZSourceReader<SevenZProgressFileReader<'a, F, C>>>,
    solid_batch_bytes: &mut u64,
    methods: &[SevenZEncoderConfiguration],
) -> Result<()>
where
    F: FnMut(u64),
    C: FnMut() -> bool,
{
    if solid_entries.is_empty() {
        return Ok(());
    }

    writer.set_content_methods(methods.to_vec());
    writer
        .push_archive_entries(std::mem::take(solid_entries), std::mem::take(solid_readers))
        .context("Failed to add solid 7Z batch")?;
    *solid_batch_bytes = 0;
    Ok(())
}

fn push_sevenz_non_solid_file<'a, F, C>(
    writer: &mut SevenZWriter<File>,
    file: PreparedSevenZFile,
    methods: &[SevenZEncoderConfiguration],
    shared_context: Rc<RefCell<SharedProgressContext<'a, F, C>>>,
    report: &mut CompressionReport,
) -> Result<()>
where
    F: FnMut(u64),
    C: FnMut() -> bool,
{
    writer.set_content_methods(methods.to_vec());
    let entry = SevenZArchiveEntry::from_path(&file.source_path, file.archive_path);
    let source_path = file.source_path;
    writer
        .push_archive_entry(
            entry,
            Some(SevenZProgressFileReader::new(
                source_path.clone(),
                shared_context,
            )),
        )
        .with_context(|| format!("Failed to add file {}", source_path.display()))?;
    report.files_added += 1;
    report.input_bytes += file.input_len;
    Ok(())
}

fn ensure_not_canceled_shared<F, C>(
    shared_context: &Rc<RefCell<SharedProgressContext<'_, F, C>>>,
) -> Result<()>
where
    F: FnMut(u64),
    C: FnMut() -> bool,
{
    let mut context = shared_context.borrow_mut();
    if (context.should_cancel)() {
        bail!("Operation canceled");
    }
    Ok(())
}

fn is_likely_incompressible_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    let file_name = file_name.to_ascii_lowercase();

    [
        ".7z", ".zip", ".rar", ".tar", ".gz", ".tgz", ".bz2", ".bzip2", ".xz", ".txz", ".zst",
        ".tzst", ".jpg", ".jpeg", ".png", ".gif", ".webp", ".avif", ".heic", ".heif", ".mp3",
        ".aac", ".m4a", ".ogg", ".opus", ".flac", ".wav", ".mp4", ".m4v", ".mkv", ".mov", ".avi",
        ".wmv", ".webm", ".apk", ".ipa", ".jar", ".war", ".ear", ".docx", ".xlsx", ".pptx", ".odt",
        ".ods", ".odp", ".epub", ".cbz",
    ]
    .iter()
    .any(|suffix| file_name.ends_with(suffix))
}

fn estimate_dir_size(path: &Path) -> Result<u64> {
    if path.is_file() {
        return Ok(path.metadata()?.len());
    }
    let mut total: u64 = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_file() {
            total += entry.metadata()?.len();
        } else if entry_path.is_dir() {
            total += estimate_dir_size(&entry_path)?;
        }
    }
    Ok(total)
}

fn create_single_file_stream_archive<F>(
    mut output_file: File,
    sources: &[PathBuf],
    options: &CompressionOptions,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    report: &mut CompressionReport,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()>
where
    F: FnMut(u64),
{
    let source = validate_single_file_stream_source(
        sources,
        options.format,
        excluded_paths,
        excluded_outputs,
    )?;
    let input =
        File::open(&source).with_context(|| format!("Failed to read {}", source.display()))?;
    let input_len = input
        .metadata()
        .with_context(|| format!("Failed to inspect {}", source.display()))?
        .len();
    let mut reader = ProgressReader::new(input, progress, should_cancel);

    match options.format {
        CompressionFormat::Gz => {
            if input_len <= GZIP_LIBDEFLATER_MAX_BYTES {
                let mut input_data = Vec::with_capacity(input_len as usize);
                reader
                    .read_to_end(&mut input_data)
                    .with_context(|| format!("Failed to read {}", source.display()))?;
                let mut compressor = create_gzip_compressor(options.level)?;
                let compressed = compress_gzip_bytes(&input_data, &mut compressor)?;
                output_file
                    .write_all(&compressed)
                    .with_context(|| format!("Failed to write {}", source.display()))?;
            } else {
                let mut encoder =
                    GzEncoder::new(output_file, GzCompression::new(options.level.gzip_level()));
                io::copy(&mut reader, &mut encoder)
                    .with_context(|| format!("Failed to compress {}", source.display()))?;
                encoder
                    .finish()
                    .with_context(|| format!("Failed to finalize {}", source.display()))?;
            }
        }
        CompressionFormat::Bz2 => {
            let mut encoder =
                BzEncoder::new(output_file, BzCompression::new(options.level.bzip2_level()));
            io::copy(&mut reader, &mut encoder)
                .with_context(|| format!("Failed to compress {}", source.display()))?;
            encoder
                .finish()
                .with_context(|| format!("Failed to finalize {}", source.display()))?;
        }
        CompressionFormat::Xz => {
            let mut encoder = create_xz_encoder(output_file, options)?;
            io::copy(&mut reader, &mut encoder)
                .with_context(|| format!("Failed to compress {}", source.display()))?;
            encoder
                .finish()
                .with_context(|| format!("Failed to finalize {}", source.display()))?;
        }
        CompressionFormat::Zst => {
            let mut encoder = create_zstd_encoder(output_file, options)?;
            io::copy(&mut reader, &mut encoder)
                .with_context(|| format!("Failed to compress {}", source.display()))?;
            encoder
                .finish()
                .with_context(|| format!("Failed to finalize {}", source.display()))?;
        }
        CompressionFormat::Lz4 => {
            let mut encoder = lz4_flex::frame::FrameEncoder::new(output_file);
            io::copy(&mut reader, &mut encoder)
                .with_context(|| format!("Failed to compress {}", source.display()))?;
            encoder
                .finish()
                .with_context(|| format!("Failed to finalize {}", source.display()))?;
        }
        _ => bail!("Unsupported single-file stream format: {}", options.format),
    }

    report.files_added = 1;
    report.input_bytes = input_len;
    Ok(())
}

fn validate_single_file_stream_source(
    sources: &[PathBuf],
    format: CompressionFormat,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
) -> Result<PathBuf> {
    let mut included_files = Vec::new();

    for source in sources {
        if !source.exists() {
            bail!("Compression source not found: {}", source.display());
        }
        if should_skip_compression_path(source, excluded_paths, excluded_outputs) {
            continue;
        }
        if source.is_dir() {
            bail!(
                "{} only supports a single source file. Use {} for folders or multiple items.",
                format,
                companion_tar_format_label(format)
            );
        }
        included_files.push(source.clone());
    }

    match included_files.as_slice() {
        [source] => Ok(source.clone()),
        [] => bail!("No files or folders were selected for compression"),
        _ => bail!(
            "{} only supports a single source file. Use {} for folders or multiple items.",
            format,
            companion_tar_format_label(format)
        ),
    }
}

fn companion_tar_format_label(format: CompressionFormat) -> &'static str {
    match format {
        CompressionFormat::Gz => "TAR.GZ",
        CompressionFormat::Bz2 => "TAR.BZ2",
        CompressionFormat::Xz => "TAR.XZ",
        _ => "TAR",
    }
}

fn should_skip_compression_path(
    path: &Path,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
) -> bool {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    excluded_paths
        .iter()
        .any(|excluded| canonical == *excluded || canonical.starts_with(excluded))
        || excluded_outputs
            .iter()
            .any(|excluded| canonical == *excluded || canonical.starts_with(excluded))
}

pub(crate) fn create_archive_from_sources<F>(
    sources: &[PathBuf],
    excluded_paths: &[PathBuf],
    output_path: &Path,
    options: CompressionOptions,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<CompressionReport>
where
    F: FnMut(u64),
{
    if sources.is_empty() {
        bail!("No files or folders were selected for compression");
    }
    if options.split_volume_size().is_some() && split_volume_base_path(output_path).is_some() {
        bail!("Choose a base archive name like archive.7z or archive.zip, not a split volume path");
    }
    if options.password().is_some() && !options.format.supports_password_encryption() {
        bail!(
            "{} compression does not support password encryption",
            options.format.label()
        );
    }
    if options.encrypt_file_names && !options.format.supports_encrypt_file_names() {
        bail!(
            "{} compression does not support encrypting file names",
            options.format.label()
        );
    }
    if options.encrypt_file_names && options.password().is_none() {
        bail!("Encrypting file names requires a password");
    }

    let parent = output_path.parent().ok_or_else(|| {
        anyhow!(
            "Cannot determine the parent folder for {}",
            output_path.display()
        )
    })?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "Failed to create archive output directory {}",
            parent.display()
        )
    })?;

    if output_path.is_dir() {
        bail!(
            "Archive output path points to a directory: {}",
            output_path.display()
        );
    }

    let staged_output = temporary_output_path(output_path)?;
    let mut output_file = File::create(&staged_output)
        .with_context(|| format!("Failed to create archive {}", staged_output.display()))?;
    let mut excluded_outputs = vec![
        fs::canonicalize(&staged_output).unwrap_or_else(|_| staged_output.clone()),
        fs::canonicalize(output_path).unwrap_or_else(|_| output_path.to_path_buf()),
    ];
    if options.split_volume_size().is_some() {
        excluded_outputs.extend(collect_existing_split_volume_paths(output_path)?);
    }
    excluded_outputs.sort();
    excluded_outputs.dedup();
    let mut excluded_paths = excluded_paths
        .iter()
        .map(|path| fs::canonicalize(path).unwrap_or_else(|_| path.clone()))
        .collect::<Vec<_>>();
    excluded_paths.sort();
    excluded_paths.dedup();
    let mut report = CompressionReport::new(output_path.to_path_buf());
    let mut seen_entries = HashSet::new();
    let result = (|| -> Result<()> {
        match options.format {
            CompressionFormat::SevenZip => {
                create_sevenz_archive(
                    output_file,
                    sources,
                    &mut report,
                    &options,
                    &excluded_paths,
                    &excluded_outputs,
                    progress,
                    should_cancel,
                )?;
            }
            CompressionFormat::Zip => {
                create_zip_archive(
                    output_file,
                    sources,
                    &mut report,
                    &options,
                    &excluded_paths,
                    &excluded_outputs,
                    progress,
                    should_cancel,
                    &staged_output,
                )?;
            }
            CompressionFormat::Tar => {
                let mut builder = TarBuilder::new(output_file);
                add_sources_to_tar(
                    &mut builder,
                    sources,
                    &mut report,
                    &mut seen_entries,
                    &excluded_paths,
                    &excluded_outputs,
                    progress,
                    should_cancel,
                )?;
                builder.finish().with_context(|| {
                    format!("Failed to finalize archive {}", staged_output.display())
                })?;
            }
            CompressionFormat::TarGz => {
                let total_source_bytes: u64 = sources
                    .iter()
                    .filter_map(|s| estimate_dir_size(s).ok())
                    .sum();
                let has_password = options.password().is_some();
                if total_source_bytes <= GZIP_LIBDEFLATER_MAX_BYTES && !has_password {
                    // Build tar in a temp file, then compress with libdeflater gzip
                    let temp_dir = std::env::temp_dir();
                    let temp_path =
                        temp_dir.join(format!(".fastzip_tar_{}.tmp", std::process::id()));
                    {
                        let temp_file = File::create(&temp_path).with_context(
                            || "Failed to create temporary file for tar.gz compression",
                        )?;
                        let buf = BufWriter::new(temp_file);
                        let mut builder = TarBuilder::new(buf);
                        add_sources_to_tar(
                            &mut builder,
                            sources,
                            &mut report,
                            &mut seen_entries,
                            &excluded_paths,
                            &excluded_outputs,
                            progress,
                            should_cancel,
                        )?;
                        builder
                            .finish()
                            .with_context(|| "Failed to finalize tar stage for gzip compression")?;
                    }
                    let mut tar_data = Vec::new();
                    File::open(&temp_path)
                        .with_context(|| "Failed to open temporary tar for gzip compression")?
                        .read_to_end(&mut tar_data)
                        .with_context(|| "Failed to read temporary tar for gzip compression")?;
                    let _ = fs::remove_file(&temp_path);
                    let mut compressor = create_gzip_compressor(options.level)?;
                    let compressed = compress_gzip_bytes(&tar_data, &mut compressor)?;
                    output_file
                        .write_all(&compressed)
                        .with_context(|| "Failed to write gzip-compressed output")?;
                } else {
                    let encoder =
                        GzEncoder::new(output_file, GzCompression::new(options.level.gzip_level()));
                    let mut builder = TarBuilder::new(encoder);
                    add_sources_to_tar(
                        &mut builder,
                        sources,
                        &mut report,
                        &mut seen_entries,
                        &excluded_paths,
                        &excluded_outputs,
                        progress,
                        should_cancel,
                    )?;
                    builder.finish().with_context(|| {
                        format!("Failed to finalize archive {}", staged_output.display())
                    })?;
                    builder
                        .into_inner()
                        .with_context(|| {
                            format!("Failed to finalize archive {}", staged_output.display())
                        })?
                        .finish()
                        .with_context(|| {
                            format!("Failed to finalize archive {}", staged_output.display())
                        })?;
                }
            }
            CompressionFormat::TarBz2 => {
                let encoder =
                    BzEncoder::new(output_file, BzCompression::new(options.level.bzip2_level()));
                let mut builder = TarBuilder::new(encoder);
                add_sources_to_tar(
                    &mut builder,
                    sources,
                    &mut report,
                    &mut seen_entries,
                    &excluded_paths,
                    &excluded_outputs,
                    progress,
                    should_cancel,
                )?;
                builder.finish().with_context(|| {
                    format!("Failed to finalize archive {}", staged_output.display())
                })?;
                builder
                    .into_inner()
                    .with_context(|| {
                        format!("Failed to finalize archive {}", staged_output.display())
                    })?
                    .finish()
                    .with_context(|| {
                        format!("Failed to finalize archive {}", staged_output.display())
                    })?;
            }
            CompressionFormat::TarXz => {
                let encoder = create_xz_encoder(output_file, &options)?;
                let mut builder = TarBuilder::new(encoder);
                add_sources_to_tar(
                    &mut builder,
                    sources,
                    &mut report,
                    &mut seen_entries,
                    &excluded_paths,
                    &excluded_outputs,
                    progress,
                    should_cancel,
                )?;
                builder.finish().with_context(|| {
                    format!("Failed to finalize archive {}", staged_output.display())
                })?;
                builder
                    .into_inner()
                    .with_context(|| {
                        format!("Failed to finalize archive {}", staged_output.display())
                    })?
                    .finish()
                    .with_context(|| {
                        format!("Failed to finalize archive {}", staged_output.display())
                    })?;
            }
            CompressionFormat::TarZst => {
                create_tar_zst_archive(
                    output_file,
                    sources,
                    &mut report,
                    &options,
                    &excluded_paths,
                    &excluded_outputs,
                    progress,
                    should_cancel,
                )?;
            }
            CompressionFormat::TarLz4 => {
                create_tar_lz4_archive(
                    output_file,
                    sources,
                    &mut report,
                    &options,
                    &excluded_paths,
                    &excluded_outputs,
                    progress,
                    should_cancel,
                )?;
            }
            CompressionFormat::Gz
            | CompressionFormat::Bz2
            | CompressionFormat::Xz
            | CompressionFormat::Zst
            | CompressionFormat::Lz4 => {
                create_single_file_stream_archive(
                    output_file,
                    sources,
                    &options,
                    &excluded_paths,
                    &excluded_outputs,
                    &mut report,
                    progress,
                    should_cancel,
                )?;
            }
        }
        ensure_not_canceled(should_cancel)?;
        let committed_output = commit_staged_archive_output(
            &staged_output,
            output_path,
            options.split_volume_size(),
            should_cancel,
        )?;
        report.archive_path = committed_output.archive_path;
        report.output_bytes = committed_output.output_bytes;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&staged_output);
    }

    result.map(|_| report)
}

fn create_zip_archive<F>(
    output_file: File,
    sources: &[PathBuf],
    report: &mut CompressionReport,
    options: &CompressionOptions,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
    staged_output: &Path,
) -> Result<()>
where
    F: FnMut(u64),
{
    let mut prepared_directories = Vec::new();
    let mut prepared_files = Vec::new();
    let mut collected_entries = HashSet::new();
    let password_encryption_enabled = options.password().is_some();

    for source in sources {
        ensure_not_canceled(should_cancel)?;
        if !source.exists() {
            bail!("Compression source not found: {}", source.display());
        }

        let root_name = source
            .file_name()
            .ok_or_else(|| anyhow!("Failed to derive an archive name from {}", source.display()))?;
        collect_zip_entries(
            source,
            &PathBuf::from(root_name),
            options,
            &mut prepared_directories,
            &mut prepared_files,
            &mut collected_entries,
            excluded_paths,
            excluded_outputs,
            should_cancel,
        )?;
    }

    if !password_encryption_enabled
        && should_use_parallel_deflate_single_file(options, &prepared_files)
    {
        write_parallel_deflate_single_file_zip_archive(
            output_file,
            &prepared_directories,
            &prepared_files[0],
            report,
            options,
            progress,
            should_cancel,
            staged_output,
        )?;
        return Ok(());
    }

    let strategy = if password_encryption_enabled {
        ZipCompressionStrategy::DirectSerial
    } else {
        zip_compression_strategy(options, &prepared_files)
    };
    if !password_encryption_enabled {
        if let ZipCompressionStrategy::ParallelWorkers(plan) = strategy {
            if let Some(parallel_direct_plan) =
                parallel_direct_zip_worker_plan(options, &prepared_files, plan)
            {
                write_parallel_deflate_multi_file_zip_archive(
                    output_file,
                    &prepared_directories,
                    &prepared_files,
                    report,
                    options,
                    progress,
                    should_cancel,
                    staged_output,
                    parallel_direct_plan,
                )?;
                return Ok(());
            }
        }
    }

    if !password_encryption_enabled
        && matches!(strategy, ZipCompressionStrategy::DirectSerial)
        && should_use_direct_serial_zip_archive(options, &prepared_files)
    {
        write_serial_direct_zip_archive(
            output_file,
            &prepared_directories,
            &prepared_files,
            report,
            options,
            progress,
            should_cancel,
            staged_output,
        )?;
        return Ok(());
    }

    let mut writer = zip::ZipWriter::new(BufWriter::with_capacity(1024 * 1024, output_file));
    let mut writer_seen_entries = HashSet::new();

    for directory in &prepared_directories {
        add_zip_directory(
            &mut writer,
            Path::new(&directory.archive_path),
            options,
            report,
            &mut writer_seen_entries,
        )?;
    }

    if matches!(strategy, ZipCompressionStrategy::DirectSerial) {
        for file in &prepared_files {
            add_zip_file(
                &mut writer,
                &file.source_path,
                Path::new(&file.archive_path),
                options,
                file.zip_method,
                report,
                &mut writer_seen_entries,
                progress,
                should_cancel,
            )?;
        }

        let mut buffered_output = writer
            .finish()
            .with_context(|| format!("Failed to finalize archive {}", staged_output.display()))?;
        buffered_output
            .flush()
            .with_context(|| format!("Failed to flush archive {}", staged_output.display()))?;
        drop(buffered_output);
        return Ok(());
    }

    report.files_added = prepared_files.len();
    report.input_bytes = prepared_files.iter().map(|file| file.input_len).sum();

    let parallel_plan = match strategy {
        ZipCompressionStrategy::DirectSerial => unreachable!("serial strategy handled above"),
        ZipCompressionStrategy::ParallelWorkers(plan) => plan,
    };
    let worker_chunks = split_prepared_zip_files(prepared_files, parallel_plan.task_count);
    let cancel_requested = Arc::new(AtomicBool::new(false));
    let (progress_tx, progress_rx) = mpsc::channel();
    let mut worker_tasks = VecDeque::with_capacity(worker_chunks.len());
    let mut worker_paths = Vec::with_capacity(worker_chunks.len());
    let mut worker_handles = Vec::with_capacity(worker_chunks.len());

    for (task_index, chunk) in worker_chunks.into_iter().enumerate() {
        let worker_path = temporary_parallel_zip_path(staged_output, task_index)?;
        worker_paths.push(worker_path);
        worker_tasks.push_back(ZipWorkerTask {
            worker_path: worker_paths[task_index].clone(),
            chunk,
        });
    }

    let shared_worker_tasks = Arc::new(Mutex::new(worker_tasks));

    for _ in 0..parallel_plan.worker_count {
        let worker_options = options.clone();
        let worker_progress_tx = progress_tx.clone();
        let worker_cancel_requested = Arc::clone(&cancel_requested);
        let worker_tasks = Arc::clone(&shared_worker_tasks);
        worker_handles.push(Some(thread::spawn(move || -> Result<()> {
            loop {
                if worker_cancel_requested.load(Ordering::Relaxed) {
                    break;
                }

                let task = {
                    let mut pending_tasks = worker_tasks
                        .lock()
                        .map_err(|_| anyhow!("ZIP worker task queue was poisoned"))?;
                    pending_tasks.pop_front()
                };
                let Some(task) = task else {
                    break;
                };

                if worker_cancel_requested.load(Ordering::Relaxed) {
                    break;
                }

                write_parallel_zip_worker_archive(
                    task.worker_path,
                    task.chunk,
                    &worker_options,
                    worker_progress_tx.clone(),
                    Arc::clone(&worker_cancel_requested),
                )?;
            }

            Ok(())
        })));
    }
    drop(progress_tx);

    let result = (|| -> Result<()> {
        let mut remaining_workers = worker_handles.len();
        let mut first_error = None;

        while remaining_workers > 0 {
            if should_cancel() {
                cancel_requested.store(true, Ordering::Relaxed);
            }

            while let Ok(delta) = progress_rx.try_recv() {
                progress(delta);
            }

            let mut joined_worker = false;
            for handle in &mut worker_handles {
                let Some(join_handle) = handle.as_ref() else {
                    continue;
                };
                if !join_handle.is_finished() {
                    continue;
                }

                let join_handle = handle.take().unwrap();
                remaining_workers -= 1;
                joined_worker = true;

                match join_handle.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                        cancel_requested.store(true, Ordering::Relaxed);
                    }
                    Err(_) => {
                        if first_error.is_none() {
                            first_error = Some(anyhow!("ZIP worker thread panicked"));
                        }
                        cancel_requested.store(true, Ordering::Relaxed);
                    }
                }
            }

            if remaining_workers == 0 {
                break;
            }

            if !joined_worker {
                match progress_rx.recv_timeout(Duration::from_millis(25)) {
                    Ok(delta) => progress(delta),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {}
                }
            }
        }

        while let Ok(delta) = progress_rx.try_recv() {
            progress(delta);
        }

        if cancel_requested.load(Ordering::Relaxed) && first_error.is_none() {
            bail!("Operation canceled");
        }
        if let Some(error) = first_error {
            return Err(error);
        }

        for worker_path in &worker_paths {
            ensure_not_canceled(should_cancel)?;
            let file = File::open(worker_path)
                .with_context(|| format!("Failed to open worker ZIP {}", worker_path.display()))?;
            let mut archive = ZipArchive::new(file).with_context(|| {
                format!(
                    "Failed to open worker ZIP archive {}",
                    worker_path.display()
                )
            })?;

            for index in 0..archive.len() {
                ensure_not_canceled(should_cancel)?;
                let entry = archive.by_index(index).with_context(|| {
                    format!(
                        "Failed to inspect worker ZIP entry #{index} in {}",
                        worker_path.display()
                    )
                })?;
                writer.raw_copy_file(entry).with_context(|| {
                    format!(
                        "Failed to merge worker ZIP entry #{index} from {}",
                        worker_path.display()
                    )
                })?;
            }
        }

        let mut buffered_output = writer
            .finish()
            .with_context(|| format!("Failed to finalize archive {}", staged_output.display()))?;
        buffered_output
            .flush()
            .with_context(|| format!("Failed to flush archive {}", staged_output.display()))?;
        drop(buffered_output);
        Ok(())
    })();

    for worker_path in worker_paths {
        let _ = fs::remove_file(worker_path);
    }

    result
}

fn collect_zip_entries(
    source: &Path,
    archive_path: &Path,
    options: &CompressionOptions,
    prepared_directories: &mut Vec<PreparedZipDirectory>,
    prepared_files: &mut Vec<PreparedZipFile>,
    seen_entries: &mut HashSet<String>,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()> {
    let source_canonical = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    collect_zip_entries_with_canonical(
        source,
        &source_canonical,
        archive_path,
        options,
        prepared_directories,
        prepared_files,
        seen_entries,
        excluded_paths,
        excluded_outputs,
        should_cancel,
    )
}

fn collect_zip_entries_with_canonical(
    source: &Path,
    source_canonical: &Path,
    archive_path: &Path,
    options: &CompressionOptions,
    prepared_directories: &mut Vec<PreparedZipDirectory>,
    prepared_files: &mut Vec<PreparedZipFile>,
    seen_entries: &mut HashSet<String>,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()> {
    ensure_not_canceled(should_cancel)?;
    if excluded_paths
        .iter()
        .any(|path| source_canonical == path || source_canonical.starts_with(path))
        || excluded_outputs
            .iter()
            .any(|path| source_canonical == path || source_canonical.starts_with(path))
    {
        return Ok(());
    }

    if source.is_dir() {
        let entry_name = normalize_zip_directory_path(archive_path)?;
        if seen_entries.insert(format!("dir:{entry_name}")) {
            prepared_directories.push(PreparedZipDirectory {
                archive_path: entry_name,
            });
        }

        let mut children = fs::read_dir(source)
            .with_context(|| format!("Failed to enumerate {}", source.display()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("Failed to read {}", source.display()))?;
        children.sort_by_key(|entry| entry.path());

        for child in children {
            let child_path = child.path();
            let child_name = child.file_name();
            let child_canonical = match child.file_type() {
                Ok(file_type) if !file_type.is_symlink() => source_canonical.join(&child_name),
                _ => fs::canonicalize(&child_path).unwrap_or_else(|_| child_path.clone()),
            };
            if excluded_paths
                .iter()
                .any(|path| child_canonical == *path || child_canonical.starts_with(path))
            {
                continue;
            }
            if excluded_outputs.iter().any(|path| path == &child_canonical) {
                continue;
            }

            collect_zip_entries_with_canonical(
                &child_path,
                &child_canonical,
                &archive_path.join(child_name),
                options,
                prepared_directories,
                prepared_files,
                seen_entries,
                excluded_paths,
                excluded_outputs,
                should_cancel,
            )?;
        }
        return Ok(());
    }

    if !source.is_file() {
        bail!("Unsupported compression source type: {}", source.display());
    }

    let entry_name = normalize_zip_file_path(archive_path)?;
    if !seen_entries.insert(format!("file:{entry_name}")) {
        bail!("Duplicate archive entry name: {entry_name}");
    }

    let input_len = source
        .metadata()
        .with_context(|| format!("Failed to inspect {}", source.display()))?
        .len();
    prepared_files.push(PreparedZipFile {
        source_path: source.to_path_buf(),
        archive_path: entry_name,
        archive_index: prepared_files.len(),
        input_len,
        zip_method: effective_zip_method_for_path(source, options),
    });
    Ok(())
}

fn write_parallel_zip_worker_archive(
    worker_path: PathBuf,
    chunk: ZipWorkerChunk,
    options: &CompressionOptions,
    progress_tx: mpsc::Sender<u64>,
    cancel_requested: Arc<AtomicBool>,
) -> Result<()> {
    let output_file = File::create(&worker_path)
        .with_context(|| format!("Failed to create worker ZIP {}", worker_path.display()))?;
    let mut writer = zip::ZipWriter::new(BufWriter::with_capacity(1024 * 1024, output_file));
    let mut report = CompressionReport::new(worker_path.clone());
    let mut seen_entries = HashSet::new();
    let mut pending_progress = 0u64;
    let mut progress = |delta: u64| {
        pending_progress = pending_progress.saturating_add(delta);
        if pending_progress >= ZIP_PROGRESS_EMIT_BYTES {
            let _ = progress_tx.send(pending_progress);
            pending_progress = 0;
        }
    };
    let mut should_cancel = || cancel_requested.load(Ordering::Relaxed);

    let result = (|| -> Result<()> {
        for file in &chunk.files {
            add_zip_file(
                &mut writer,
                &file.source_path,
                Path::new(&file.archive_path),
                &options,
                file.zip_method,
                &mut report,
                &mut seen_entries,
                &mut progress,
                &mut should_cancel,
            )?;
        }

        let mut buffered_output = writer
            .finish()
            .with_context(|| format!("Failed to finalize worker ZIP {}", worker_path.display()))?;
        buffered_output
            .flush()
            .with_context(|| format!("Failed to flush worker ZIP {}", worker_path.display()))?;
        drop(buffered_output);
        Ok(())
    })();

    if pending_progress > 0 {
        let _ = progress_tx.send(pending_progress);
    }

    if result.is_err() {
        let _ = fs::remove_file(&worker_path);
    }

    result
}

fn zip_compression_strategy(
    options: &CompressionOptions,
    prepared_files: &[PreparedZipFile],
) -> ZipCompressionStrategy {
    let file_count = prepared_files.len();
    let total_bytes: u64 = prepared_files.iter().map(|file| file.input_len).sum();
    let requested_threads = options.thread_count.max(1) as usize;

    if requested_threads <= 1 || file_count <= 1 {
        return ZipCompressionStrategy::DirectSerial;
    }

    let (
        worker_limit,
        minimum_parallel_bytes,
        minimum_worker_bytes,
        task_multiplier,
        minimum_task_bytes,
    ) = match options.zip_method {
        ZipCompressionMethod::Stored => return ZipCompressionStrategy::DirectSerial,
        ZipCompressionMethod::Deflate => (
            requested_threads,
            1024 * 1024,
            1024 * 1024,
            3usize,
            1024 * 1024,
        ),
        ZipCompressionMethod::Zstd => (
            requested_threads.min(16),
            16 * 1024 * 1024,
            16 * 1024 * 1024,
            2usize,
            16 * 1024 * 1024,
        ),
        ZipCompressionMethod::Bzip2 => (
            requested_threads.min(8),
            16 * 1024 * 1024,
            16 * 1024 * 1024,
            2usize,
            16 * 1024 * 1024,
        ),
        ZipCompressionMethod::Xz => (
            requested_threads.min(4),
            32 * 1024 * 1024,
            32 * 1024 * 1024,
            2usize,
            32 * 1024 * 1024,
        ),
    };

    if total_bytes < minimum_parallel_bytes {
        return ZipCompressionStrategy::DirectSerial;
    }

    let byte_limited_workers = ceil_div_u64(total_bytes, minimum_worker_bytes) as usize;
    let worker_count = worker_limit
        .min(file_count)
        .min(byte_limited_workers.max(1));
    if worker_count <= 1 {
        ZipCompressionStrategy::DirectSerial
    } else {
        let byte_limited_tasks = ceil_div_u64(total_bytes, minimum_task_bytes) as usize;
        let task_count = worker_count
            .saturating_mul(task_multiplier)
            .min(file_count)
            .min(byte_limited_tasks.max(worker_count));
        ZipCompressionStrategy::ParallelWorkers(ZipParallelPlan {
            worker_count,
            task_count,
        })
    }
}

fn split_prepared_zip_files(
    prepared_files: Vec<PreparedZipFile>,
    task_count: usize,
) -> Vec<ZipWorkerChunk> {
    let task_count = task_count.max(1).min(prepared_files.len());
    if task_count <= 1 {
        let total_bytes = prepared_files.iter().map(|file| file.input_len).sum();
        return vec![ZipWorkerChunk {
            files: prepared_files,
            total_bytes,
        }];
    }

    let mut files = prepared_files;
    files.sort_by(|left, right| {
        right
            .input_len
            .cmp(&left.input_len)
            .then_with(|| left.archive_index.cmp(&right.archive_index))
    });

    let mut chunks = (0..task_count)
        .map(|_| ZipWorkerChunk {
            files: Vec::new(),
            total_bytes: 0,
        })
        .collect::<Vec<_>>();

    for file in files {
        let target_index = chunks
            .iter()
            .enumerate()
            .min_by_key(|(_, chunk)| (chunk.total_bytes, chunk.files.len()))
            .map(|(index, _)| index)
            .unwrap_or(0);
        let chunk = &mut chunks[target_index];
        chunk.total_bytes = chunk.total_bytes.saturating_add(file.input_len.max(1));
        chunk.files.push(file);
    }

    chunks.retain(|chunk| !chunk.files.is_empty());
    for chunk in &mut chunks {
        chunk.files.sort_by_key(|file| file.archive_index);
    }
    chunks.sort_by_key(|chunk| chunk.files[0].archive_index);

    chunks
}

fn temporary_parallel_zip_path(staged_output: &Path, worker_index: usize) -> Result<PathBuf> {
    let file_name = staged_output
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Invalid staged ZIP file name: {}", staged_output.display()))?;
    let worker_output =
        staged_output.with_file_name(format!("{file_name}.zip-worker-{worker_index}.zip"));
    temporary_output_path(&worker_output)
}

fn parallel_direct_zip_worker_plan(
    options: &CompressionOptions,
    prepared_files: &[PreparedZipFile],
    plan: ZipParallelPlan,
) -> Option<ZipParallelPlan> {
    if matches!(options.zip_method, ZipCompressionMethod::Stored) {
        return None;
    }
    if prepared_files.len() <= 1 || plan.worker_count <= 1 {
        return None;
    }
    if prepared_files.iter().any(|file| {
        !matches!(
            file.zip_method,
            ZipCompressionMethod::Deflate
                | ZipCompressionMethod::Stored
                | ZipCompressionMethod::Zstd
                | ZipCompressionMethod::Bzip2
                | ZipCompressionMethod::Xz
        )
    }) {
        return None;
    }
    let has_stored_entries = prepared_files
        .iter()
        .any(|file| file.zip_method == ZipCompressionMethod::Stored);
    if has_stored_entries {
        let total_bytes: u64 = prepared_files.iter().map(|file| file.input_len).sum();
        if total_bytes > ZIP_PARALLEL_DIRECT_MIXED_MAX_TOTAL_BYTES
            || prepared_files.len() > ZIP_PARALLEL_DIRECT_MIXED_MAX_FILE_COUNT
        {
            return None;
        }
    }

    let max_file_bytes = prepared_files
        .iter()
        .map(|file| file.input_len)
        .max()
        .unwrap_or(0);
    if max_file_bytes == 0 || max_file_bytes > ZIP_PARALLEL_DIRECT_DEFLATE_MAX_FILE_BYTES {
        return None;
    }

    let budget_workers = ceil_div_u64(
        ZIP_PARALLEL_DIRECT_DEFLATE_MAX_INFLIGHT_INPUT_BYTES,
        max_file_bytes,
    ) as usize;
    let worker_count = plan
        .worker_count
        .min(prepared_files.len())
        .min(budget_workers.max(1));
    if worker_count <= 1 {
        return None;
    }

    let task_count = plan.task_count.max(worker_count).min(prepared_files.len());
    Some(ZipParallelPlan {
        worker_count,
        task_count,
    })
}

fn should_use_direct_serial_zip_archive(
    options: &CompressionOptions,
    prepared_files: &[PreparedZipFile],
) -> bool {
    matches!(
        options.zip_method,
        ZipCompressionMethod::Stored | ZipCompressionMethod::Deflate
    ) && !prepared_files.is_empty()
        && prepared_files.iter().all(|file| {
            matches!(
                file.zip_method,
                ZipCompressionMethod::Stored | ZipCompressionMethod::Deflate
            )
        })
        && prepared_files
            .iter()
            .map(|file| file.input_len)
            .max()
            .unwrap_or(0)
            <= ZIP_PARALLEL_DIRECT_DEFLATE_MAX_FILE_BYTES
}

fn should_use_parallel_deflate_single_file(
    options: &CompressionOptions,
    prepared_files: &[PreparedZipFile],
) -> bool {
    matches!(options.zip_method, ZipCompressionMethod::Deflate)
        && options.thread_count > 1
        && prepared_files.len() == 1
        && prepared_files[0].zip_method == ZipCompressionMethod::Deflate
        && prepared_files[0].input_len >= ZIP_PARALLEL_DEFLATE_MIN_FILE_BYTES
}

fn write_parallel_deflate_multi_file_zip_archive<F>(
    output: File,
    directories: &[PreparedZipDirectory],
    prepared_files: &[PreparedZipFile],
    report: &mut CompressionReport,
    options: &CompressionOptions,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
    staged_output: &Path,
    plan: ZipParallelPlan,
) -> Result<()>
where
    F: FnMut(u64),
{
    let directory_layout = stored_directory_zip_header_layout();
    let mut output = BufWriter::with_capacity(4 * 1024 * 1024, output);
    let mut central_directory_entries =
        Vec::with_capacity(directories.len() + prepared_files.len());
    let mut current_output_offset = 0u64;

    for directory in directories {
        ensure_not_canceled(should_cancel)?;
        let entry_name = directory.archive_path.as_bytes().to_vec();
        let local_header_offset = current_output_offset;
        write_single_entry_zip_local_header(&mut output, &directory_layout, &entry_name, 0, 0, 0)?;
        current_output_offset = current_output_offset
            .checked_add(zip_local_file_record_len(
                &directory_layout,
                entry_name.len(),
                0,
            ))
            .ok_or_else(|| anyhow!("ZIP output offset overflowed"))?;
        central_directory_entries.push(ManualZipCentralDirectoryEntry {
            header_layout: directory_layout,
            entry_name,
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            local_header_offset,
            external_attributes: (ZIP_DIRECTORY_UNIX_MODE << 16) | ZIP_DIRECTORY_DOS_ATTRIBUTE,
        });
    }

    let cancel_requested = Arc::new(AtomicBool::new(false));
    let worker_chunks = split_prepared_zip_files(prepared_files.to_vec(), plan.task_count);
    let shared_tasks = Arc::new(Mutex::new(VecDeque::from(worker_chunks)));
    let (progress_tx, progress_rx) = mpsc::channel();
    let result_channel_capacity = plan
        .worker_count
        .saturating_mul(4)
        .max(1)
        .min(prepared_files.len().max(1));
    let (result_tx, result_rx) = mpsc::sync_channel(result_channel_capacity);
    let mut worker_handles = Vec::with_capacity(plan.worker_count);

    for _ in 0..plan.worker_count {
        let shared_tasks = Arc::clone(&shared_tasks);
        let progress_tx = progress_tx.clone();
        let result_tx = result_tx.clone();
        let cancel_requested = Arc::clone(&cancel_requested);
        let level = options.level;
        worker_handles.push(Some(thread::spawn(move || -> Result<()> {
            loop {
                if cancel_requested.load(Ordering::Relaxed) {
                    break;
                }

                let task = {
                    let mut pending_tasks = shared_tasks
                        .lock()
                        .map_err(|_| anyhow!("ZIP direct worker task queue was poisoned"))?;
                    pending_tasks.pop_front()
                };
                let Some(chunk) = task else {
                    break;
                };

                let result = compress_parallel_zip_file_chunk(
                    chunk,
                    level,
                    &progress_tx,
                    &result_tx,
                    Arc::clone(&cancel_requested),
                );
                if let Err(error) = result {
                    let _ = result_tx.send(Err(error));
                    cancel_requested.store(true, Ordering::Relaxed);
                    break;
                }
            }

            Ok(())
        })));
    }
    drop(progress_tx);
    drop(result_tx);

    let mut pending_entries = BTreeMap::new();
    let mut expected_archive_index = prepared_files
        .first()
        .map(|file| file.archive_index)
        .unwrap_or(0);
    let mut remaining_workers = worker_handles.len();
    let mut written_files = 0usize;
    let mut first_error = None;

    while remaining_workers > 0 {
        if should_cancel() {
            cancel_requested.store(true, Ordering::Relaxed);
        }

        while let Ok(delta) = progress_rx.try_recv() {
            progress(delta);
        }

        match result_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(result) => {
                handle_parallel_zip_file_entry_result(
                    result,
                    &mut pending_entries,
                    &mut expected_archive_index,
                    &mut central_directory_entries,
                    &mut current_output_offset,
                    &mut written_files,
                    &mut output,
                    &mut first_error,
                    &cancel_requested,
                )?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }

        for handle in &mut worker_handles {
            let Some(join_handle) = handle.as_ref() else {
                continue;
            };
            if !join_handle.is_finished() {
                continue;
            }

            let join_handle = handle.take().unwrap();
            remaining_workers -= 1;

            match join_handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    cancel_requested.store(true, Ordering::Relaxed);
                }
                Err(_) => {
                    if first_error.is_none() {
                        first_error = Some(anyhow!("ZIP direct worker thread panicked"));
                    }
                    cancel_requested.store(true, Ordering::Relaxed);
                }
            }
        }
    }

    while let Ok(delta) = progress_rx.try_recv() {
        progress(delta);
    }
    while let Ok(result) = result_rx.try_recv() {
        handle_parallel_zip_file_entry_result(
            result,
            &mut pending_entries,
            &mut expected_archive_index,
            &mut central_directory_entries,
            &mut current_output_offset,
            &mut written_files,
            &mut output,
            &mut first_error,
            &cancel_requested,
        )?;
    }

    if cancel_requested.load(Ordering::Relaxed) && first_error.is_none() {
        bail!("Operation canceled");
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    if written_files != prepared_files.len() {
        bail!("Parallel ZIP deflate writer did not produce every file entry");
    }

    write_manual_zip_central_directory_and_end(&mut output, &central_directory_entries)?;
    output
        .flush()
        .with_context(|| format!("Failed to flush archive {}", staged_output.display()))?;
    report.files_added = prepared_files.len();
    report.directories_added = directories.len();
    report.input_bytes = prepared_files.iter().map(|file| file.input_len).sum();
    Ok(())
}

fn write_serial_direct_zip_archive<F>(
    output: File,
    directories: &[PreparedZipDirectory],
    prepared_files: &[PreparedZipFile],
    report: &mut CompressionReport,
    options: &CompressionOptions,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
    staged_output: &Path,
) -> Result<()>
where
    F: FnMut(u64),
{
    let directory_layout = stored_directory_zip_header_layout();
    let mut output = BufWriter::with_capacity(1024 * 1024, output);
    let mut central_directory_entries =
        Vec::with_capacity(directories.len() + prepared_files.len());
    let mut current_output_offset = 0u64;

    for directory in directories {
        ensure_not_canceled(should_cancel)?;
        let entry_name = directory.archive_path.as_bytes().to_vec();
        let local_header_offset = current_output_offset;
        write_single_entry_zip_local_header(&mut output, &directory_layout, &entry_name, 0, 0, 0)?;
        current_output_offset = current_output_offset
            .checked_add(zip_local_file_record_len(
                &directory_layout,
                entry_name.len(),
                0,
            ))
            .ok_or_else(|| anyhow!("ZIP output offset overflowed"))?;
        central_directory_entries.push(ManualZipCentralDirectoryEntry {
            header_layout: directory_layout,
            entry_name,
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            local_header_offset,
            external_attributes: (ZIP_DIRECTORY_UNIX_MODE << 16) | ZIP_DIRECTORY_DOS_ATTRIBUTE,
        });
        report.directories_added += 1;
    }

    let mut reusable_input = Vec::new();
    let mut deflate_compressor = create_zip_whole_buffer_deflate_compressor(options.level)?;

    for file in prepared_files {
        ensure_not_canceled(should_cancel)?;
        let entry_name = file.archive_path.as_bytes().to_vec();
        let crc32 = read_zip_file_bytes_into(
            &file.source_path,
            file.input_len,
            progress,
            should_cancel,
            &mut reusable_input,
        )?;

        let (header_layout, compressed) = match file.zip_method {
            ZipCompressionMethod::Stored => {
                let input_capacity = reusable_input.capacity();
                let input_bytes =
                    std::mem::replace(&mut reusable_input, Vec::with_capacity(input_capacity));
                (stored_file_zip_header_layout(file.input_len), input_bytes)
            }
            ZipCompressionMethod::Deflate => (
                deflate_zip_header_layout(file.input_len),
                compress_zip_whole_buffer_deflate_bytes(&reusable_input, &mut deflate_compressor)?,
            ),
            ZipCompressionMethod::Zstd => (
                single_entry_zip_header_layout(file.input_len, ZIP_ZSTD_METHOD_ID),
                compress_zstd_bytes(&reusable_input, options.level)?,
            ),
            ZipCompressionMethod::Bzip2 => (
                single_entry_zip_header_layout(file.input_len, ZIP_BZIP2_METHOD_ID),
                compress_bzip2_bytes(&reusable_input, options.level)?,
            ),
            ZipCompressionMethod::Xz => (
                single_entry_zip_header_layout(file.input_len, ZIP_XZ_METHOD_ID),
                compress_xz_bytes(&reusable_input, options.level)?,
            ),
        };
        let compressed_size = compressed.len() as u64;
        reusable_input.clear();

        write_manual_zip_file_entry(
            &mut output,
            ParallelZipFileEntryOutput {
                archive_index: file.archive_index,
                header_layout,
                entry_name,
                compressed,
                crc32,
                compressed_size,
                uncompressed_size: file.input_len,
                external_attributes: ZIP_REGULAR_FILE_UNIX_MODE << 16,
            },
            &mut central_directory_entries,
            &mut current_output_offset,
        )?;
        report.files_added += 1;
        report.input_bytes = report.input_bytes.saturating_add(file.input_len);
    }

    write_manual_zip_central_directory_and_end(&mut output, &central_directory_entries)?;
    output
        .flush()
        .with_context(|| format!("Failed to flush archive {}", staged_output.display()))?;
    Ok(())
}

fn handle_parallel_zip_file_entry_result<W>(
    result: Result<ParallelZipFileEntryOutput>,
    pending_entries: &mut BTreeMap<usize, ParallelZipFileEntryOutput>,
    expected_archive_index: &mut usize,
    central_directory_entries: &mut Vec<ManualZipCentralDirectoryEntry>,
    current_output_offset: &mut u64,
    written_files: &mut usize,
    output: &mut W,
    first_error: &mut Option<anyhow::Error>,
    cancel_requested: &Arc<AtomicBool>,
) -> Result<()>
where
    W: Write + Seek,
{
    match result {
        Ok(entry) => {
            pending_entries.insert(entry.archive_index, entry);
            flush_parallel_zip_file_entries(
                output,
                pending_entries,
                expected_archive_index,
                central_directory_entries,
                current_output_offset,
                written_files,
            )?;
        }
        Err(error) => {
            if first_error.is_none() {
                *first_error = Some(error);
            }
            cancel_requested.store(true, Ordering::Relaxed);
        }
    }

    Ok(())
}

fn flush_parallel_zip_file_entries<W>(
    output: &mut W,
    pending_entries: &mut BTreeMap<usize, ParallelZipFileEntryOutput>,
    expected_archive_index: &mut usize,
    central_directory_entries: &mut Vec<ManualZipCentralDirectoryEntry>,
    current_output_offset: &mut u64,
    written_files: &mut usize,
) -> Result<()>
where
    W: Write + Seek,
{
    while let Some(entry) = pending_entries.remove(expected_archive_index) {
        write_manual_zip_file_entry(
            output,
            entry,
            central_directory_entries,
            current_output_offset,
        )?;
        *expected_archive_index += 1;
        *written_files += 1;
    }

    Ok(())
}

fn write_manual_zip_file_entry<W>(
    output: &mut W,
    entry: ParallelZipFileEntryOutput,
    central_directory_entries: &mut Vec<ManualZipCentralDirectoryEntry>,
    current_output_offset: &mut u64,
) -> Result<()>
where
    W: Write + Seek,
{
    let local_header_offset = *current_output_offset;
    write_single_entry_zip_local_header(
        output,
        &entry.header_layout,
        &entry.entry_name,
        entry.crc32,
        entry.uncompressed_size,
        entry.compressed_size,
    )?;
    output.write_all(&entry.compressed)?;
    *current_output_offset = current_output_offset
        .checked_add(zip_local_file_record_len(
            &entry.header_layout,
            entry.entry_name.len(),
            entry.compressed_size,
        ))
        .ok_or_else(|| anyhow!("ZIP output offset overflowed"))?;
    central_directory_entries.push(ManualZipCentralDirectoryEntry {
        header_layout: entry.header_layout,
        entry_name: entry.entry_name,
        crc32: entry.crc32,
        compressed_size: entry.compressed_size,
        uncompressed_size: entry.uncompressed_size,
        local_header_offset,
        external_attributes: entry.external_attributes,
    });
    Ok(())
}

fn compress_parallel_zip_file_chunk(
    chunk: ZipWorkerChunk,
    level: CompressionLevel,
    progress_tx: &mpsc::Sender<u64>,
    result_tx: &mpsc::SyncSender<Result<ParallelZipFileEntryOutput>>,
    cancel_requested: Arc<AtomicBool>,
) -> Result<()> {
    let mut pending_progress = 0u64;
    let mut deflate_compressor = create_zip_whole_buffer_deflate_compressor(level)?;

    for file in chunk.files {
        if cancel_requested.load(Ordering::Relaxed) {
            bail!("Operation canceled");
        }

        let entry_name = file.archive_path.as_bytes().to_vec();
        let input = read_parallel_zip_file_bytes(
            &file.source_path,
            file.input_len,
            progress_tx,
            Arc::clone(&cancel_requested),
            &mut pending_progress,
        )?;

        let (header_layout, compressed) = match file.zip_method {
            ZipCompressionMethod::Stored => {
                (stored_file_zip_header_layout(file.input_len), input.bytes)
            }
            ZipCompressionMethod::Deflate => (
                deflate_zip_header_layout(file.input_len),
                compress_zip_whole_buffer_deflate_bytes(&input.bytes, &mut deflate_compressor)?,
            ),
            ZipCompressionMethod::Zstd => (
                single_entry_zip_header_layout(file.input_len, ZIP_ZSTD_METHOD_ID),
                compress_zstd_bytes(&input.bytes, level)?,
            ),
            ZipCompressionMethod::Bzip2 => (
                single_entry_zip_header_layout(file.input_len, ZIP_BZIP2_METHOD_ID),
                compress_bzip2_bytes(&input.bytes, level)?,
            ),
            ZipCompressionMethod::Xz => (
                single_entry_zip_header_layout(file.input_len, ZIP_XZ_METHOD_ID),
                compress_xz_bytes(&input.bytes, level)?,
            ),
        };
        let compressed_size = compressed.len() as u64;

        result_tx
            .send(Ok(ParallelZipFileEntryOutput {
                archive_index: file.archive_index,
                header_layout,
                entry_name,
                compressed,
                crc32: input.crc32,
                compressed_size,
                uncompressed_size: file.input_len,
                external_attributes: ZIP_REGULAR_FILE_UNIX_MODE << 16,
            }))
            .map_err(|_| anyhow!("ZIP direct writer result channel disconnected"))?;
    }

    flush_parallel_zip_progress(progress_tx, &mut pending_progress);
    Ok(())
}

fn read_parallel_zip_file_bytes(
    source_path: &Path,
    expected_len: u64,
    progress_tx: &mpsc::Sender<u64>,
    cancel_requested: Arc<AtomicBool>,
    pending_progress: &mut u64,
) -> Result<ParallelZipFileInput> {
    let mut input = File::open(source_path)
        .with_context(|| format!("Failed to open {}", source_path.display()))?;
    let capacity = usize::try_from(expected_len)
        .context("ZIP direct worker input exceeded addressable memory")?;
    let mut bytes = vec![0u8; capacity];
    let mut crc = Crc32Hasher::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        if cancel_requested.load(Ordering::Relaxed) {
            bail!("Operation canceled");
        }

        let end = (offset + ZIP_PROGRESS_EMIT_BYTES as usize).min(bytes.len());
        let slice = &mut bytes[offset..end];
        input.read_exact(slice).with_context(|| {
            format!("Failed to read ZIP worker input {}", source_path.display())
        })?;
        crc.update(slice);
        let read = slice.len() as u64;
        offset = end;
        *pending_progress = pending_progress.saturating_add(read);
        if *pending_progress >= ZIP_PROGRESS_EMIT_BYTES {
            flush_parallel_zip_progress(progress_tx, pending_progress);
        }
    }

    Ok(ParallelZipFileInput {
        bytes,
        crc32: crc.finalize(),
    })
}

fn read_zip_file_bytes_into<F>(
    source_path: &Path,
    expected_len: u64,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
    bytes: &mut Vec<u8>,
) -> Result<u32>
where
    F: FnMut(u64),
{
    let mut input = File::open(source_path)
        .with_context(|| format!("Failed to open {}", source_path.display()))?;
    let capacity =
        usize::try_from(expected_len).context("ZIP direct input exceeded addressable memory")?;
    bytes.clear();
    if bytes.capacity() < capacity {
        bytes.reserve(capacity - bytes.len());
    }

    let mut crc = Crc32Hasher::new();
    while bytes.len() < capacity {
        ensure_not_canceled(should_cancel)?;
        let read_len = (capacity - bytes.len()).min(ZIP_PROGRESS_EMIT_BYTES as usize);
        let spare = &mut bytes.spare_capacity_mut()[..read_len];
        let slice =
            unsafe { std::slice::from_raw_parts_mut(spare.as_mut_ptr() as *mut u8, read_len) };
        input
            .read_exact(slice)
            .with_context(|| format!("Failed to read ZIP input {}", source_path.display()))?;
        crc.update(slice);
        unsafe {
            bytes.set_len(bytes.len() + read_len);
        }
        progress(read_len as u64);
    }

    Ok(crc.finalize())
}

fn flush_parallel_zip_progress(progress_tx: &mpsc::Sender<u64>, pending_progress: &mut u64) {
    if *pending_progress > 0 {
        let _ = progress_tx.send(*pending_progress);
        *pending_progress = 0;
    }
}

#[cfg(test)]
fn write_parallel_deflate_single_entry_zip<F>(
    zip_path: &Path,
    file: &PreparedZipFile,
    options: &CompressionOptions,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()>
where
    F: FnMut(u64),
{
    let header_layout = deflate_zip_header_layout(file.input_len);
    let cancel_requested = Arc::new(AtomicBool::new(false));
    let result = (|| -> Result<()> {
        let mut output = File::create(zip_path)
            .with_context(|| format!("Failed to create parallel ZIP {}", zip_path.display()))?;
        let entry_name_bytes = file.archive_path.as_bytes().to_vec();
        write_single_entry_zip_local_header(
            &mut output,
            &header_layout,
            &entry_name_bytes,
            0,
            file.input_len,
            0,
        )?;
        let file_output = write_parallel_deflate_entry_data(
            &mut output,
            file,
            options,
            progress,
            should_cancel,
            &cancel_requested,
        )?;

        finalize_single_entry_zip_archive(
            &mut output,
            &header_layout,
            &entry_name_bytes,
            file_output.crc32,
            file_output.compressed_size,
            file.input_len,
        )?;
        output
            .flush()
            .with_context(|| format!("Failed to flush parallel ZIP {}", zip_path.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(zip_path);
    }

    result
}

fn write_parallel_deflate_single_file_zip_archive<F>(
    mut output: File,
    directories: &[PreparedZipDirectory],
    file: &PreparedZipFile,
    report: &mut CompressionReport,
    options: &CompressionOptions,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
    staged_output: &Path,
) -> Result<()>
where
    F: FnMut(u64),
{
    let directory_layout = stored_directory_zip_header_layout();
    let file_layout = deflate_zip_header_layout(file.input_len);
    let file_entry_name = file.archive_path.as_bytes().to_vec();
    let mut central_directory_entries = Vec::with_capacity(directories.len() + 1);

    for directory in directories {
        ensure_not_canceled(should_cancel)?;
        let entry_name = directory.archive_path.as_bytes().to_vec();
        let local_header_offset = output.stream_position()?;
        write_single_entry_zip_local_header(&mut output, &directory_layout, &entry_name, 0, 0, 0)?;
        central_directory_entries.push(ManualZipCentralDirectoryEntry {
            header_layout: directory_layout,
            entry_name,
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            local_header_offset,
            external_attributes: (ZIP_DIRECTORY_UNIX_MODE << 16) | ZIP_DIRECTORY_DOS_ATTRIBUTE,
        });
    }

    ensure_not_canceled(should_cancel)?;
    let cancel_requested = Arc::new(AtomicBool::new(false));
    let file_local_header_offset = output.stream_position()?;
    write_single_entry_zip_local_header(
        &mut output,
        &file_layout,
        &file_entry_name,
        0,
        file.input_len,
        0,
    )?;
    let file_output = write_parallel_deflate_entry_data(
        &mut output,
        file,
        options,
        progress,
        should_cancel,
        &cancel_requested,
    )?;
    let after_file_data = output.stream_position()?;

    output.seek(SeekFrom::Start(file_local_header_offset))?;
    write_single_entry_zip_local_header(
        &mut output,
        &file_layout,
        &file_entry_name,
        file_output.crc32,
        file.input_len,
        file_output.compressed_size,
    )?;
    output.seek(SeekFrom::Start(after_file_data))?;

    central_directory_entries.push(ManualZipCentralDirectoryEntry {
        header_layout: file_layout,
        entry_name: file_entry_name,
        crc32: file_output.crc32,
        compressed_size: file_output.compressed_size,
        uncompressed_size: file.input_len,
        local_header_offset: file_local_header_offset,
        external_attributes: ZIP_REGULAR_FILE_UNIX_MODE << 16,
    });
    write_manual_zip_central_directory_and_end(&mut output, &central_directory_entries)?;
    output
        .flush()
        .with_context(|| format!("Failed to flush archive {}", staged_output.display()))?;
    report.files_added = 1;
    report.directories_added = directories.len();
    report.input_bytes = file.input_len;
    Ok(())
}

fn write_parallel_deflate_entry_data<W, F>(
    output: &mut W,
    file: &PreparedZipFile,
    options: &CompressionOptions,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
    cancel_requested: &Arc<AtomicBool>,
) -> Result<ParallelDeflateEntryOutput>
where
    W: Write,
    F: FnMut(u64),
{
    let chunk_specs = parallel_deflate_chunk_specs(file.input_len, options.thread_count.max(1));
    let shared_specs = Arc::new(Mutex::new(VecDeque::from(chunk_specs.clone())));
    let (message_tx, message_rx) = mpsc::channel();
    let worker_count = (options.thread_count.max(1) as usize).min(chunk_specs.len().max(1));
    let mut worker_handles = Vec::with_capacity(worker_count);

    cancel_requested.store(false, Ordering::Relaxed);

    for _ in 0..worker_count {
        let source_path = file.source_path.clone();
        let shared_specs = Arc::clone(&shared_specs);
        let cancel_requested = Arc::clone(cancel_requested);
        let message_tx = message_tx.clone();
        let level = options.level;
        worker_handles.push(Some(thread::spawn(move || -> Result<()> {
            loop {
                if cancel_requested.load(Ordering::Relaxed) {
                    break;
                }

                let spec = {
                    let mut pending_specs = shared_specs
                        .lock()
                        .map_err(|_| anyhow!("Parallel deflate task queue was poisoned"))?;
                    pending_specs.pop_front()
                };
                let Some(spec) = spec else {
                    break;
                };

                let result = compress_parallel_deflate_chunk(
                    &source_path,
                    spec,
                    level,
                    Arc::clone(&cancel_requested),
                    &message_tx,
                );
                let failed = result.is_err();
                let _ = message_tx.send(ParallelDeflateWorkerMessage::Chunk(result));
                if failed {
                    cancel_requested.store(true, Ordering::Relaxed);
                    break;
                }
            }

            Ok(())
        })));
    }
    drop(message_tx);

    let mut pending_outputs = BTreeMap::new();
    let mut expected_index = 0usize;
    let mut compressed_size = 0u64;
    let mut combined_crc: Option<Crc32Hasher> = None;
    let mut remaining_workers = worker_handles.len();
    let mut first_error = None;

    while remaining_workers > 0 {
        if should_cancel() {
            cancel_requested.store(true, Ordering::Relaxed);
        }

        match message_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(ParallelDeflateWorkerMessage::Progress(delta)) => progress(delta),
            Ok(ParallelDeflateWorkerMessage::Chunk(result)) => {
                handle_parallel_deflate_chunk_result(
                    result,
                    &mut pending_outputs,
                    &mut expected_index,
                    &mut combined_crc,
                    &mut compressed_size,
                    output,
                    &mut first_error,
                    cancel_requested,
                )?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }

        for handle in &mut worker_handles {
            let Some(join_handle) = handle.as_ref() else {
                continue;
            };
            if !join_handle.is_finished() {
                continue;
            }

            let join_handle = handle.take().unwrap();
            remaining_workers -= 1;

            match join_handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    cancel_requested.store(true, Ordering::Relaxed);
                }
                Err(_) => {
                    if first_error.is_none() {
                        first_error = Some(anyhow!("Parallel deflate worker thread panicked"));
                    }
                    cancel_requested.store(true, Ordering::Relaxed);
                }
            }
        }
    }

    while let Ok(message) = message_rx.try_recv() {
        match message {
            ParallelDeflateWorkerMessage::Progress(delta) => progress(delta),
            ParallelDeflateWorkerMessage::Chunk(result) => {
                handle_parallel_deflate_chunk_result(
                    result,
                    &mut pending_outputs,
                    &mut expected_index,
                    &mut combined_crc,
                    &mut compressed_size,
                    output,
                    &mut first_error,
                    cancel_requested,
                )?;
            }
        }
    }

    if cancel_requested.load(Ordering::Relaxed) && first_error.is_none() {
        bail!("Operation canceled");
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    if expected_index != chunk_specs.len() {
        bail!("Parallel deflate did not produce every ZIP chunk");
    }

    let crc32 = combined_crc
        .ok_or_else(|| anyhow!("Parallel deflate CRC was not computed"))?
        .finalize();

    Ok(ParallelDeflateEntryOutput {
        crc32,
        compressed_size,
    })
}

fn handle_parallel_deflate_chunk_result<W>(
    result: Result<ParallelDeflateChunkOutput>,
    pending_outputs: &mut BTreeMap<usize, ParallelDeflateChunkOutput>,
    expected_index: &mut usize,
    combined_crc: &mut Option<Crc32Hasher>,
    compressed_size: &mut u64,
    output: &mut W,
    first_error: &mut Option<anyhow::Error>,
    cancel_requested: &Arc<AtomicBool>,
) -> Result<()>
where
    W: Write,
{
    match result {
        Ok(output_chunk) => {
            pending_outputs.insert(output_chunk.index, output_chunk);
            flush_parallel_deflate_outputs(
                output,
                pending_outputs,
                expected_index,
                combined_crc,
                compressed_size,
            )?;
        }
        Err(error) => {
            if first_error.is_none() {
                *first_error = Some(error);
            }
            cancel_requested.store(true, Ordering::Relaxed);
        }
    }

    Ok(())
}

fn flush_parallel_deflate_outputs<W>(
    output: &mut W,
    pending_outputs: &mut BTreeMap<usize, ParallelDeflateChunkOutput>,
    expected_index: &mut usize,
    combined_crc: &mut Option<Crc32Hasher>,
    compressed_size: &mut u64,
) -> Result<()>
where
    W: Write,
{
    while let Some(chunk) = pending_outputs.remove(expected_index) {
        output.write_all(&chunk.compressed)?;
        *compressed_size = compressed_size
            .checked_add(chunk.compressed.len() as u64)
            .ok_or_else(|| anyhow!("Parallel ZIP compressed size overflow"))?;
        match combined_crc {
            Some(crc) => crc.combine(&chunk.crc),
            None => *combined_crc = Some(chunk.crc),
        }
        *expected_index += 1;
    }

    Ok(())
}

fn compress_parallel_deflate_chunk(
    source_path: &Path,
    spec: ParallelDeflateChunkSpec,
    level: CompressionLevel,
    cancel_requested: Arc<AtomicBool>,
    message_tx: &mpsc::Sender<ParallelDeflateWorkerMessage>,
) -> Result<ParallelDeflateChunkOutput> {
    if cancel_requested.load(Ordering::Relaxed) {
        bail!("Operation canceled");
    }

    let mut input = File::open(source_path)
        .with_context(|| format!("Failed to open {}", source_path.display()))?;
    let dictionary_len = spec.offset.min(ZIP_PARALLEL_DEFLATE_WINDOW_BYTES);
    let dictionary = if dictionary_len > 0 {
        input.seek(SeekFrom::Start(spec.offset - dictionary_len))?;
        let mut dictionary = vec![0u8; dictionary_len as usize];
        input.read_exact(&mut dictionary).with_context(|| {
            format!(
                "Failed to read the ZIP preset dictionary from {}",
                source_path.display()
            )
        })?;
        Some(dictionary)
    } else {
        None
    };

    input.seek(SeekFrom::Start(spec.offset))?;
    let mut chunk = vec![0u8; spec.input_len as usize];
    let mut filled = 0usize;
    let mut pending_progress = 0u64;
    while filled < chunk.len() {
        if cancel_requested.load(Ordering::Relaxed) {
            bail!("Operation canceled");
        }

        let bytes_read = input.read(&mut chunk[filled..]).with_context(|| {
            format!(
                "Failed to read ZIP chunk from {} at offset {}",
                source_path.display(),
                spec.offset + filled as u64
            )
        })?;
        if bytes_read == 0 {
            bail!(
                "Unexpected end of file while reading ZIP chunk from {}",
                source_path.display()
            );
        }

        filled += bytes_read;
        pending_progress = pending_progress.saturating_add(bytes_read as u64);
        if pending_progress >= ZIP_PROGRESS_EMIT_BYTES {
            let _ = message_tx.send(ParallelDeflateWorkerMessage::Progress(pending_progress));
            pending_progress = 0;
        }
    }

    if pending_progress > 0 {
        let _ = message_tx.send(ParallelDeflateWorkerMessage::Progress(pending_progress));
    }

    let mut crc = Crc32Hasher::new();
    crc.update(&chunk);
    let compressed =
        compress_parallel_deflate_bytes(&chunk, dictionary.as_deref(), spec.is_last, level)?;

    Ok(ParallelDeflateChunkOutput {
        index: spec.index,
        compressed,
        crc,
    })
}

fn compress_parallel_deflate_bytes(
    input: &[u8],
    dictionary: Option<&[u8]>,
    is_last: bool,
    level: CompressionLevel,
) -> Result<Vec<u8>> {
    let mut compressor = create_zip_deflate_compressor(level);
    compress_parallel_deflate_bytes_with_compressor(input, dictionary, is_last, &mut compressor)
}

fn create_zip_whole_buffer_deflate_compressor(
    level: CompressionLevel,
) -> Result<WholeBufferDeflateCompressor> {
    let compression_level = WholeBufferDeflateLevel::new(level.zip_deflate_level() as i32)
        .map_err(|_| anyhow!("Unsupported ZIP whole-buffer deflate level"))?;
    Ok(WholeBufferDeflateCompressor::new(compression_level))
}

fn compress_zip_whole_buffer_deflate_bytes(
    input: &[u8],
    compressor: &mut WholeBufferDeflateCompressor,
) -> Result<Vec<u8>> {
    let output_capacity = compressor.deflate_compress_bound(input.len());
    let mut output = Vec::with_capacity(output_capacity);
    // The compressor fully initializes the returned prefix before we truncate it.
    unsafe {
        output.set_len(output_capacity);
    }
    let written = compressor
        .deflate_compress(input, &mut output)
        .map_err(|error| anyhow!("Failed to compress a ZIP deflate buffer: {error}"))?;
    output.truncate(written);
    Ok(output)
}

fn compress_zstd_bytes(input: &[u8], level: CompressionLevel) -> Result<Vec<u8>> {
    let zstd_level = level.zstd_level() as i32;
    zstd::encode_all(input, zstd_level)
        .map_err(|error| anyhow!("Failed to compress a ZIP Zstd buffer: {error}"))
}

fn compress_bzip2_bytes(input: &[u8], level: CompressionLevel) -> Result<Vec<u8>> {
    let bz_level = BzCompression::new(level.bzip2_level());
    let mut encoder = BzEncoder::new(Vec::new(), bz_level);
    encoder
        .write_all(input)
        .map_err(|error| anyhow!("Failed to compress a ZIP Bzip2 buffer: {error}"))?;
    encoder
        .finish()
        .map_err(|error| anyhow!("Failed to finalize Bzip2 buffer: {error}"))
}

fn compress_xz_bytes(input: &[u8], level: CompressionLevel) -> Result<Vec<u8>> {
    let xz_level = level.xz_level();
    let mut encoder = XzEncoder::new(Vec::new(), xz_level);
    encoder
        .write_all(input)
        .map_err(|error| anyhow!("Failed to compress a ZIP XZ buffer: {error}"))?;
    encoder
        .finish()
        .map_err(|error| anyhow!("Failed to finalize XZ buffer: {error}"))
}

const GZIP_LIBDEFLATER_MAX_BYTES: u64 = 512 * 1024 * 1024;

fn create_gzip_compressor(level: CompressionLevel) -> Result<WholeBufferDeflateCompressor> {
    let compression_level = WholeBufferDeflateLevel::new(level.gzip_level() as i32)
        .map_err(|_| anyhow!("Unsupported gzip compression level"))?;
    Ok(WholeBufferDeflateCompressor::new(compression_level))
}

fn compress_gzip_bytes(
    input: &[u8],
    compressor: &mut WholeBufferDeflateCompressor,
) -> Result<Vec<u8>> {
    let output_capacity = compressor.gzip_compress_bound(input.len());
    let mut output = Vec::with_capacity(output_capacity);
    unsafe {
        output.set_len(output_capacity);
    }
    let written = compressor
        .gzip_compress(input, &mut output)
        .map_err(|error| anyhow!("Failed to compress gzip buffer: {error}"))?;
    output.truncate(written);
    Ok(output)
}

fn create_zip_deflate_compressor(level: CompressionLevel) -> RawDeflateCompressor {
    RawDeflateCompressor::new(GzCompression::new(level.zip_deflate_level()), false)
}

fn compress_parallel_deflate_bytes_with_compressor(
    input: &[u8],
    dictionary: Option<&[u8]>,
    is_last: bool,
    compressor: &mut RawDeflateCompressor,
) -> Result<Vec<u8>> {
    compressor.reset();
    if let Some(dictionary) = dictionary.filter(|dictionary| !dictionary.is_empty()) {
        compressor
            .set_dictionary(dictionary)
            .context("Failed to configure the ZIP preset dictionary")?;
    }

    let mut output = Vec::with_capacity(input.len() / 2 + ZIP_PARALLEL_DEFLATE_OUTPUT_BUFFER_BYTES);
    let mut scratch = [0u8; ZIP_PARALLEL_DEFLATE_OUTPUT_BUFFER_BYTES];
    let mut input_offset = 0usize;
    let final_flush = if is_last {
        FlushCompress::Finish
    } else {
        FlushCompress::Sync
    };

    loop {
        let before_in = compressor.total_in();
        let before_out = compressor.total_out();
        let flush = if input_offset >= input.len() {
            final_flush
        } else {
            FlushCompress::None
        };
        let status = compressor
            .compress(&input[input_offset..], &mut scratch, flush)
            .context("Failed to compress a ZIP deflate chunk")?;
        let consumed = (compressor.total_in() - before_in) as usize;
        let produced = (compressor.total_out() - before_out) as usize;
        input_offset += consumed;
        if produced > 0 {
            output.extend_from_slice(&scratch[..produced]);
        }

        if input_offset >= input.len() {
            match final_flush {
                FlushCompress::Finish if status == FlateStatus::StreamEnd => break,
                FlushCompress::Sync if produced == 0 => break,
                _ => {}
            }
        }

        if consumed == 0 && produced == 0 {
            bail!("ZIP deflate chunk compressor made no forward progress");
        }
    }

    Ok(output)
}

fn parallel_deflate_chunk_specs(
    total_bytes: u64,
    thread_count: u32,
) -> Vec<ParallelDeflateChunkSpec> {
    let thread_count = thread_count.max(1) as u64;
    let target_chunk_bytes = ceil_div_u64(total_bytes, thread_count.saturating_mul(3)).clamp(
        ZIP_PARALLEL_DEFLATE_MIN_CHUNK_BYTES,
        ZIP_PARALLEL_DEFLATE_MAX_CHUNK_BYTES,
    );
    let mut specs = Vec::new();
    let mut offset = 0u64;
    let mut index = 0usize;

    while offset < total_bytes {
        let input_len = (total_bytes - offset).min(target_chunk_bytes);
        specs.push(ParallelDeflateChunkSpec {
            index,
            offset,
            input_len,
            is_last: offset + input_len >= total_bytes,
        });
        offset += input_len;
        index += 1;
    }

    specs
}

fn single_entry_zip_header_layout(
    uncompressed_size: u64,
    method: u16,
) -> SingleEntryZipHeaderLayout {
    let use_zip64 = uncompressed_size > ZIP64_U32_MAX;
    let version_needed = if use_zip64 { 45 } else { 20 };
    let version_made_by = (3u16 << 8) | version_needed;
    let general_purpose_flag = ZIP_UTF8_GENERAL_PURPOSE_FLAG;
    let last_modified = DateTime::default_for_write();
    let (last_mod_date, last_mod_time): (u16, u16) = last_modified.into();

    SingleEntryZipHeaderLayout {
        version_needed,
        version_made_by,
        general_purpose_flag,
        method,
        last_mod_time,
        last_mod_date,
        use_zip64,
    }
}

fn deflate_zip_header_layout(uncompressed_size: u64) -> SingleEntryZipHeaderLayout {
    single_entry_zip_header_layout(uncompressed_size, ZIP_DEFLATE_METHOD_ID)
}

fn stored_directory_zip_header_layout() -> SingleEntryZipHeaderLayout {
    let version_needed = 20;
    let version_made_by = (3u16 << 8) | version_needed;
    let last_modified = DateTime::default_for_write();
    let (last_mod_date, last_mod_time): (u16, u16) = last_modified.into();

    SingleEntryZipHeaderLayout {
        version_needed,
        version_made_by,
        general_purpose_flag: ZIP_UTF8_GENERAL_PURPOSE_FLAG,
        method: ZIP_STORED_METHOD_ID,
        last_mod_time,
        last_mod_date,
        use_zip64: false,
    }
}

fn stored_file_zip_header_layout(uncompressed_size: u64) -> SingleEntryZipHeaderLayout {
    let use_zip64 = uncompressed_size > ZIP64_U32_MAX;
    let version_needed = if use_zip64 { 45 } else { 20 };
    let version_made_by = (3u16 << 8) | version_needed;
    let last_modified = DateTime::default_for_write();
    let (last_mod_date, last_mod_time): (u16, u16) = last_modified.into();

    SingleEntryZipHeaderLayout {
        version_needed,
        version_made_by,
        general_purpose_flag: ZIP_UTF8_GENERAL_PURPOSE_FLAG,
        method: ZIP_STORED_METHOD_ID,
        last_mod_time,
        last_mod_date,
        use_zip64,
    }
}

fn zip_local_file_record_len(
    layout: &SingleEntryZipHeaderLayout,
    entry_name_len: usize,
    compressed_size: u64,
) -> u64 {
    let zip64_extra_len = if layout.use_zip64 { 20u64 } else { 0u64 };
    30u64 + entry_name_len as u64 + zip64_extra_len + compressed_size
}

fn write_single_entry_zip_local_header<W: Write>(
    output: &mut W,
    layout: &SingleEntryZipHeaderLayout,
    entry_name: &[u8],
    crc32: u32,
    uncompressed_size: u64,
    compressed_size: u64,
) -> Result<()> {
    write_u32(output, ZIP_LOCAL_FILE_HEADER_SIGNATURE)?;
    write_u16(output, layout.version_needed)?;
    write_u16(output, layout.general_purpose_flag)?;
    write_u16(output, layout.method)?;
    write_u16(output, layout.last_mod_time)?;
    write_u16(output, layout.last_mod_date)?;
    write_u32(output, crc32)?;
    if layout.use_zip64 {
        write_u32(output, u32::MAX)?;
        write_u32(output, u32::MAX)?;
    } else {
        write_u32(
            output,
            u32::try_from(compressed_size)
                .context("ZIP compressed size overflowed 32-bit local header field")?,
        )?;
        write_u32(
            output,
            u32::try_from(uncompressed_size)
                .context("ZIP uncompressed size overflowed 32-bit local header field")?,
        )?;
    }
    write_u16(
        output,
        u16::try_from(entry_name.len()).context("ZIP entry name is too long")?,
    )?;
    let local_extra_len = if layout.use_zip64 { 20u16 } else { 0u16 };
    write_u16(output, local_extra_len)?;
    output.write_all(entry_name)?;
    if layout.use_zip64 {
        write_u16(output, 0x0001)?;
        write_u16(output, 16)?;
        write_u64(output, uncompressed_size)?;
        write_u64(output, compressed_size)?;
    }
    Ok(())
}

#[cfg(test)]
fn finalize_single_entry_zip_archive<W>(
    output: &mut W,
    layout: &SingleEntryZipHeaderLayout,
    entry_name: &[u8],
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
) -> Result<()>
where
    W: Write + Seek,
{
    if !layout.use_zip64 && (compressed_size > ZIP64_U32_MAX || uncompressed_size > ZIP64_U32_MAX) {
        bail!("Parallel ZIP entry exceeded the non-ZIP64 size limits");
    }

    let local_header_offset = 0u64;
    output.seek(SeekFrom::Start(0))?;
    write_single_entry_zip_local_header(
        output,
        layout,
        entry_name,
        crc32,
        uncompressed_size,
        compressed_size,
    )?;

    let central_directory_offset = output.seek(SeekFrom::End(0))?;
    write_single_entry_zip_central_directory_entry(
        output,
        layout,
        entry_name,
        crc32,
        compressed_size,
        uncompressed_size,
        local_header_offset,
        ZIP_REGULAR_FILE_UNIX_MODE << 16,
    )?;
    let central_directory_size = output
        .stream_position()?
        .checked_sub(central_directory_offset)
        .ok_or_else(|| anyhow!("ZIP central directory position underflow"))?;
    write_manual_zip_end_of_central_directory(
        output,
        layout.version_made_by,
        layout.version_needed,
        1,
        central_directory_size,
        central_directory_offset,
        layout.use_zip64,
    )
}

fn write_manual_zip_central_directory_and_end<W>(
    output: &mut W,
    entries: &[ManualZipCentralDirectoryEntry],
) -> Result<()>
where
    W: Write + Seek,
{
    let central_directory_offset = output.stream_position()?;
    let mut version_needed = 20u16;
    let mut version_made_by = (3u16 << 8) | version_needed;
    let mut requires_zip64 = false;

    for entry in entries {
        write_single_entry_zip_central_directory_entry(
            output,
            &entry.header_layout,
            &entry.entry_name,
            entry.crc32,
            entry.compressed_size,
            entry.uncompressed_size,
            entry.local_header_offset,
            entry.external_attributes,
        )?;
        version_needed = version_needed.max(entry.header_layout.version_needed);
        version_made_by = version_made_by.max(entry.header_layout.version_made_by);
        requires_zip64 |=
            entry.header_layout.use_zip64 || entry.local_header_offset > ZIP64_U32_MAX;
    }

    let central_directory_size = output
        .stream_position()?
        .checked_sub(central_directory_offset)
        .ok_or_else(|| anyhow!("ZIP central directory position underflow"))?;
    write_manual_zip_end_of_central_directory(
        output,
        version_made_by,
        version_needed,
        entries.len() as u64,
        central_directory_size,
        central_directory_offset,
        requires_zip64,
    )
}

fn write_manual_zip_end_of_central_directory<W>(
    output: &mut W,
    version_made_by: u16,
    version_needed: u16,
    entry_count: u64,
    central_directory_size: u64,
    central_directory_offset: u64,
    requires_zip64: bool,
) -> Result<()>
where
    W: Write + Seek,
{
    let needs_zip64 = requires_zip64
        || entry_count > u16::MAX as u64
        || central_directory_size > ZIP64_U32_MAX
        || central_directory_offset > ZIP64_U32_MAX;
    let archive_version_needed = if needs_zip64 {
        version_needed.max(45)
    } else {
        version_needed
    };
    let archive_version_made_by = if needs_zip64 {
        version_made_by.max((3u16 << 8) | 45)
    } else {
        version_made_by
    };

    if needs_zip64 {
        let zip64_eocd_offset = output.stream_position()?;
        write_u32(output, ZIP64_END_OF_CENTRAL_DIRECTORY_SIGNATURE)?;
        write_u64(output, 44)?;
        write_u16(output, archive_version_made_by)?;
        write_u16(output, archive_version_needed)?;
        write_u32(output, 0)?;
        write_u32(output, 0)?;
        write_u64(output, entry_count)?;
        write_u64(output, entry_count)?;
        write_u64(output, central_directory_size)?;
        write_u64(output, central_directory_offset)?;

        write_u32(output, ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_SIGNATURE)?;
        write_u32(output, 0)?;
        write_u64(output, zip64_eocd_offset)?;
        write_u32(output, 1)?;
    }

    write_u32(output, ZIP_END_OF_CENTRAL_DIRECTORY_SIGNATURE)?;
    write_u16(output, 0)?;
    write_u16(output, 0)?;
    if needs_zip64 {
        write_u16(output, u16::MAX)?;
        write_u16(output, u16::MAX)?;
        write_u32(output, u32::MAX)?;
        write_u32(output, u32::MAX)?;
    } else {
        write_u16(
            output,
            u16::try_from(entry_count).context("ZIP entry count overflowed 16-bit EOCD field")?,
        )?;
        write_u16(
            output,
            u16::try_from(entry_count).context("ZIP entry count overflowed 16-bit EOCD field")?,
        )?;
        write_u32(
            output,
            u32::try_from(central_directory_size)
                .context("ZIP central directory size overflowed 32-bit EOCD field")?,
        )?;
        write_u32(
            output,
            u32::try_from(central_directory_offset)
                .context("ZIP central directory offset overflowed 32-bit EOCD field")?,
        )?;
    }
    write_u16(output, 0)?;
    Ok(())
}

fn write_single_entry_zip_central_directory_entry<W: Write>(
    output: &mut W,
    layout: &SingleEntryZipHeaderLayout,
    entry_name: &[u8],
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
    local_header_offset: u64,
    external_attributes: u32,
) -> Result<()> {
    let use_zip64_sizes = layout.use_zip64;
    let use_zip64_offset = local_header_offset > ZIP64_U32_MAX;
    let zip64_extra_bytes =
        (if use_zip64_sizes { 16 } else { 0 }) + (if use_zip64_offset { 8 } else { 0 });
    let central_extra_len = if zip64_extra_bytes > 0 {
        4u16.checked_add(zip64_extra_bytes)
            .ok_or_else(|| anyhow!("ZIP central directory extra field length overflow"))?
    } else {
        0u16
    };

    write_u32(output, ZIP_CENTRAL_DIRECTORY_HEADER_SIGNATURE)?;
    write_u16(output, layout.version_made_by)?;
    write_u16(output, layout.version_needed)?;
    write_u16(output, layout.general_purpose_flag)?;
    write_u16(output, layout.method)?;
    write_u16(output, layout.last_mod_time)?;
    write_u16(output, layout.last_mod_date)?;
    write_u32(output, crc32)?;
    if use_zip64_sizes {
        write_u32(output, u32::MAX)?;
        write_u32(output, u32::MAX)?;
    } else {
        write_u32(
            output,
            u32::try_from(compressed_size)
                .context("ZIP compressed size overflowed 32-bit central directory field")?,
        )?;
        write_u32(
            output,
            u32::try_from(uncompressed_size)
                .context("ZIP uncompressed size overflowed 32-bit central directory field")?,
        )?;
    }
    write_u16(
        output,
        u16::try_from(entry_name.len()).context("ZIP entry name is too long")?,
    )?;
    write_u16(output, central_extra_len)?;
    write_u16(output, 0)?;
    write_u16(output, 0)?;
    write_u16(output, 0)?;
    write_u32(output, external_attributes)?;
    if use_zip64_offset {
        write_u32(output, u32::MAX)?;
    } else {
        write_u32(
            output,
            u32::try_from(local_header_offset)
                .context("ZIP local header offset overflowed 32-bit central directory field")?,
        )?;
    }
    output.write_all(entry_name)?;
    if zip64_extra_bytes > 0 {
        write_u16(output, 0x0001)?;
        write_u16(output, zip64_extra_bytes)?;
        if use_zip64_sizes {
            write_u64(output, uncompressed_size)?;
            write_u64(output, compressed_size)?;
        }
        if use_zip64_offset {
            write_u64(output, local_header_offset)?;
        }
    }
    Ok(())
}

fn write_u16<W: Write>(writer: &mut W, value: u16) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn write_u32<W: Write>(writer: &mut W, value: u32) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn write_u64<W: Write>(writer: &mut W, value: u64) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn add_zip_directory(
    writer: &mut zip::ZipWriter<BufWriter<File>>,
    archive_path: &Path,
    options: &CompressionOptions,
    report: &mut CompressionReport,
    seen_entries: &mut HashSet<String>,
) -> Result<()> {
    let entry_name = normalize_zip_directory_path(archive_path)?;
    if !seen_entries.insert(entry_name.clone()) {
        return Ok(());
    }

    writer
        .add_directory(&entry_name, zip_directory_options(options, 0o755))
        .with_context(|| format!("Failed to add directory {} to ZIP archive", entry_name))?;
    report.directories_added += 1;
    Ok(())
}

fn add_zip_file<F>(
    writer: &mut zip::ZipWriter<BufWriter<File>>,
    source: &Path,
    archive_path: &Path,
    options: &CompressionOptions,
    zip_method: ZipCompressionMethod,
    report: &mut CompressionReport,
    seen_entries: &mut HashSet<String>,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()>
where
    F: FnMut(u64),
{
    let entry_name = normalize_zip_file_path(archive_path)?;
    if !seen_entries.insert(entry_name.clone()) {
        bail!("Duplicate archive entry name: {entry_name}");
    }

    let metadata = source
        .metadata()
        .with_context(|| format!("Failed to inspect {}", source.display()))?;
    let mut input =
        File::open(source).with_context(|| format!("Failed to open {}", source.display()))?;

    writer
        .start_file(&entry_name, zip_file_options(options, zip_method, 0o644))
        .with_context(|| format!("Failed to add file {} to ZIP archive", entry_name))?;
    copy_with_progress(&mut input, writer, progress, should_cancel)
        .with_context(|| format!("Failed to write {} into ZIP archive", source.display()))?;
    report.files_added += 1;
    report.input_bytes += metadata.len();
    Ok(())
}

fn add_sources_to_tar<W, F>(
    builder: &mut TarBuilder<W>,
    sources: &[PathBuf],
    report: &mut CompressionReport,
    seen_entries: &mut HashSet<String>,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()>
where
    W: Write,
    F: FnMut(u64),
{
    for source in sources {
        ensure_not_canceled(should_cancel)?;
        if !source.exists() {
            bail!("Compression source not found: {}", source.display());
        }

        let root_name = source
            .file_name()
            .ok_or_else(|| anyhow!("Failed to derive an archive name from {}", source.display()))?;
        add_source_to_tar(
            builder,
            source,
            &PathBuf::from(root_name),
            report,
            seen_entries,
            excluded_paths,
            excluded_outputs,
            progress,
            should_cancel,
        )?;
    }
    Ok(())
}

fn add_source_to_tar<W, F>(
    builder: &mut TarBuilder<W>,
    source: &Path,
    archive_path: &Path,
    report: &mut CompressionReport,
    seen_entries: &mut HashSet<String>,
    excluded_paths: &[PathBuf],
    excluded_outputs: &[PathBuf],
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()>
where
    W: Write,
    F: FnMut(u64),
{
    ensure_not_canceled(should_cancel)?;
    let source_canonical = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    if excluded_paths
        .iter()
        .any(|path| source_canonical == *path || source_canonical.starts_with(path))
    {
        return Ok(());
    }

    if source.is_dir() {
        add_tar_directory(builder, source, archive_path, report, seen_entries)?;

        let mut children = fs::read_dir(source)
            .with_context(|| format!("Failed to enumerate {}", source.display()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("Failed to read {}", source.display()))?;
        children.sort_by_key(|entry| entry.path());

        for child in children {
            let child_path = child.path();
            let child_canonical =
                fs::canonicalize(&child_path).unwrap_or_else(|_| child_path.clone());
            if excluded_paths
                .iter()
                .any(|path| child_canonical == *path || child_canonical.starts_with(path))
            {
                continue;
            }
            if excluded_outputs.iter().any(|path| path == &child_canonical) {
                continue;
            }

            add_source_to_tar(
                builder,
                &child_path,
                &archive_path.join(child.file_name()),
                report,
                seen_entries,
                excluded_paths,
                excluded_outputs,
                progress,
                should_cancel,
            )?;
        }
        return Ok(());
    }

    if !source.is_file() {
        bail!("Unsupported compression source type: {}", source.display());
    }

    add_tar_file(
        builder,
        source,
        archive_path,
        report,
        seen_entries,
        progress,
        should_cancel,
    )
}

fn add_tar_directory<W>(
    builder: &mut TarBuilder<W>,
    source: &Path,
    archive_path: &Path,
    report: &mut CompressionReport,
    seen_entries: &mut HashSet<String>,
) -> Result<()>
where
    W: Write,
{
    let entry_name = normalize_tar_entry_path(archive_path)?;
    if !seen_entries.insert(format!("dir:{entry_name}")) {
        return Ok(());
    }

    builder
        .append_dir(&entry_name, source)
        .with_context(|| format!("Failed to add directory {} to TAR archive", entry_name))?;
    report.directories_added += 1;
    Ok(())
}

fn add_tar_file<W, F>(
    builder: &mut TarBuilder<W>,
    source: &Path,
    archive_path: &Path,
    report: &mut CompressionReport,
    seen_entries: &mut HashSet<String>,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()>
where
    W: Write,
    F: FnMut(u64),
{
    let entry_name = normalize_tar_entry_path(archive_path)?;
    if !seen_entries.insert(format!("file:{entry_name}")) {
        bail!("Duplicate archive entry name: {entry_name}");
    }

    let metadata = source
        .metadata()
        .with_context(|| format!("Failed to inspect {}", source.display()))?;
    let input =
        File::open(source).with_context(|| format!("Failed to open {}", source.display()))?;

    let mut header = TarHeader::new_gnu();
    header.set_metadata_in_mode(&metadata, TarHeaderMode::Complete);
    let mut input = ProgressReader::new(input, progress, should_cancel);
    builder
        .append_data(&mut header, &entry_name, &mut input)
        .with_context(|| format!("Failed to add file {} to TAR archive", entry_name))?;
    report.files_added += 1;
    report.input_bytes += metadata.len();
    Ok(())
}

fn extract_stream_to_destination<R, F>(
    reader: &mut R,
    destination: &Path,
    report: &mut ExtractionReport,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
    scan_files: bool,
) -> Result<()>
where
    R: Read + ?Sized,
    F: FnMut(u64),
{
    let staged_output = temporary_output_path(destination)?;
    let result = (|| -> Result<()> {
        let mut output = File::create(&staged_output)
            .with_context(|| format!("Failed to create {}", staged_output.display()))?;
        copy_with_progress(reader, &mut output, progress, should_cancel)
            .with_context(|| format!("Failed to extract {}", destination.display()))?;
        output.flush().ok();
        drop(output);
        if scan_files {
            let data = fs::read(&staged_output).with_context(|| {
                format!(
                    "Failed to read staged output for AMSI scan: {}",
                    staged_output.display()
                )
            })?;
            let name = destination
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            amsi_scan_data(
                name,
                &data,
                &ExtractOptions {
                    scan_files: true,
                    ..ExtractOptions::default()
                },
            )?;
        }
        commit_staged_output_file(&staged_output, destination)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&staged_output);
        if let Some(parent) = staged_output.parent() {
            let _ = remove_empty_directories_upward(parent, destination.parent());
        }
    } else {
        report.files_written += 1;
    }

    result
}

struct CommittedArchiveOutput {
    archive_path: PathBuf,
    output_bytes: u64,
}

fn commit_staged_archive_output(
    staged_output: &Path,
    destination: &Path,
    split_volume_size: Option<u64>,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<CommittedArchiveOutput> {
    let output_bytes = staged_output
        .metadata()
        .with_context(|| format!("Failed to inspect archive {}", staged_output.display()))?
        .len();

    if let Some(volume_size) = split_volume_size {
        let archive_path = split_staged_output_file_into_volumes(
            staged_output,
            destination,
            volume_size,
            should_cancel,
        )?;
        return Ok(CommittedArchiveOutput {
            archive_path,
            output_bytes,
        });
    }

    commit_staged_output_file(staged_output, destination)?;
    Ok(CommittedArchiveOutput {
        archive_path: destination.to_path_buf(),
        output_bytes,
    })
}

fn temporary_output_path(destination: &Path) -> Result<PathBuf> {
    let parent = destination.parent().ok_or_else(|| {
        anyhow!(
            "Cannot determine the parent folder for {}",
            destination.display()
        )
    })?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Invalid destination file name: {}", destination.display()))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..1024u32 {
        let candidate = parent.join(format!(
            ".{file_name}.fastzip-part-{}-{timestamp}-{attempt}",
            std::process::id()
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    bail!(
        "Failed to allocate a temporary output path for {}",
        destination.display()
    )
}

fn commit_staged_output_file(staged_output: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        if destination.is_dir() {
            bail!(
                "Cannot replace directory with file: {}",
                destination.display()
            );
        }
        fs::remove_file(destination)
            .with_context(|| format!("Failed to replace {}", destination.display()))?;
    }

    fs::rename(staged_output, destination).with_context(|| {
        format!(
            "Failed to move {} into {}",
            staged_output.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn split_staged_output_file_into_volumes(
    staged_output: &Path,
    destination: &Path,
    volume_size: u64,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<PathBuf> {
    let total_bytes = staged_output
        .metadata()
        .with_context(|| format!("Failed to inspect archive {}", staged_output.display()))?
        .len();
    let part_count: u32 = ceil_div_u64(total_bytes.max(1), volume_size.max(1))
        .try_into()
        .map_err(|_| anyhow!("Split volume count overflowed"))?;
    let width = 3usize.max(part_count.to_string().len());
    let final_parts = (1..=part_count)
        .map(|part_index| split_volume_output_path(destination, part_index, width))
        .collect::<Result<Vec<_>>>()?;
    let staged_parts = final_parts
        .iter()
        .map(|path| temporary_output_path(path))
        .collect::<Result<Vec<_>>>()?;

    let result = (|| -> Result<()> {
        let mut input = File::open(staged_output)
            .with_context(|| format!("Failed to open {}", staged_output.display()))?;
        let mut buffer = vec![0u8; 1024 * 1024];

        for staged_part in &staged_parts {
            ensure_not_canceled(should_cancel)?;
            let mut remaining = volume_size;
            let mut wrote_bytes = 0u64;
            let part_file = File::create(staged_part)
                .with_context(|| format!("Failed to create {}", staged_part.display()))?;
            let mut output = BufWriter::with_capacity(1024 * 1024, part_file);

            while remaining > 0 {
                ensure_not_canceled(should_cancel)?;
                let read_len = remaining.min(buffer.len() as u64) as usize;
                let bytes_read = input
                    .read(&mut buffer[..read_len])
                    .with_context(|| format!("Failed to read {}", staged_output.display()))?;
                if bytes_read == 0 {
                    break;
                }

                output
                    .write_all(&buffer[..bytes_read])
                    .with_context(|| format!("Failed to write {}", staged_part.display()))?;
                wrote_bytes = wrote_bytes.saturating_add(bytes_read as u64);
                remaining = remaining.saturating_sub(bytes_read as u64);
            }

            output
                .flush()
                .with_context(|| format!("Failed to flush {}", staged_part.display()))?;
            if wrote_bytes == 0 {
                bail!(
                    "Failed to split archive into volume {}",
                    staged_part.display()
                );
            }
        }

        if destination.exists() && destination.is_file() {
            fs::remove_file(destination)
                .with_context(|| format!("Failed to replace {}", destination.display()))?;
        }
        for existing_part in collect_existing_split_volume_paths(destination)? {
            if existing_part.is_file() {
                fs::remove_file(&existing_part)
                    .with_context(|| format!("Failed to replace {}", existing_part.display()))?;
            }
        }

        for (staged_part, final_part) in staged_parts.iter().zip(final_parts.iter()) {
            if final_part.exists() && final_part.is_file() {
                fs::remove_file(final_part)
                    .with_context(|| format!("Failed to replace {}", final_part.display()))?;
            }
            fs::rename(staged_part, final_part).with_context(|| {
                format!(
                    "Failed to move {} into {}",
                    staged_part.display(),
                    final_part.display()
                )
            })?;
        }

        fs::remove_file(staged_output)
            .with_context(|| format!("Failed to remove {}", staged_output.display()))?;
        Ok(())
    })();

    if result.is_err() {
        for staged_part in staged_parts {
            let _ = fs::remove_file(staged_part);
        }
    }

    result?;
    Ok(final_parts[0].clone())
}

fn remove_empty_directories_upward(start: &Path, stop_at: Option<&Path>) -> Result<()> {
    let stop_at = stop_at.and_then(|path| fs::canonicalize(path).ok());
    let mut current = Some(start.to_path_buf());

    while let Some(path) = current {
        if stop_at.as_ref().is_some_and(|stop| stop == &path) {
            break;
        }

        match fs::remove_dir(&path) {
            Ok(()) => {
                current = path.parent().map(Path::to_path_buf);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                current = path.parent().map(Path::to_path_buf);
            }
            Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => break,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to clean up temporary directory {}", path.display())
                });
            }
        }
    }

    Ok(())
}

fn copy_with_progress<R, W, F>(
    reader: &mut R,
    writer: &mut W,
    progress: &mut F,
    should_cancel: &mut impl FnMut() -> bool,
) -> io::Result<u64>
where
    R: Read + ?Sized,
    W: Write,
    F: FnMut(u64),
{
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut total = 0u64;

    loop {
        if should_cancel() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation canceled",
            ));
        }
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        writer.write_all(&buffer[..bytes_read])?;
        total += bytes_read as u64;
        progress(bytes_read as u64);
    }

    Ok(total)
}

struct ProgressReader<'a, R, F, C> {
    inner: R,
    progress: &'a mut F,
    should_cancel: &'a mut C,
}

impl<'a, R, F, C> ProgressReader<'a, R, F, C> {
    fn new(inner: R, progress: &'a mut F, should_cancel: &'a mut C) -> Self {
        Self {
            inner,
            progress,
            should_cancel,
        }
    }
}

impl<R, F, C> Read for ProgressReader<'_, R, F, C>
where
    R: Read,
    F: FnMut(u64),
    C: FnMut() -> bool,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if (self.should_cancel)() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation canceled",
            ));
        }

        let bytes_read = self.inner.read(buf)?;
        if bytes_read > 0 {
            (self.progress)(bytes_read as u64);
        }
        Ok(bytes_read)
    }
}

fn ensure_not_canceled(should_cancel: &mut impl FnMut() -> bool) -> Result<()> {
    if should_cancel() {
        bail!("Operation canceled");
    }
    Ok(())
}

fn ceil_div_u64(value: u64, divisor: u64) -> u64 {
    if divisor == 0 {
        return value;
    }
    value.saturating_add(divisor - 1) / divisor
}

fn effective_zip_method_for_path(
    path: &Path,
    options: &CompressionOptions,
) -> ZipCompressionMethod {
    if matches!(options.zip_method, ZipCompressionMethod::Stored)
        || !is_likely_incompressible_path(path)
    {
        options.zip_method
    } else {
        ZipCompressionMethod::Stored
    }
}

fn zip_file_options<'a>(
    options: &'a CompressionOptions,
    zip_method: ZipCompressionMethod,
    unix_permissions: u32,
) -> ZipFileOptions<'a, ()> {
    let zip_method = zip_method.to_zip_method();
    let compression_level = match zip_method {
        CompressionMethod::Stored => None,
        CompressionMethod::Zstd => Some(options.level.zstd_level()),
        CompressionMethod::Deflated => Some(i64::from(options.level.zip_deflate_level())),
        _ => options.level.zip_level(),
    };

    let file_options = ZipFileOptions::default()
        .compression_method(zip_method)
        .compression_level(compression_level)
        .unix_permissions(unix_permissions);
    if let Some(password) = options.password() {
        file_options.with_aes_encryption(ZipAesMode::Aes256, password)
    } else {
        file_options
    }
}

fn zip_directory_options(options: &CompressionOptions, unix_permissions: u32) -> SimpleFileOptions {
    let compression_level = match options.zip_method {
        ZipCompressionMethod::Stored => None,
        ZipCompressionMethod::Zstd => Some(options.level.zstd_level()),
        ZipCompressionMethod::Deflate => Some(i64::from(options.level.zip_deflate_level())),
        _ => options.level.zip_level(),
    };

    SimpleFileOptions::default()
        .compression_method(options.zip_method.to_zip_method())
        .compression_level(compression_level)
        .unix_permissions(unix_permissions)
}

fn normalize_zip_file_path(path: &Path) -> Result<String> {
    normalize_zip_path(path, false)
}

fn normalize_zip_directory_path(path: &Path) -> Result<String> {
    normalize_zip_path(path, true)
}

fn normalize_tar_entry_path(path: &Path) -> Result<String> {
    normalize_zip_path(path, false)
}

fn normalize_zip_path(path: &Path, trailing_slash: bool) -> Result<String> {
    let sanitized = sanitize_relative_path(path);
    if sanitized.as_os_str().is_empty() {
        bail!("Archive entry path is empty");
    }

    let mut value = sanitized
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().replace('\\', "/")),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if value.is_empty() {
        bail!("Archive entry path is empty");
    }
    if trailing_slash && !value.ends_with('/') {
        value.push('/');
    }
    Ok(value)
}

pub(crate) fn resolve_output_path(
    relative: &Path,
    options: &ExtractOptions,
    plan: &ExtractPathPlan,
) -> Result<PathBuf> {
    if let Some(path) = plan.renamed_path(relative) {
        return Ok(path.clone());
    }

    if options.keep_paths {
        return Ok(options.output_dir.join(relative));
    }

    let file_name = relative.file_name().ok_or_else(|| {
        anyhow!(
            "Failed to derive a flat output file name from {}",
            relative.display()
        )
    })?;
    Ok(options.output_dir.join(file_name))
}

#[cfg(target_os = "windows")]
pub(crate) fn amsi_scan_data(file_name: &str, data: &[u8], options: &ExtractOptions) -> Result<()> {
    if !options.scan_files {
        return Ok(());
    }
    let session = crate::amsi::AmsiSession::new();
    if let Some(session) = session {
        match session.scan(file_name, data) {
            crate::amsi::AmsiResult::Detected => {
                anyhow::bail!(
                    "AMSI detected malware in file '{}' — extraction blocked",
                    file_name
                );
            }
            crate::amsi::AmsiResult::Clean => {}
        }
    }
    // If AMSI is unavailable (no session), silently pass — fail open
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn amsi_scan_data(
    _file_name: &str,
    _data: &[u8],
    _options: &ExtractOptions,
) -> Result<()> {
    Ok(())
}

pub(crate) fn prepare_output_file(
    destination: &Path,
    overwrite_mode: OverwriteMode,
    report: &mut ExtractionReport,
) -> Result<bool> {
    if let Some(parent) = destination.parent() {
        create_directory(parent, report)?;
    }

    if destination.exists() {
        match overwrite_mode {
            OverwriteMode::Overwrite => {
                if destination.is_dir() {
                    bail!(
                        "Cannot overwrite directory with file: {}",
                        destination.display()
                    );
                }
                Ok(true)
            }
            OverwriteMode::Skip => Ok(false),
            OverwriteMode::Error => bail!(
                "Refusing to overwrite existing file {}",
                destination.display()
            ),
        }
    } else {
        Ok(true)
    }
}

pub(crate) fn create_directory(path: &Path, report: &mut ExtractionReport) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }

    if path.exists() {
        if path.is_dir() {
            return Ok(());
        }
        bail!(
            "Expected a directory path, found a file at {}",
            path.display()
        );
    }

    fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory {}", path.display()))?;
    report.directories_created += 1;
    Ok(())
}

pub(crate) fn sanitize_relative_path(path: &Path) -> PathBuf {
    let mut sanitized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => sanitized.push(value),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {}
        }
    }
    sanitized
}

pub(crate) fn contains_unsafe_components(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    })
}

pub(crate) fn single_file_output_name(path: &Path, suffix: &str) -> Result<OsString> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("Archive path is missing a file name"))?;
    let file_name = file_name.to_string_lossy();

    if !file_name.to_ascii_lowercase().ends_with(suffix) {
        bail!(
            "Archive {} does not end with the expected suffix {}",
            path.display(),
            suffix
        );
    }

    let output_name = &file_name[..file_name.len() - suffix.len()];
    if output_name.is_empty() {
        bail!("Failed to derive an output name from {}", path.display());
    }

    Ok(OsString::from(output_name))
}

pub(crate) fn open_context(path: &Path) -> impl FnOnce() -> String + '_ {
    move || format!("Failed to open {}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Decompress as RawDeflateDecompressor;
    use flate2::FlushDecompress;
    use tempfile::tempdir;

    #[test]
    fn parallel_raw_deflate_chunks_round_trip() {
        let payload = vec![b'Z'; 24 * 1024 * 1024];
        let specs = parallel_deflate_chunk_specs(payload.len() as u64, 4);
        assert!(specs.len() > 1);

        let mut encoded = Vec::new();
        for spec in specs {
            let start = spec.offset as usize;
            let end = start + spec.input_len as usize;
            let dictionary_start = start.saturating_sub(ZIP_PARALLEL_DEFLATE_WINDOW_BYTES as usize);
            let dictionary =
                (dictionary_start < start).then_some(&payload[dictionary_start..start]);
            let chunk = compress_parallel_deflate_bytes(
                &payload[start..end],
                dictionary,
                spec.is_last,
                CompressionLevel::Fastest,
            )
            .unwrap();
            encoded.extend_from_slice(&chunk);
        }

        let mut decoder = RawDeflateDecompressor::new(false);
        let mut decoded = vec![0u8; payload.len()];
        let status = decoder
            .decompress(&encoded, &mut decoded, FlushDecompress::Finish)
            .unwrap();
        assert_eq!(status, FlateStatus::StreamEnd);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn writes_parallel_single_entry_zip_archives() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("payload.bin");
        let payload = vec![b'Q'; 40 * 1024 * 1024];
        fs::write(&source_path, &payload).unwrap();
        let zip_path = temp.path().join("parallel-single.zip");
        let prepared = PreparedZipFile {
            source_path: source_path.clone(),
            archive_path: "payload.bin".to_string(),
            archive_index: 0,
            input_len: payload.len() as u64,
            zip_method: ZipCompressionMethod::Deflate,
        };

        write_parallel_deflate_single_entry_zip(
            &zip_path,
            &prepared,
            &CompressionOptions {
                format: CompressionFormat::Zip,
                level: CompressionLevel::Fastest,
                zip_method: ZipCompressionMethod::Deflate,
                thread_count: 4,
                ..CompressionOptions::default()
            },
            &mut |_delta| {},
            &mut || false,
        )
        .unwrap();

        let file = File::open(&zip_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let mut entry = archive.by_name("payload.bin").unwrap();
        let mut decoded = Vec::new();
        entry.read_to_end(&mut decoded).unwrap();
        assert_eq!(decoded, payload);
        assert_eq!(entry.compression(), CompressionMethod::Deflated);
    }

    #[test]
    fn suggests_parent_output_dir_for_matching_wrapped_root() {
        let archive = Path::new(r"D:\demo\notes.zip");
        let entries = vec![
            ArchiveEntry {
                path: PathBuf::from("notes"),
                is_dir: true,
                uncompressed_size: None,
                compressed_size: None,
            },
            ArchiveEntry {
                path: PathBuf::from("notes/chapter1.txt"),
                is_dir: false,
                uncompressed_size: Some(12),
                compressed_size: Some(8),
            },
        ];

        assert_eq!(
            suggested_extract_output_dir(archive, &entries, true),
            PathBuf::from(r"D:\demo")
        );
    }

    #[test]
    fn keeps_default_output_dir_when_archive_is_not_wrapped_by_matching_root() {
        let archive = Path::new(r"D:\demo\notes.zip");
        let entries = vec![ArchiveEntry {
            path: PathBuf::from("chapter1.txt"),
            is_dir: false,
            uncompressed_size: Some(12),
            compressed_size: Some(8),
        }];

        assert_eq!(
            suggested_extract_output_dir(archive, &entries, true),
            PathBuf::from(r"D:\demo\notes")
        );
    }

    #[test]
    fn keeps_default_output_dir_for_flat_extraction() {
        let archive = Path::new(r"D:\demo\notes.zip");
        let entries = vec![ArchiveEntry {
            path: PathBuf::from("notes/chapter1.txt"),
            is_dir: false,
            uncompressed_size: Some(12),
            compressed_size: Some(8),
        }];

        assert_eq!(
            suggested_extract_output_dir(archive, &entries, false),
            PathBuf::from(r"D:\demo\notes")
        );
    }
}
