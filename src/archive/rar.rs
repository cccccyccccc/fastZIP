use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};

use super::{
    ArchiveEntry, BackendKind, BackendStatus, ExtractOptions, ExtractPathPlan, ExtractionReport,
    OverwriteMode, contains_unsafe_components, create_directory, prepare_output_file,
    resolve_output_path, sanitize_relative_path,
};

const RAR_ENV_VAR: &str = "FASTZIP_RAR_TOOL";

#[derive(Debug, Clone, Default)]
pub(crate) struct RarBackend {
    executable: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct RarListedEntry {
    relative_path: PathBuf,
    is_dir: bool,
}

impl RarBackend {
    pub(crate) fn new() -> Self {
        Self {
            executable: discover_rar_executable(),
        }
    }

    pub(crate) fn status(&self) -> BackendStatus {
        match &self.executable {
            Some(path) => BackendStatus {
                kind: BackendKind::RarAdapter,
                label: BackendKind::RarAdapter.label().to_string(),
                available: true,
                detail: format!("Ready. Using {}.", path.display()),
                formats: vec![".rar".to_string()],
            },
            None => BackendStatus {
                kind: BackendKind::RarAdapter,
                label: BackendKind::RarAdapter.label().to_string(),
                available: false,
                detail: format!(
                    "Not configured. Install UnRAR/WinRAR or set {}.",
                    RAR_ENV_VAR
                ),
                formats: vec![".rar".to_string()],
            },
        }
    }

    pub(crate) fn list_archive(&self, path: &Path) -> Result<Vec<ArchiveEntry>> {
        let entries = self.list_entries(path)?;
        Ok(entries
            .into_iter()
            .map(|entry| ArchiveEntry {
                path: entry.relative_path,
                is_dir: entry.is_dir,
                uncompressed_size: None,
                compressed_size: None,
            })
            .collect())
    }

    pub(crate) fn extract_archive_with_progress_and_cancel<F, C>(
        &self,
        path: &Path,
        options: &ExtractOptions,
        plan: &ExtractPathPlan,
        _progress: &mut F,
        should_cancel: &mut C,
    ) -> Result<ExtractionReport>
    where
        F: FnMut(u64),
        C: FnMut() -> bool,
    {
        if !plan.is_empty() {
            return self.extract_archive_with_plan(path, options, plan, _progress, should_cancel);
        }

        fs::create_dir_all(&options.output_dir).with_context(|| {
            format!(
                "Failed to create output directory {}",
                options.output_dir.display()
            )
        })?;

        let entries = self.list_entries(path)?;
        self.validate_overwrite_mode(&entries, options, plan)?;

        let executable = self.require_executable()?;
        let mode = if options.keep_paths { "x" } else { "e" };
        let overwrite = match options.overwrite_mode {
            OverwriteMode::Overwrite => "-o+",
            OverwriteMode::Skip | OverwriteMode::Error => "-o-",
        };
        let output_dir = ensure_trailing_separator(&options.output_dir);

        let mut child = Command::new(executable)
            .arg(mode)
            .arg(overwrite)
            .arg("-idq")
            .arg(path.as_os_str())
            .arg(output_dir.as_os_str())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to invoke RAR adapter for {}", path.display()))?;

        loop {
            if should_cancel() {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Operation canceled");
            }

            if child
                .try_wait()
                .with_context(|| format!("Failed to wait for {}", path.display()))?
                .is_some()
            {
                break;
            }

            thread::sleep(Duration::from_millis(80));
        }

        let output = child.wait_with_output().with_context(|| {
            format!(
                "Failed to capture RAR adapter output for {}",
                path.display()
            )
        })?;

        if !output.status.success() {
            let combined = format_command_output(&output.stdout, &output.stderr);
            bail!(
                "RAR adapter failed for {}.\n{}",
                path.display(),
                combined.trim()
            );
        }

        let mut report = planned_report(&entries, options, plan)?;
        if matches!(options.overwrite_mode, OverwriteMode::Skip) {
            report.files_written = planned_file_writes_for_skip(&entries, options, plan)?;
        }
        Ok(report)
    }

