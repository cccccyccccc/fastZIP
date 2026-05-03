use std::hint::black_box;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use bzip2::Compression as BzCompression;
use bzip2::write::BzEncoder;
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use fastzip::archive::{ArchiveService, ExtractOptions, OverwriteMode};
use flate2::Compression as GzCompression;
use flate2::write::GzEncoder;
use tar::Builder;
use tempfile::{TempDir, tempdir};
use xz2::write::XzEncoder;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

const SMALL_FILE_COUNT: usize = 96;
const SMALL_FILE_SIZE: usize = 64 * 1024;
const SINGLE_FILE_SIZE: usize = 8 * 1024 * 1024;

struct ArchiveFixture {
    _temp: TempDir,
    zip_path: PathBuf,
    tar_gz_path: PathBuf,
    tar_bz2_path: PathBuf,
    tar_xz_path: PathBuf,
    gz_path: PathBuf,
    total_bytes: u64,
    entry_count: usize,
    single_file_bytes: u64,
}

fn benchmark_config() -> Criterion {
    Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(6))
}

fn archive_list_benchmarks(c: &mut Criterion) {
    let fixture = create_fixture();
    let service = ArchiveService::new();
    let mut group = c.benchmark_group("archive_list");
    group.throughput(Throughput::Elements(fixture.entry_count as u64));

    group.bench_function("zip_small_files", |b| {
        b.iter(|| {
            let entries = service.list_archive(black_box(&fixture.zip_path)).unwrap();
            black_box(entries.len())
        });
    });

    group.bench_function("tar_gz_small_files", |b| {
        b.iter(|| {
            let entries = service
                .list_archive(black_box(&fixture.tar_gz_path))
                .unwrap();
            black_box(entries.len())
        });
    });

    group.bench_function("tar_bz2_small_files", |b| {
        b.iter(|| {
            let entries = service
                .list_archive(black_box(&fixture.tar_bz2_path))
                .unwrap();
            black_box(entries.len())
        });
    });

    group.bench_function("tar_xz_small_files", |b| {
        b.iter(|| {
            let entries = service
                .list_archive(black_box(&fixture.tar_xz_path))
                .unwrap();
            black_box(entries.len())
        });
    });

    group.finish();
}

