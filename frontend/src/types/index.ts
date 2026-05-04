// ── Archive types (mirrors Rust serde derives) ──────────────────────

export type ArchiveFormat =
  | "seven_zip" | "zip" | "tar" | "tar_gz" | "tar_bz2" | "tar_xz"
  | "gz" | "bz2" | "xz" | "zst" | "tar_zst" | "lz4" | "tar_lz4"
  | "rar" | "wim" | "iso";

export type BackendKind = "native" | "rar_adapter";

export type CompressionFormat =
  | "seven_zip" | "zip" | "tar" | "tar_gz" | "tar_bz2" | "tar_xz"
  | "gz" | "bz2" | "xz" | "zst" | "tar_zst" | "lz4" | "tar_lz4";

export type CompressionLevel = "fastest" | "fast" | "normal" | "maximum" | "ultra";

export type ZipCompressionMethod = "deflate" | "stored" | "bzip2" | "zstd" | "xz";

export type OverwriteMode = "overwrite" | "skip" | "error";

export type FilenameEncoding = "utf8" | "auto" | "shift_jis" | "gbk" | "euc_kr" | "windows1251";

export type HashAlgorithm = "crc32" | "sha256" | "blake3";

export interface ArchiveEntry {
  path: string;
  is_dir: boolean;
  uncompressed_size: number | null;
  compressed_size: number | null;
}

export interface ArchiveInspection {
  archive_path: string;
  format: ArchiveFormat;
  backend_kind: BackendKind;
  backend_label: string;
  backend_available: boolean;
  backend_detail: string;
  suggested_output_dir: string;
}

export interface BackendStatus {
  kind: BackendKind;
  label: string;
  available: boolean;
  detail: string;
  formats: string[];
}

export interface ExtractOptions {
  output_dir: string;
  overwrite_mode: OverwriteMode;
  keep_paths: boolean;
  password: string | null;
  filename_encoding: FilenameEncoding;
  scan_files: boolean;
}

export interface ExtractionReport {
  output_dir: string;
  files_written: number;
  directories_created: number;
}

export interface CompressionOptions {
  format: CompressionFormat;
  level: CompressionLevel;
  zip_method: ZipCompressionMethod;
  thread_count: number;
  password: string | null;
  encrypt_file_names: boolean;
  split_volume_size: number | null;
  sfx: boolean;
}

export interface CompressionReport {
  archive_path: string;
  files_added: number;
  directories_added: number;
  input_bytes: number;
  output_bytes: number;
}

export interface TestReport {
  archive_path: string;
  format: ArchiveFormat;
  entries_tested: number;
  entries_failed: number;
  bytes_read: number;
  elapsed: number; // seconds (f64)
  errors: string[];
  is_healthy?: boolean;
}

export interface ChecksumResult {
  algorithm: HashAlgorithm;
  hex_digest: string;
  file_size: number;
  elapsed: number;
}

export interface BenchmarkEntry {
  format: CompressionFormat;
  level: CompressionLevel;
  input_bytes: number;
  output_bytes: number;
  elapsed: number;
  compression_ratio?: number;
  throughput_mbps?: number;
}

export interface ReleaseInfo {
  version: string;
  download_url: string;
  body: string;
}

export interface FileInfo {
  path: string;
  name: string;
  is_dir: boolean;
  size: number;
  modified_secs: number;
}

// ── Task event payloads ─────────────────────────────────────────────

export interface TaskProgressEvent {
  task_id: number;
  bytes_processed: number;
  total_bytes: number;
  elapsed: number;
  speed_mbps: number;
}

export interface TaskCompletedEvent {
  task_id: number;
}

export interface TaskFailedEvent {
  task_id: number;
  error: string;
}

export interface TaskCanceledEvent {
  task_id: number;
}

// ── Locale ──────────────────────────────────────────────────────────

export interface AppLocale {
  code: string;
  name_en: string;
  name_zh: string;
}
