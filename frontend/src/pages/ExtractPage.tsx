import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useTaskStore } from "../state/taskStore";
import { useUIStore } from "../state/uiStore";
import type { ArchiveEntry, ArchiveInspection, OverwriteMode } from "../types";

export default function ExtractPage() {
  const [archivePath, setArchivePath] = useState("");
  const [outputDir, setOutputDir] = useState("");
  const [password, setPassword] = useState("");
  const [overwrite, setOverwrite] = useState<OverwriteMode>("overwrite");
  const [keepPaths, setKeepPaths] = useState(true);
  const [entries, setEntries] = useState<ArchiveEntry[]>([]);
  const [inspection, setInspection] = useState<ArchiveInspection | null>(null);
  const [loading, setLoading] = useState(false);

  const addTask = useTaskStore((s) => s.addTask);
  const addToast = useUIStore((s) => s.addToast);

  const browseArchive = async () => {
    const selected = await open({ multiple: false, filters: [{ name: "Archives", extensions: ["zip", "7z", "rar", "tar", "tar.gz", "tar.bz2", "tar.xz", "gz", "bz2", "xz", "zst", "lz4", "iso", "wim"] }] });
    if (selected) {
      setArchivePath(selected as string);
      await inspectArchive(selected as string);
    }
  };

  const browseOutput = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected) setOutputDir(selected as string);
  };

  const inspectArchive = async (path: string) => {
    setLoading(true);
    try {
      const info = await invoke<ArchiveInspection>("inspect_archive", { path });
      setInspection(info);
      const list = await invoke<ArchiveEntry[]>("list_archive", { path, password: password || null });
      setEntries(list);
    } catch (e) {
      addToast(`Failed to inspect archive: ${e}`, "error");
    }
    setLoading(false);
  };

  const startExtract = async () => {
    if (!archivePath || !outputDir) return;
    const taskId = addTask("extract", archivePath.split(/[\\/]/).pop() || archivePath);
    invoke("start_extract", {
      request: {
        path: archivePath,
        output_dir: outputDir,
        overwrite_mode: overwrite,
        keep_paths: keepPaths,
        password: password || null,
        filename_encoding: "utf8",
        scan_files: false,
      },
      taskId,
    });
    addToast("Extraction started", "success");
  };

  return (
    <div className="flex flex-col h-full gap-4">
      <h2 className="text-xl font-bold">Extract Archive</h2>

      {/* Archive selection */}
      <div className="flex gap-2">
        <input
          type="text"
          value={archivePath}
          onChange={(e) => setArchivePath(e.target.value)}
          placeholder="Archive path..."
          className="flex-1 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
        />
        <button onClick={browseArchive} className="px-4 py-2 bg-blue-600 text-white rounded-lg text-sm hover:bg-blue-700">
          Browse
        </button>
      </div>

      {/* Output directory */}
      <div className="flex gap-2">
        <input
          type="text"
          value={outputDir}
          onChange={(e) => setOutputDir(e.target.value)}
          placeholder={inspection?.suggested_output_dir || "Output directory..."}
          className="flex-1 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
        />
        <button onClick={browseOutput} className="px-4 py-2 bg-blue-600 text-white rounded-lg text-sm hover:bg-blue-700">
          Browse
        </button>
      </div>

      {/* Options */}
      <div className="flex flex-wrap gap-4 items-center">
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="Password (optional)"
          className="px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm w-48"
        />
        <select
          value={overwrite}
          onChange={(e) => setOverwrite(e.target.value as OverwriteMode)}
          className="px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
        >
          <option value="overwrite">Overwrite</option>
          <option value="skip">Skip existing</option>
          <option value="error">Error on conflict</option>
        </select>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={keepPaths} onChange={(e) => setKeepPaths(e.target.checked)} />
          Keep folder structure
        </label>
      </div>

      {/* Entry preview */}
      {loading && <p className="text-sm text-gray-500">Loading entries...</p>}
      {entries.length > 0 && (
        <div className="flex-1 min-h-0 overflow-auto border border-gray-200 dark:border-gray-700 rounded-lg">
          <table className="w-full text-sm">
            <thead className="bg-gray-50 dark:bg-gray-800 sticky top-0">
              <tr>
                <th className="text-left px-3 py-2">Name</th>
                <th className="text-right px-3 py-2">Size</th>
                <th className="text-right px-3 py-2">Packed</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((e, i) => (
                <tr key={i} className="border-t border-gray-100 dark:border-gray-800">
                  <td className="px-3 py-1.5">{e.path}{e.is_dir ? "/" : ""}</td>
                  <td className="text-right px-3 py-1.5 text-gray-500">{e.uncompressed_size != null ? formatBytes(e.uncompressed_size) : "-"}</td>
                  <td className="text-right px-3 py-1.5 text-gray-500">{e.compressed_size != null ? formatBytes(e.compressed_size) : "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <button
        onClick={startExtract}
        disabled={!archivePath || !outputDir}
        className="px-6 py-2.5 bg-green-600 text-white rounded-lg text-sm font-medium hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        Start Extraction
      </button>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}