fn archive_extract_benchmarks(c: &mut Criterion) {
    let fixture = create_fixture();
    let service = ArchiveService::new();
    let mut group = c.benchmark_group("archive_extract");
    group.throughput(Throughput::Bytes(fixture.total_bytes));

    group.bench_function("zip_small_files", |b| {
        b.iter_batched(
            || tempdir().unwrap(),
            |output| {
                let report = service
                    .extract_archive(
                        black_box(&fixture.zip_path),
                        &ExtractOptions {
                            output_dir: output.path().join("zip-out"),
                            overwrite_mode: OverwriteMode::Overwrite,
                            keep_paths: true,
                        },
                    )
                    .unwrap();
                black_box(report.files_written)
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("tar_gz_small_files", |b| {
        b.iter_batched(
            || tempdir().unwrap(),
            |output| {
                let report = service
                    .extract_archive(
                        black_box(&fixture.tar_gz_path),
                        &ExtractOptions {
                            output_dir: output.path().join("tar-gz-out"),
                            overwrite_mode: OverwriteMode::Overwrite,
                            keep_paths: true,
                        },
                    )
                    .unwrap();
                black_box(report.files_written)
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("tar_bz2_small_files", |b| {
        b.iter_batched(
            || tempdir().unwrap(),
            |output| {
                let report = service
                    .extract_archive(
                        black_box(&fixture.tar_bz2_path),
                        &ExtractOptions {
                            output_dir: output.path().join("tar-bz2-out"),
                            overwrite_mode: OverwriteMode::Overwrite,
                            keep_paths: true,
                        },
                    )
                    .unwrap();
                black_box(report.files_written)
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("tar_xz_small_files", |b| {
        b.iter_batched(
            || tempdir().unwrap(),
            |output| {
                let report = service
                    .extract_archive(
                        black_box(&fixture.tar_xz_path),
                        &ExtractOptions {
                            output_dir: output.path().join("tar-xz-out"),
                            overwrite_mode: OverwriteMode::Overwrite,
                            keep_paths: true,
                        },
                    )
                    .unwrap();
                black_box(report.files_written)
            },
            BatchSize::PerIteration,
        );
    });

    group.finish();

    let mut single_group = c.benchmark_group("single_file_extract");
    single_group.throughput(Throughput::Bytes(fixture.single_file_bytes));

    single_group.bench_function("gz_large_single_file", |b| {
        b.iter_batched(
            || tempdir().unwrap(),
            |output| {
                let report = service
                    .extract_archive(
                        black_box(&fixture.gz_path),
                        &ExtractOptions {
                            output_dir: output.path().join("gz-out"),
                            overwrite_mode: OverwriteMode::Overwrite,
                            keep_paths: true,
                        },
                    )
                    .unwrap();
                black_box(report.files_written)
            },
            BatchSize::PerIteration,
        );
    });

    single_group.finish();
}

criterion_group!(
    name = benches;
    config = benchmark_config();
    targets = archive_list_benchmarks, archive_extract_benchmarks
);
criterion_main!(benches);

fn create_fixture() -> ArchiveFixture {
    let temp = tempdir().unwrap();
    let source_files = make_source_files(SMALL_FILE_COUNT, SMALL_FILE_SIZE);
    let total_bytes = source_files
        .iter()
        .map(|(_, bytes)| bytes.len() as u64)
        .sum();

    let zip_path = temp.path().join("fixture.zip");
    let tar_gz_path = temp.path().join("fixture.tar.gz");
    let tar_bz2_path = temp.path().join("fixture.tar.bz2");
    let tar_xz_path = temp.path().join("fixture.tar.xz");
    let gz_path = temp.path().join("fixture.bin.gz");

    create_zip_archive(&zip_path, &source_files);
    create_tar_gz_archive(&tar_gz_path, &source_files);
    create_tar_bz2_archive(&tar_bz2_path, &source_files);
    create_tar_xz_archive(&tar_xz_path, &source_files);

    let single_payload = make_payload(0x00C0_FFEE_F00D_u64, SINGLE_FILE_SIZE);
    create_gz_file(&gz_path, &single_payload);

    ArchiveFixture {
        _temp: temp,
        zip_path,
        tar_gz_path,
        tar_bz2_path,
        tar_xz_path,
        gz_path,
        total_bytes,
        entry_count: source_files.len(),
        single_file_bytes: single_payload.len() as u64,
    }
}

fn make_source_files(file_count: usize, file_size: usize) -> Vec<(String, Vec<u8>)> {
    (0..file_count)
        .map(|index| {
            let folder = index % 8;
            let file_name = format!("dataset/folder-{folder:02}/file-{index:03}.bin");
            let seed = 0x9E37_79B9_7F4A_7C15_u64 ^ index as u64;
            (file_name, make_payload(seed, file_size))
        })
        .collect()
}

fn make_payload(mut state: u64, len: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; len];
    for byte in &mut buffer {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = (state as u8) ^ ((state >> 8) as u8) ^ ((state >> 16) as u8);
    }
    buffer
}

fn create_zip_archive(path: &Path, files: &[(String, Vec<u8>)]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for (entry_name, bytes) in files {
        writer.start_file(entry_name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }

    writer.finish().unwrap();
}

fn create_tar_gz_archive(path: &Path, files: &[(String, Vec<u8>)]) {
    let file = std::fs::File::create(path).unwrap();
    let encoder = GzEncoder::new(file, GzCompression::default());
    let mut builder = Builder::new(encoder);
    append_tar_entries(&mut builder, files);
    builder.finish().unwrap();
}

fn create_tar_bz2_archive(path: &Path, files: &[(String, Vec<u8>)]) {
    let file = std::fs::File::create(path).unwrap();
    let encoder = BzEncoder::new(file, BzCompression::default());
    let mut builder = Builder::new(encoder);
    append_tar_entries(&mut builder, files);
    builder.finish().unwrap();
}

fn create_tar_xz_archive(path: &Path, files: &[(String, Vec<u8>)]) {
    let file = std::fs::File::create(path).unwrap();
    let encoder = XzEncoder::new(file, 6);
    let mut builder = Builder::new(encoder);
    append_tar_entries(&mut builder, files);
    builder.finish().unwrap();
}

fn append_tar_entries<W: Write>(builder: &mut Builder<W>, files: &[(String, Vec<u8>)]) {
    for (entry_name, bytes) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, entry_name, bytes.as_slice())
            .unwrap();
    }
}

fn create_gz_file(path: &Path, bytes: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = GzEncoder::new(file, GzCompression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap();
}