    fn extract_archive_with_plan<F, C>(
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
        let entries = self.list_entries(path)?;
        let stage_root = env::temp_dir().join("fastzip-rar-stage").join(format!(
            "stage-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _stage_guard = TempDirGuard(stage_root.clone());
        fs::create_dir_all(&stage_root).with_context(|| {
            format!(
                "Failed to create staging directory {}",
                stage_root.display()
            )
        })?;

        let executable = self.require_executable()?;
        let output_dir = ensure_trailing_separator(&stage_root);
        let mut child = Command::new(executable)
            .arg("x")
            .arg("-o+")
            .arg("-idq")
            .arg(path.as_os_str())
            .arg(output_dir.as_os_str())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to invoke RAR adapter for {}", path.display()))?;

        loop {
            if should_cancel() {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Operation canceled");
            }

            if child
                .try_wait()
                .with_context(|| format!("Failed to wait for {}", path.display()))?
                .is_some()
            {
                break;
            }

            thread::sleep(Duration::from_millis(80));
        }

        let output = child.wait_with_output().with_context(|| {
            format!(
                "Failed to capture RAR adapter output for {}",
                path.display()
            )
        })?;

        if !output.status.success() {
            let combined = format_command_output(&output.stdout, &output.stderr);
            bail!(
                "RAR adapter failed for {}.\n{}",
                path.display(),
                combined.trim()
            );
        }

        let mut report = ExtractionReport::new(options.output_dir.clone());
        for entry in entries {
            if should_cancel() {
                bail!("Operation canceled");
            }
            if plan.should_skip(&entry.relative_path) {
                continue;
            }

            let destination = resolve_output_path(&entry.relative_path, options, plan)?;
            if entry.is_dir {
                create_directory(&destination, &mut report)?;
                continue;
            }

            let staged_source = stage_root.join(&entry.relative_path);
            if !prepare_output_file(&destination, options.overwrite_mode, &mut report)? {
                continue;
            }

            copy_staged_file_to_destination(
                &staged_source,
                &destination,
                &mut report,
                progress,
                should_cancel,
            )?;
        }

        Ok(report)
    }

    fn list_entries(&self, path: &Path) -> Result<Vec<RarListedEntry>> {
        let executable = self.require_executable()?;
        let output = Command::new(executable)
            .arg("lb")
            .arg(path.as_os_str())
            .output()
            .with_context(|| format!("Failed to inspect RAR archive {}", path.display()))?;

        if !output.status.success() {
            let combined = format_command_output(&output.stdout, &output.stderr);
            bail!(
                "RAR adapter failed while listing {}.\n{}",
                path.display(),
                combined.trim()
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();
        for raw_line in stdout.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            let raw_path = Path::new(line);
            if contains_unsafe_components(raw_path) {
                bail!("RAR archive contains an unsafe entry path: {line}");
            }

            let relative_path = sanitize_relative_path(raw_path);
            if relative_path.as_os_str().is_empty() {
                continue;
            }

            let is_dir = line.ends_with('/') || line.ends_with('\\');
            entries.push(RarListedEntry {
                relative_path,
                is_dir,
            });
        }

        Ok(entries)
    }

    fn validate_overwrite_mode(
        &self,
        entries: &[RarListedEntry],
        options: &ExtractOptions,
        plan: &ExtractPathPlan,
    ) -> Result<()> {
        if !matches!(options.overwrite_mode, OverwriteMode::Error) {
            return Ok(());
        }

        for entry in entries.iter().filter(|entry| !entry.is_dir) {
            if plan.should_skip(&entry.relative_path) {
                continue;
            }
            let destination = resolve_output_path(&entry.relative_path, options, plan)?;
            if destination.exists() {
                bail!(
                    "Refusing to overwrite existing file {}",
                    destination.display()
                );
            }
        }

        Ok(())
    }

    pub(crate) fn require_executable(&self) -> Result<&Path> {
        self.executable.as_deref().ok_or_else(|| {
            anyhow!(
                "RAR support is not configured. Install UnRAR/WinRAR or set {} to a CLI executable path.",
                RAR_ENV_VAR
            )
        })
    }
}

fn discover_rar_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(value) = env::var_os(RAR_ENV_VAR) {
        candidates.push(PathBuf::from(value));
    }

    candidates.extend(search_path_for("unrar.exe"));
    candidates.extend(search_path_for("unrar"));
    candidates.extend(search_path_for("rar.exe"));
    candidates.extend(search_path_for("rar"));
    candidates.extend([
        PathBuf::from(r"C:\Program Files\WinRAR\UnRAR.exe"),
        PathBuf::from(r"C:\Program Files\WinRAR\rar.exe"),
        PathBuf::from(r"C:\Program Files (x86)\WinRAR\UnRAR.exe"),
        PathBuf::from(r"C:\Program Files (x86)\WinRAR\rar.exe"),
    ]);

    candidates.into_iter().find(|path| path.is_file())
}

fn search_path_for(program: &str) -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|paths| {
            env::split_paths(&paths)
                .map(|path| path.join(program))
                .collect()
        })
        .unwrap_or_default()
}

fn ensure_trailing_separator(path: &Path) -> OsString {
    let mut value = path.as_os_str().to_os_string();
    let rendered = path.to_string_lossy();
    if !rendered.ends_with('\\') && !rendered.ends_with('/') {
        value.push("\\");
    }
    value
}

fn format_command_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    match (stdout.trim(), stderr.trim()) {
        ("", "") => "No additional error output.".to_string(),
        ("", stderr) => stderr.to_string(),
        (stdout, "") => stdout.to_string(),
        (stdout, stderr) => format!("{stdout}\n{stderr}"),
    }
}

