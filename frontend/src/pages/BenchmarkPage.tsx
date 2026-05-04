import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { BenchmarkEntry } from "../types";

export default function BenchmarkPage() {
  const [outputDir, setOutputDir] = useState("");
  const [results, setResults] = useState<BenchmarkEntry[]>([]);
  const [running, setRunning] = useState(false);

  const browseOutput = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected) setOutputDir(selected as string);
  };

  const runBenchmark = () => {
    setRunning(true);
    invoke<BenchmarkEntry[]>("run_benchmark", { outputDir: outputDir || "benchmark_results" })
      .then((data) => {
        setResults(data.map((e) => ({
          ...e,
          compression_ratio: e.output_bytes / Math.max(1, e.input_bytes),
          throughput_mbps: e.elapsed > 0 ? (e.input_bytes / e.elapsed) / (1024 * 1024) : 0,
        })));
      })
      .catch((e) => console.error("Benchmark failed:", e))
      .finally(() => setRunning(false));
  };

  return (
    <div className="flex flex-col h-full gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold">Benchmark</h2>
        <div className="flex gap-2">
          <input
            type="text"
            value={outputDir}
            onChange={(e) => setOutputDir(e.target.value)}
            placeholder="Output directory (optional)..."
            className="px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm w-64"
          />
          <button onClick={browseOutput} className="px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded-lg text-sm">
            Browse
          </button>
          <button
            onClick={runBenchmark}
            disabled={running}
            className="px-6 py-2 bg-orange-600 text-white rounded-lg text-sm font-medium hover:bg-orange-700 disabled:opacity-50"
          >
            {running ? "Running..." : "Run Benchmark"}
          </button>
        </div>
      </div>

      <p className="text-xs text-gray-500">
        Benchmarks compression speed across all formats and levels using 1 MB of data.
      </p>

      {results.length > 0 && (
        <div className="flex-1 min-h-0 overflow-auto border border-gray-200 dark:border-gray-700 rounded-lg">
          <table className="w-full text-sm">
            <thead className="bg-gray-50 dark:bg-gray-800 sticky top-0">
              <tr>
                <th className="text-left px-3 py-2">Format</th>
                <th className="text-left px-3 py-2">Level</th>
                <th className="text-right px-3 py-2">Input</th>
                <th className="text-right px-3 py-2">Output</th>
                <th className="text-right px-3 py-2">Ratio</th>
                <th className="text-right px-3 py-2">Speed</th>
              </tr>
            </thead>
            <tbody>
              {results.map((e, i) => (
                <tr key={i} className="border-t border-gray-100 dark:border-gray-800">
                  <td className="px-3 py-1.5 font-medium">{e.format}</td>
                  <td className="px-3 py-1.5">{e.level}</td>
                  <td className="text-right px-3 py-1.5 text-gray-500">{formatBytes(e.input_bytes)}</td>
                  <td className="text-right px-3 py-1.5 text-gray-500">{formatBytes(e.output_bytes)}</td>
                  <td className="text-right px-3 py-1.5 text-gray-500">{(e.compression_ratio ?? 0).toFixed(3)}</td>
                  <td className="text-right px-3 py-1.5 text-gray-500">{(e.throughput_mbps ?? 0).toFixed(1)} MB/s</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}
