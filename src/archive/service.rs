use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::{
    ArchiveEntry, ArchiveFormat, BackendKind, CompressionOptions, CompressionReport,
    ExtractOptions, ExtractPathPlan, ExtractionReport, FilenameEncoding, default_output_dir,
    native::NativeBackend, rar::RarBackend, test,
};
use super::test::TestReport;

#[derive(Debug, Clone)]
pub struct BackendStatus {
    pub kind: BackendKind,
    pub label: String,
    pub available: bool,
    pub detail: String,
    pub formats: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ArchiveInspection {
    pub archive_path: PathBuf,
    pub format: ArchiveFormat,
    pub backend_kind: BackendKind,
    pub backend_label: String,
    pub backend_available: bool,
    pub backend_detail: String,
    pub suggested_output_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ArchiveService {
    native: NativeBackend,
    rar: RarBackend,
}

impl Default for ArchiveService {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveService {
    pub fn new() -> Self {
        Self {
            native: NativeBackend,
            rar: RarBackend::new(),
        }
    }

    pub fn backend_statuses(&self) -> Vec<BackendStatus> {
        vec![self.native.status(), self.rar.status()]
    }

    pub fn inspect_archive(&self, path: &Path) -> Result<ArchiveInspection> {
        let format = ArchiveFormat::detect(path)?;
        let status = match format {
            ArchiveFormat::Rar => self.rar.status(),
            _ => self.native.status(),
        };

        Ok(ArchiveInspection {
            archive_path: path.to_path_buf(),
            format,
            backend_kind: status.kind,
            backend_label: status.label,
            backend_available: status.available,
            backend_detail: status.detail,
            suggested_output_dir: default_output_dir(path),
        })
    }

    pub fn list_archive(&self, path: &Path) -> Result<Vec<ArchiveEntry>> {
        self.list_archive_with_password(path, None)
    }

    pub fn list_archive_with_password(
        &self,
        path: &Path,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>> {
        self.list_archive_with_password_and_encoding(path, password, FilenameEncoding::Utf8)
    }

    pub fn list_archive_with_password_and_encoding(
        &self,
        path: &Path,
        password: Option<&str>,
        encoding: FilenameEncoding,
    ) -> Result<Vec<ArchiveEntry>> {
        let format = ArchiveFormat::detect(path)?;
        match format {
            ArchiveFormat::Rar => self.rar.list_archive(path),
            _ => self
                .native
                .list_archive_with_password(path, format, password, encoding),
        }
    }

    pub fn test_archive(&self, path: &Path) -> Result<TestReport> {
        self.test_archive_with_password(path, None)
    }

    pub fn test_archive_with_password(
        &self,
        path: &Path,
        password: Option<&str>,
    ) -> Result<TestReport> {
        test::test_archive(path, password)
    }

    pub fn extract_archive(
        &self,
        path: &Path,
        options: &ExtractOptions,
    ) -> Result<ExtractionReport> {
        let plan = ExtractPathPlan::default();
        self.extract_archive_with_progress_and_cancel_with_plan(
            path,
            options,
            &plan,
            &mut |_delta| {},
            &mut || false,
        )
    }

    pub fn extract_archive_with_progress<F>(
        &self,
        path: &Path,
        options: &ExtractOptions,
        mut progress: F,
    ) -> Result<ExtractionReport>
    where
        F: FnMut(u64),
    {
        let plan = ExtractPathPlan::default();
        self.extract_archive_with_progress_and_cancel_with_plan(
            path,
            options,
            &plan,
            &mut progress,
            &mut || false,
        )
    }

    pub fn extract_archive_with_progress_and_cancel<F, C>(
        &self,
        path: &Path,
        options: &ExtractOptions,
        progress: &mut F,
        should_cancel: &mut C,
    ) -> Result<ExtractionReport>
    where
        F: FnMut(u64),
        C: FnMut() -> bool,
    {
        let plan = ExtractPathPlan::default();
        self.extract_archive_with_progress_and_cancel_with_plan(
            path,
            options,
            &plan,
            progress,
            should_cancel,
        )
    }

    pub fn extract_archive_with_progress_and_cancel_with_plan<F, C>(
        &self,
        path: &Path,
        options: &ExtractOptions,
        plan: &ExtractPathPlan,
        progress: &mut F,
        should_cancel: &mut C,
    ) -> Result<ExtractionReport>
    where
        F: FnMut(u64),
        C: FnMut() -> bool,
    {
        let format = ArchiveFormat::detect(path)?;
        match format {
            ArchiveFormat::Rar => self.rar.extract_archive_with_progress_and_cancel(
                path,
                options,
                plan,
                progress,
                should_cancel,
            ),
            _ => self.native.extract_archive_with_progress_and_cancel(
                path,
                format,
                options,
                plan,
                progress,
                should_cancel,
            ),
        }
    }

    pub fn compress_with_options(
        &self,
        sources: &[PathBuf],
        output_path: &Path,
        options: CompressionOptions,
    ) -> Result<CompressionReport> {
        if options.sfx {
            self.compress_sfx(sources, &[], output_path, options, &mut |_| {}, &mut || false)
        } else {
            self.native.compress_with_options_and_exclusions_and_progress_and_cancel(
                sources,
                &[],
                output_path,
                options,
                &mut |_delta| {},
                &mut || false,
            )
        }
    }

    pub fn compress_with_options_and_exclusions_and_progress_and_cancel<F, C>(
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
        if options.sfx {
            self.compress_sfx(sources, excluded_paths, output_path, options, progress, should_cancel)
        } else {
            self.native
                .compress_with_options_and_exclusions_and_progress_and_cancel(
                    sources,
                    excluded_paths,
                    output_path,
                    options,
                    progress,
                    should_cancel,
                )
        }
    }

    fn compress_sfx<F, C>(
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
        let temp_file = tempfile::Builder::new()
            .suffix(options.format.default_extension())
            .tempfile()
            .context("Failed to create temp file for SFX compression")?;
        let temp_path = temp_file.path().to_path_buf();

        let format = options.format;
        let report = self.native
            .compress_with_options_and_exclusions_and_progress_and_cancel(
                sources,
                excluded_paths,
                &temp_path,
                CompressionOptions { sfx: false, ..options },
                progress,
                should_cancel,
            )?;

        // Wrap the compressed archive into an SFX executable
        super::sfx::wrap_sfx(&temp_path, output_path, format)?;

        Ok(CompressionReport {
            archive_path: output_path.to_path_buf(),
            ..report
        })
    }

    /// Read an archive from stdin and list its entries.
    /// `format` is required because stdin has no file extension for detection.
    pub fn list_archive_from_stdin(
        &self,
        format: ArchiveFormat,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>> {
        let temp_file = read_stdin_to_temp(format)?;
        match format {
            ArchiveFormat::Rar => self.rar.list_archive(temp_file.path()),
            _ => self
                .native
                .list_archive_with_password(
                    temp_file.path(),
                    format,
                    password,
                    FilenameEncoding::Utf8,
                ),
        }
    }

    /// Read an archive from stdin and test its integrity.
    pub fn test_archive_from_stdin(
        &self,
        format: ArchiveFormat,
        password: Option<&str>,
    ) -> Result<TestReport> {
        let temp_file = read_stdin_to_temp(format)?;
        test::test_archive(temp_file.path(), password)
    }

    /// Read an archive from stdin and extract it.
    pub fn extract_archive_from_stdin(
        &self,
        format: ArchiveFormat,
        options: &ExtractOptions,
    ) -> Result<ExtractionReport> {
        let temp_file = read_stdin_to_temp(format)?;
        let plan = ExtractPathPlan::default();
        match format {
            ArchiveFormat::Rar => self.rar.extract_archive_with_progress_and_cancel(
                temp_file.path(),
                options,
                &plan,
                &mut |_| {},
                &mut || false,
            ),
            _ => self.native.extract_archive_with_progress_and_cancel(
                temp_file.path(),
                format,
                options,
                &plan,
                &mut |_| {},
                &mut || false,
            ),
        }
    }

    /// Compress sources and write the result to stdout.
    pub fn compress_to_stdout(
        &self,
        sources: &[PathBuf],
        options: CompressionOptions,
    ) -> Result<CompressionReport> {
        let temp_file = tempfile::Builder::new()
            .suffix(options.format.default_extension())
            .tempfile()
            .context("Failed to create temp file for stdout compression")?;
        let report = self.native
            .compress_with_options_and_exclusions_and_progress_and_cancel(
                sources,
                &[],
                temp_file.path(),
                options,
                &mut |_| {},
                &mut || false,
            )?;
        let mut stdout = io::stdout();
        let mut file = std::fs::File::open(temp_file.path())?;
        io::copy(&mut file, &mut stdout)?;
        stdout.flush()?;
        Ok(report)
    }

}

/// Read stdin into a NamedTempFile with the appropriate extension for format detection.
fn read_stdin_to_temp(format: ArchiveFormat) -> Result<tempfile::NamedTempFile> {
    let mut data = Vec::new();
    io::stdin()
        .read_to_end(&mut data)
        .context("Failed to read archive from stdin")?;
    let mut temp_file = tempfile::Builder::new()
        .suffix(format.default_extension())
        .tempfile()
        .context("Failed to create temp file for stdin archive")?;
    temp_file
        .write_all(&data)
        .context("Failed to write stdin data to temp file")?;
    temp_file
        .flush()
        .context("Failed to flush temp file")?;
    Ok(temp_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspection_routes_rar_to_the_separate_adapter() {
        let service = ArchiveService::new();
        let inspection = service
            .inspect_archive(Path::new("D:\\archives\\sample.rar"))
            .unwrap();

        assert_eq!(inspection.format, ArchiveFormat::Rar);
        assert_eq!(inspection.backend_kind, BackendKind::RarAdapter);
    }
}
