import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useTaskStore } from "../state/taskStore";
import { useUIStore } from "../state/uiStore";
import type { FileInfo, ChecksumResult, CompressionFormat, CompressionLevel } from "../types";

export default function FileManagerPage() {
  const [currentDir, setCurrentDir] = useState("");
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [checksums, setChecksums] = useState<ChecksumResult[]>([]);
  const [checksumFile, setChecksumFile] = useState("");

  const addTask = useTaskStore((s) => s.addTask);
  const addToast = useUIStore((s) => s.addToast);

  const browseDir = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      setCurrentDir(selected as string);
      loadDir(selected as string);
    }
  };

  const loadDir = async (path: string) => {
    try {
      const list = await invoke<FileInfo[]>("list_directory", { path });
      setFiles(list);
      setSelected(new Set());
    } catch (e) {
      addToast(`Failed to list directory: ${e}`, "error");
    }
  };

  const toggleSelect = (path: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      next.has(path) ? next.delete(path) : next.add(path);
      return next;
    });
  };

  const calcChecksum = async (filePath: string) => {
    try {
      const results = await invoke<ChecksumResult[]>("calculate_all_checksums", { path: filePath });
      setChecksums(results);
      setChecksumFile(filePath);
    } catch (e) {
      addToast(`Checksum failed: ${e}`, "error");
    }
  };

  const compressSelected = async () => {
    const selectedFiles = files.filter((f) => selected.has(f.path)).map((f) => f.path);
    if (!selectedFiles.length) return;
    const output = await open({
      title: "Save archive as",
      filters: [{ name: "ZIP Archive", extensions: ["zip"] }],
    });
    if (!output) return;
    const taskId = addTask("compress", selectedFiles.length === 1
      ? selectedFiles[0].split(/[\\/]/).pop() || "archive"
      : `${selectedFiles.length} files`);
    invoke("start_compress", {
      request: {
        sources: selectedFiles,
        output_path: output,
        options: {
          format: "zip" as CompressionFormat,
          level: "normal" as CompressionLevel,
          zip_method: "deflate",
          thread_count: 4,
          password: null,
          encrypt_file_names: false,
          split_volume_size: null,
          sfx: false,
        },
      },
      taskId,
    });
    addToast("Compression started", "success");
  };

  const selectAll = () => {
    if (selected.size === files.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(files.filter((f) => !f.is_dir).map((f) => f.path)));
    }
  };

  return (
    <div className="flex flex-col h-full gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold">File Manager</h2>
        <div className="flex gap-2">
          {selected.size > 0 && (
            <button onClick={compressSelected} className="px-4 py-2 bg-green-600 text-white rounded-lg text-sm hover:bg-green-700">
              Compress {selected.size} selected
            </button>
          )}
          <button onClick={browseDir} className="px-4 py-2 bg-blue-600 text-white rounded-lg text-sm hover:bg-blue-700">
            Open Folder
          </button>
        </div>
      </div>

      {currentDir && (
        <p className="text-xs text-gray-500 truncate">{currentDir}</p>
      )}

      {/* File list */}
      <div className="flex-1 min-h-0 overflow-auto border border-gray-200 dark:border-gray-700 rounded-lg">
        {files.length === 0 ? (
          <p className="p-4 text-gray-400 text-sm">Open a folder to browse files.</p>
        ) : (
          <table className="w-full text-sm">
            <thead className="bg-gray-50 dark:bg-gray-800 sticky top-0">
              <tr>
                <th className="w-8 px-3 py-2">
                  <input type="checkbox" checked={selected.size === files.filter((f) => !f.is_dir).length && files.length > 0}
                    onChange={selectAll} />
                </th>
                <th className="text-left px-3 py-2">Name</th>
                <th className="text-right px-3 py-2">Size</th>
                <th className="text-right px-3 py-2">Modified</th>
                <th className="px-3 py-2 w-24"></th>
              </tr>
            </thead>
            <tbody>
              {files.map((f) => (
                <tr key={f.path} className="border-t border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-850">
                  <td className="px-3 py-1.5">
                    {!f.is_dir && (
                      <input type="checkbox" checked={selected.has(f.path)} onChange={() => toggleSelect(f.path)} />
                    )}
                  </td>
                  <td className="px-3 py-1.5">{f.name}{f.is_dir ? "/" : ""}</td>
                  <td className="text-right px-3 py-1.5 text-gray-500">{f.is_dir ? "-" : formatBytes(f.size)}</td>
                  <td className="text-right px-3 py-1.5 text-gray-500 text-xs">
                    {new Date(f.modified_secs * 1000).toLocaleDateString()}
                  </td>
                  <td className="px-3 py-1.5">
                    {!f.is_dir && (
                      <button onClick={() => calcChecksum(f.path)} className="text-xs text-blue-600 hover:underline">
                        Checksum
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Checksum results */}
      {checksums.length > 0 && (
        <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-4">
          <h3 className="text-sm font-semibold mb-2">Checksums: {checksumFile.split(/[\\/]/).pop()}</h3>
          <div className="space-y-1">
            {checksums.map((c) => (
              <div key={c.algorithm} className="flex items-center gap-2 text-xs">
                <span className="font-mono text-gray-500 w-16">{c.algorithm.toUpperCase()}:</span>
                <code className="text-gray-700 dark:text-gray-300">{c.hex_digest}</code>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}
