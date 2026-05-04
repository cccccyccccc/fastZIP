#![allow(dead_code, unused_imports)]

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

#[cfg(target_os = "windows")]
#[path = "../amsi.rs"]
mod amsi;
#[path = "../archive/mod.rs"]
mod archive;
#[path = "../benchmark.rs"]
mod benchmark;
#[path = "../encoding.rs"]
mod encoding;
#[path = "../hash.rs"]
mod hash;
#[path = "../settings.rs"]
mod settings;

use archive::{
    ArchiveFormat, ArchiveService, CompressionFormat, CompressionLevel, CompressionOptions,
    ExtractOptions, OverwriteMode, ZipCompressionMethod, all_supported_formats,
    parse_volume_size_spec, suggested_extract_output_dir,
};
use encoding::FilenameEncoding;

#[derive(Debug, Parser)]
#[command(
    name = "fastzip-cli",
    version,
    about = "A slim native archive CLI without GUI startup overhead",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Test {
        archive: PathBuf,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, value_enum, value_name = "FORMAT")]
        format: Option<ArchiveFormatArg>,
    },
    List {
        archive: PathBuf,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, value_enum, value_name = "FORMAT")]
        format: Option<ArchiveFormatArg>,
    },
    Extract {
        archive: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = OverwriteArg::Overwrite)]
        overwrite: OverwriteArg,
        #[arg(long)]
        flat: bool,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, value_enum, value_name = "FORMAT")]
        format: Option<ArchiveFormatArg>,
        #[arg(long, value_name = "CODEPAGE")]
        codepage: Option<String>,
        #[arg(long)]
        scan: bool,
    },
    Compress {
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = CompressionLevelArg::Normal)]
        level: CompressionLevelArg,
        #[arg(long, value_enum, default_value_t = ZipMethodArg::Deflate)]
        method: ZipMethodArg,
        #[arg(long, default_value_t = 1)]
        threads: u32,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        encrypt_file_names: bool,
        #[arg(long, value_name = "SIZE")]
        volume_size: Option<String>,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long, value_enum, value_name = "FORMAT")]
        format: Option<ArchiveFormatArg>,
        #[arg(long)]
        sfx: bool,
    },
    Formats,
    Backends,
    Checksum {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(long, value_enum, default_value_t = HashAlgoArg::Sha256)]
        algo: HashAlgoArg,
    },
    Benchmark {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OverwriteArg {
    Overwrite,
    Skip,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CompressionLevelArg {
    Fastest,
    Fast,
    Normal,
    Maximum,
    Ultra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ZipMethodArg {
    Deflate,
    Stored,
    Bzip2,
    Zstd,
    Xz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ArchiveFormatArg {
    #[value(name = "7z")]
    SevenZip,
    #[value(name = "zip")]
    Zip,
    #[value(name = "tar")]
    Tar,
    #[value(name = "tar.gz")]
    TarGz,
    #[value(name = "tar.bz2")]
    TarBz2,
    #[value(name = "tar.xz")]
    TarXz,
    #[value(name = "tar.zst")]
    TarZst,
    #[value(name = "tar.lz4")]
    TarLz4,
    #[value(name = "gz")]
    Gz,
    #[value(name = "bz2")]
    Bz2,
    #[value(name = "xz")]
    Xz,
    #[value(name = "zst")]
    Zst,
    #[value(name = "lz4")]
    Lz4,
    #[value(name = "rar")]
    Rar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum HashAlgoArg {
    #[value(name = "sha256")]
    Sha256,
    #[value(name = "blake3")]
    Blake3,
    #[value(name = "crc32")]
    Crc32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let service = ArchiveService::new();

    match cli.command {
        Command::Test {
            archive,
            password,
            format,
        } => {
            let report = if is_stdin_path(&archive) {
                let fmt = format.ok_or_else(|| {
                    anyhow::anyhow!("--format is required when reading archive from stdin")
                })?;
                service.test_archive_from_stdin(map_archive_format(fmt), password.as_deref())?
            } else {
                service.test_archive_with_password(&archive, password.as_deref())?
            };
            println!("archive: {}", report.archive_path.display());
            println!("format: {}", report.format);
            println!("entries tested: {}", report.entries_tested);
            println!("entries failed: {}", report.entries_failed);
            println!("bytes read: {}", report.bytes_read);
            println!("elapsed: {:?}", report.elapsed);
            if !report.errors.is_empty() {
                println!("errors:");
                for error in &report.errors {
                    println!("  - {error}");
                }
            }
            if report.is_healthy() {
                println!("result: OK");
            } else {
                println!("result: CORRUPT");
                std::process::exit(2);
            }
        }
        Command::List {
            archive,
            password,
            format,
        } => {
            if is_stdin_path(&archive) {
                let fmt = format.ok_or_else(|| {
                    anyhow::anyhow!("--format is required when reading archive from stdin")
                })?;
                let entries = service
                    .list_archive_from_stdin(map_archive_format(fmt), password.as_deref())?;
                println!("archive: <stdin>");
                println!("format: {}", map_archive_format(fmt));
                println!(
                    "{:<48} {:<6} {:>12} {:>12}",
                    "path", "type", "size", "packed"
                );
                println!("{}", "-".repeat(84));
                for entry in entries {
                    println!(
                        "{:<48} {:<6} {:>12} {:>12}",
                        entry.path.display(),
                        if entry.is_dir { "dir" } else { "file" },
                        entry
                            .uncompressed_size
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        entry
                            .compressed_size
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    );
                }
            } else {
                let inspection = service.inspect_archive(&archive)?;
                let entries = service.list_archive_with_password(&archive, password.as_deref())?;
                println!("archive: {}", archive.display());
                println!("format: {}", inspection.format);
                println!("backend: {}", inspection.backend_label);
                println!("backend status: {}", inspection.backend_detail);
                println!(
                    "{:<48} {:<6} {:>12} {:>12}",
                    "path", "type", "size", "packed"
                );
                println!("{}", "-".repeat(84));
                for entry in entries {
                    println!(
                        "{:<48} {:<6} {:>12} {:>12}",
                        entry.path.display(),
                        if entry.is_dir { "dir" } else { "file" },
                        entry
                            .uncompressed_size
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        entry
                            .compressed_size
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    );
                }
            }
        }
        Command::Extract {
            archive,
            output,
            overwrite,
            flat,
            password,
            format,
            codepage,
            scan,
        } => {
            let filename_encoding = parse_codepage(codepage.as_deref());
            if is_stdin_path(&archive) {
                let fmt = format.ok_or_else(|| {
                    anyhow::anyhow!("--format is required when reading archive from stdin")
                })?;
                let output_dir = output.unwrap_or_else(|| PathBuf::from("."));
                let report = service.extract_archive_from_stdin(
                    map_archive_format(fmt),
                    &ExtractOptions {
                        output_dir: output_dir.clone(),
                        overwrite_mode: map_overwrite_mode(overwrite),
                        keep_paths: !flat,
                        password,
                        filename_encoding,
                        scan_files: scan,
                    },
                )?;
                println!("archive: <stdin>");
                println!("output: {}", report.output_dir.display());
                println!("files written: {}", report.files_written);
                println!("directories created: {}", report.directories_created);
            } else {
                let inspection = service.inspect_archive(&archive)?;
                let output_dir = if let Some(output_dir) = output {
                    output_dir
                } else if flat {
                    inspection.suggested_output_dir.clone()
                } else {
                    service
                        .list_archive_with_password(&archive, password.as_deref())
                        .map(|entries| suggested_extract_output_dir(&archive, &entries, true))
                        .unwrap_or_else(|_| inspection.suggested_output_dir.clone())
                };
                let report = service.extract_archive(
                    &archive,
                    &ExtractOptions {
                        output_dir: output_dir.clone(),
                        overwrite_mode: map_overwrite_mode(overwrite),
                        keep_paths: !flat,
                        password,
                        filename_encoding,
                        scan_files: scan,
                    },
                )?;
                println!("archive: {}", archive.display());
                println!("output: {}", report.output_dir.display());
                println!("files written: {}", report.files_written);
                println!("directories created: {}", report.directories_created);
            }
        }
        Command::Compress {
            inputs,
            output,
            level,
            method,
            threads,
            password,
            encrypt_file_names,
            volume_size,
            preset,
            format,
            sfx,
        } => {
            let split_volume_size = volume_size
                .as_deref()
                .map(parse_volume_size_spec)
                .transpose()?;
            let (
                mut preset_format,
                mut preset_level,
                mut preset_method,
                mut preset_threads,
                mut preset_encrypt,
            ) = (None, None, None, None, None);
            if let Some(ref preset_name) = preset {
                if let Some(preset_value) = settings::load_preset(preset_name) {
                    match settings::decode_preset_value(&preset_value) {
                        Ok((fmt, lvl, meth, thr, enc)) => {
                            preset_format = Some(fmt);
                            preset_level = Some(lvl);
                            preset_method = Some(meth);
                            preset_threads = Some(thr);
                            preset_encrypt = Some(enc);
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to decode preset '{preset_name}': {e:#}")
                        }
                    }
                } else {
                    eprintln!("Warning: Preset '{preset_name}' not found");
                }
            }
            let detected_format = if is_stdout_path(&output) {
                format
                    .map(map_compression_format)
                    .or(preset_format)
                    .unwrap_or(CompressionFormat::Zip)
            } else {
                preset_format.unwrap_or_else(|| {
                    format.map(map_compression_format).unwrap_or_else(|| {
                        CompressionFormat::detect(&output).unwrap_or(CompressionFormat::Zip)
                    })
                })
            };
            let options = CompressionOptions {
                format: detected_format,
                level: preset_level.unwrap_or_else(|| map_compression_level(level)),
                zip_method: preset_method.unwrap_or_else(|| map_zip_method(method)),
                thread_count: preset_threads.unwrap_or(threads).max(1),
                password,
                encrypt_file_names: preset_encrypt.unwrap_or(encrypt_file_names),
                split_volume_size,
                sfx,
            };
            if is_stdout_path(&output) {
                let report = service.compress_to_stdout(&inputs, options)?;
                eprintln!("archive: <stdout>");
                eprintln!("files added: {}", report.files_added);
                eprintln!("directories added: {}", report.directories_added);
                eprintln!("input bytes: {}", report.input_bytes);
                eprintln!("output bytes: {}", report.output_bytes);
            } else {
                let report = service.compress_with_options(&inputs, &output, options)?;
                println!("archive: {}", report.archive_path.display());
                println!("files added: {}", report.files_added);
                println!("directories added: {}", report.directories_added);
                println!("input bytes: {}", report.input_bytes);
                println!("output bytes: {}", report.output_bytes);
            }
        }
        Command::Formats => {
            for suffix in all_supported_formats() {
                println!("{suffix}");
            }
        }
        Command::Backends => {
            for backend in service.backend_statuses() {
                println!("backend: {}", backend.label);
                println!("available: {}", backend.available);
                println!("detail: {}", backend.detail);
                println!("formats: {}", backend.formats.join(", "));
                println!();
            }
        }
        Command::Checksum { files, algo } => {
            use hash::{HashAlgorithm, file_checksum};
            let algorithm = match algo {
                HashAlgoArg::Sha256 => HashAlgorithm::Sha256,
                HashAlgoArg::Blake3 => HashAlgorithm::Blake3,
                HashAlgoArg::Crc32 => HashAlgorithm::Crc32,
            };
            for file in &files {
                match file_checksum(file, algorithm) {
                    Ok(result) => {
                        println!(
                            "{}\n  {:<10} {}\n  {:<10} {} bytes\n  {:<10} {:?}",
                            file.display(),
                            "digest:",
                            result.hex_digest,
                            "size:",
                            result.file_size,
                            "time:",
                            result.elapsed,
                        );
                    }
                    Err(e) => eprintln!("{}: {e:#}", file.display()),
                }
            }
        }
        Command::Benchmark { output } => {
            let out_dir = output.unwrap_or_else(|| PathBuf::from("benchmark_results"));
            let entries = benchmark::run_benchmark(&out_dir)?;
            println!(
                "{:<12} {:<10} {:<12} {:<14} {:<14} {:<12}",
                "format", "level", "input", "output", "ratio", "throughput"
            );
            println!("{}", "-".repeat(82));
            for entry in &entries {
                println!(
                    "{:<12} {:<10} {:<12} {:<14} {:<14.4} {:<10.2} MB/s",
                    format!("{:?}", entry.format),
                    format!("{:?}", entry.level),
                    entry.input_bytes,
                    entry.output_bytes,
                    entry.compression_ratio(),
                    entry.throughput_mbps(),
                );
            }
            println!("\nResults saved to: {}", out_dir.display());
        }
    }

    Ok(())
}

fn map_overwrite_mode(value: OverwriteArg) -> OverwriteMode {
    match value {
        OverwriteArg::Overwrite => OverwriteMode::Overwrite,
        OverwriteArg::Skip => OverwriteMode::Skip,
        OverwriteArg::Error => OverwriteMode::Error,
    }
}

fn map_compression_level(value: CompressionLevelArg) -> CompressionLevel {
    match value {
        CompressionLevelArg::Fastest => CompressionLevel::Fastest,
        CompressionLevelArg::Fast => CompressionLevel::Fast,
        CompressionLevelArg::Normal => CompressionLevel::Normal,
        CompressionLevelArg::Maximum => CompressionLevel::Maximum,
        CompressionLevelArg::Ultra => CompressionLevel::Ultra,
    }
}

fn map_zip_method(value: ZipMethodArg) -> ZipCompressionMethod {
    match value {
        ZipMethodArg::Deflate => ZipCompressionMethod::Deflate,
        ZipMethodArg::Stored => ZipCompressionMethod::Stored,
        ZipMethodArg::Bzip2 => ZipCompressionMethod::Bzip2,
        ZipMethodArg::Zstd => ZipCompressionMethod::Zstd,
        ZipMethodArg::Xz => ZipCompressionMethod::Xz,
    }
}

fn parse_codepage(value: Option<&str>) -> FilenameEncoding {
    value
        .and_then(|code| FilenameEncoding::from_code(code))
        .unwrap_or(FilenameEncoding::Utf8)
}

fn map_archive_format(value: ArchiveFormatArg) -> ArchiveFormat {
    match value {
        ArchiveFormatArg::SevenZip => ArchiveFormat::SevenZip,
        ArchiveFormatArg::Zip => ArchiveFormat::Zip,
        ArchiveFormatArg::Tar => ArchiveFormat::Tar,
        ArchiveFormatArg::TarGz => ArchiveFormat::TarGz,
        ArchiveFormatArg::TarBz2 => ArchiveFormat::TarBz2,
        ArchiveFormatArg::TarXz => ArchiveFormat::TarXz,
        ArchiveFormatArg::TarZst => ArchiveFormat::TarZst,
        ArchiveFormatArg::TarLz4 => ArchiveFormat::TarLz4,
        ArchiveFormatArg::Gz => ArchiveFormat::Gz,
        ArchiveFormatArg::Bz2 => ArchiveFormat::Bz2,
        ArchiveFormatArg::Xz => ArchiveFormat::Xz,
        ArchiveFormatArg::Zst => ArchiveFormat::Zst,
        ArchiveFormatArg::Lz4 => ArchiveFormat::Lz4,
        ArchiveFormatArg::Rar => ArchiveFormat::Rar,
    }
}

fn map_compression_format(value: ArchiveFormatArg) -> CompressionFormat {
    match value {
        ArchiveFormatArg::SevenZip => CompressionFormat::SevenZip,
        ArchiveFormatArg::Zip => CompressionFormat::Zip,
        ArchiveFormatArg::Tar => CompressionFormat::Tar,
        ArchiveFormatArg::TarGz => CompressionFormat::TarGz,
        ArchiveFormatArg::TarBz2 => CompressionFormat::TarBz2,
        ArchiveFormatArg::TarXz => CompressionFormat::TarXz,
        ArchiveFormatArg::TarZst => CompressionFormat::TarZst,
        ArchiveFormatArg::TarLz4 => CompressionFormat::TarLz4,
        ArchiveFormatArg::Gz => CompressionFormat::Gz,
        ArchiveFormatArg::Bz2 => CompressionFormat::Bz2,
        ArchiveFormatArg::Xz => CompressionFormat::Xz,
        ArchiveFormatArg::Zst => CompressionFormat::Zst,
        ArchiveFormatArg::Lz4 => CompressionFormat::Lz4,
        ArchiveFormatArg::Rar => CompressionFormat::Zip,
    }
}

fn is_stdin_path(path: &PathBuf) -> bool {
    path.as_os_str() == "-"
}

fn is_stdout_path(path: &PathBuf) -> bool {
    path.as_os_str() == "-"
}
