import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useTaskStore } from "../state/taskStore";
import { useUIStore } from "../state/uiStore";
import type { CompressionFormat, CompressionLevel, ZipCompressionMethod } from "../types";

const FORMATS: { value: CompressionFormat; label: string }[] = [
  { value: "zip", label: "ZIP" },
  { value: "seven_zip", label: "7Z" },
  { value: "tar_gz", label: "TAR.GZ" },
  { value: "tar_bz2", label: "TAR.BZ2" },
  { value: "tar_xz", label: "TAR.XZ" },
  { value: "tar_zst", label: "TAR.ZST" },
  { value: "tar_lz4", label: "TAR.LZ4" },
  { value: "tar", label: "TAR" },
  { value: "gz", label: "GZ" },
  { value: "bz2", label: "BZ2" },
  { value: "xz", label: "XZ" },
  { value: "zst", label: "ZST" },
  { value: "lz4", label: "LZ4" },
];

const LEVELS: { value: CompressionLevel; label: string }[] = [
  { value: "fastest", label: "Fastest" },
  { value: "fast", label: "Fast" },
  { value: "normal", label: "Normal" },
  { value: "maximum", label: "Maximum" },
  { value: "ultra", label: "Ultra" },
];

const METHODS: { value: ZipCompressionMethod; label: string }[] = [
  { value: "deflate", label: "Deflate" },
  { value: "stored", label: "Store" },
  { value: "bzip2", label: "BZip2" },
  { value: "zstd", label: "Zstandard" },
  { value: "xz", label: "XZ" },
];

export default function CompressPage() {
  const [sources, setSources] = useState<string[]>([]);
  const [outputPath, setOutputPath] = useState("");
  const [format, setFormat] = useState<CompressionFormat>("zip");
  const [level, setLevel] = useState<CompressionLevel>("normal");
  const [method, setMethod] = useState<ZipCompressionMethod>("deflate");
  const [threads, setThreads] = useState(4);
  const [password, setPassword] = useState("");
  const [encryptNames, setEncryptNames] = useState(false);
  const [splitVolume, setSplitVolume] = useState("");
  const [sfx, setSfx] = useState(false);

  const addTask = useTaskStore((s) => s.addTask);
  const addToast = useUIStore((s) => s.addToast);

  const browseSources = async () => {
    const selected = await open({ multiple: true });
    if (selected && Array.isArray(selected)) {
      setSources(selected as string[]);
    }
  };

  const browseOutput = async () => {
    const selected = await open({
      filters: [
        { name: "Archives", extensions: ["zip", "7z", "tar.gz", "tar.bz2", "tar.xz", "tar.zst", "tar.lz4", "tar", "gz", "bz2", "xz", "zst", "lz4"] },
      ],
    });
    if (selected) setOutputPath(selected as string);
  };

  const parseVolumeBytes = (v: string): number | null => {
    if (!v) return null;
    const s = v.trim().toUpperCase();
    const num = parseFloat(s);
    if (isNaN(num)) return null;
    if (s.endsWith("GB")) return Math.round(num * 1073741824);
    if (s.endsWith("MB")) return Math.round(num * 1048576);
    if (s.endsWith("KB")) return Math.round(num * 1024);
    return Math.round(num);
  };

  const startCompress = () => {
    if (!sources.length || !outputPath) return;
    const taskId = addTask("compress", outputPath.split(/[\\/]/).pop() || outputPath);
    invoke("start_compress", {
      request: {
        sources,
        output_path: outputPath,
        options: {
          format,
          level,
          zip_method: method,
          thread_count: threads,
          password: password || null,
          encrypt_file_names: encryptNames,
          split_volume_size: parseVolumeBytes(splitVolume),
          sfx,
        },
      },
      taskId,
    });
    addToast("Compression started", "success");
  };

  return (
    <div className="flex flex-col h-full gap-4">
      <h2 className="text-xl font-bold">Compress Files</h2>

      {/* Source files */}
      <div className="flex gap-2">
        <textarea
          value={sources.join("\n")}
          readOnly
          placeholder="Selected files/folders..."
          className="flex-1 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm h-24 resize-none"
        />
        <button onClick={browseSources} className="px-4 py-2 bg-blue-600 text-white rounded-lg text-sm hover:bg-blue-700 self-start">
          Add Files
        </button>
      </div>

      {/* Output path */}
      <div className="flex gap-2">
        <input
          type="text"
          value={outputPath}
          onChange={(e) => setOutputPath(e.target.value)}
          placeholder="Output archive path..."
          className="flex-1 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
        />
        <button onClick={browseOutput} className="px-4 py-2 bg-blue-600 text-white rounded-lg text-sm hover:bg-blue-700">
          Save As
        </button>
      </div>

      {/* Options grid */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div>
          <label className="block text-xs text-gray-500 mb-1">Format</label>
          <select value={format} onChange={(e) => setFormat(e.target.value as CompressionFormat)}
            className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
          >
            {FORMATS.map((f) => <option key={f.value} value={f.value}>{f.label}</option>)}
          </select>
        </div>
        <div>
          <label className="block text-xs text-gray-500 mb-1">Level</label>
          <select value={level} onChange={(e) => setLevel(e.target.value as CompressionLevel)}
            className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
          >
            {LEVELS.map((l) => <option key={l.value} value={l.value}>{l.label}</option>)}
          </select>
        </div>
        <div>
          <label className="block text-xs text-gray-500 mb-1">Method</label>
          <select value={method} onChange={(e) => setMethod(e.target.value as ZipCompressionMethod)}
            className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
          >
            {METHODS.map((m) => <option key={m.value} value={m.value}>{m.label}</option>)}
          </select>
        </div>
        <div>
          <label className="block text-xs text-gray-500 mb-1">Threads</label>
          <input type="number" value={threads} onChange={(e) => setThreads(Number(e.target.value))} min={1} max={64}
            className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
          />
        </div>
      </div>

      {/* Advanced options */}
      <div className="flex flex-wrap gap-4 items-center">
        <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="Password (optional)"
          className="px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm w-48"
        />
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={encryptNames} onChange={(e) => setEncryptNames(e.target.checked)} />
          Encrypt file names (7z)
        </label>
        <input type="text" value={splitVolume} onChange={(e) => setSplitVolume(e.target.value)} placeholder="Split (e.g. 100MB)"
          className="px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm w-36"
        />
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={sfx} onChange={(e) => setSfx(e.target.checked)} />
          SFX (self-extracting)
        </label>
      </div>

      <button onClick={startCompress} disabled={!sources.length || !outputPath}
        className="px-6 py-2.5 bg-green-600 text-white rounded-lg text-sm font-medium hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        Start Compression
      </button>
    </div>
  );
}
