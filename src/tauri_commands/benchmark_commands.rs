use std::path::PathBuf;

use crate::benchmark::{self, BenchmarkEntry};

#[tauri::command]
pub fn run_benchmark(output_dir: PathBuf) -> Result<Vec<BenchmarkEntry>, String> {
    benchmark::run_benchmark(&output_dir).map_err(|e| e.to_string())
}