fn planned_report(
    entries: &[RarListedEntry],
    options: &ExtractOptions,
    plan: &ExtractPathPlan,
) -> Result<ExtractionReport> {
    let mut report = ExtractionReport::new(options.output_dir.clone());
    let mut directories = BTreeSet::new();

    for entry in entries {
        if plan.should_skip(&entry.relative_path) {
            continue;
        }
        let destination = resolve_output_path(&entry.relative_path, options, plan)?;
        if entry.is_dir {
            collect_missing_directories(&destination, &options.output_dir, &mut directories);
            continue;
        }

        if let Some(parent) = destination.parent() {
            collect_missing_directories(parent, &options.output_dir, &mut directories);
        }

        match options.overwrite_mode {
            OverwriteMode::Overwrite | OverwriteMode::Error => {
                report.files_written += 1;
            }
            OverwriteMode::Skip => {
                if !destination.exists() {
                    report.files_written += 1;
                }
            }
        }
    }

    report.directories_created = directories.len();
    Ok(report)
}

fn planned_file_writes_for_skip(
    entries: &[RarListedEntry],
    options: &ExtractOptions,
    plan: &ExtractPathPlan,
) -> Result<usize> {
    let mut files_written = 0;
    for entry in entries.iter().filter(|entry| !entry.is_dir) {
        if plan.should_skip(&entry.relative_path) {
            continue;
        }
        let destination = resolve_output_path(&entry.relative_path, options, plan)?;
        if !destination.exists() {
            files_written += 1;
        }
    }
    Ok(files_written)
}

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_staged_file_to_destination<F, C>(
    source: &Path,
    destination: &Path,
    report: &mut ExtractionReport,
    progress: &mut F,
    should_cancel: &mut C,
) -> Result<()>
where
    F: FnMut(u64),
    C: FnMut() -> bool,
{
    if let Some(parent) = destination.parent() {
        create_directory(parent, report)?;
    }

    let mut reader =
        File::open(source).with_context(|| format!("Failed to open {}", source.display()))?;
    let mut writer = File::create(destination)
        .with_context(|| format!("Failed to create {}", destination.display()))?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        if should_cancel() {
            bail!("Operation canceled");
        }
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        progress(read as u64);
    }
    writer.flush()?;
    report.files_written += 1;
    Ok(())
}

fn collect_missing_directories(path: &Path, root: &Path, directories: &mut BTreeSet<PathBuf>) {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() || current == root {
            break;
        }
        directories.insert(current.clone());
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
}
