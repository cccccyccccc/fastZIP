import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ReleaseInfo } from "../types";

export default function UpdateDialog() {
  const [update, setUpdate] = useState<ReleaseInfo | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    invoke<ReleaseInfo | null>("check_for_updates")
      .then((info) => { if (info) setUpdate(info); })
      .catch(() => {});
  }, []);

  if (!update || dismissed) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-xl shadow-2xl p-6 max-w-sm w-full mx-4">
        <h3 className="text-lg font-bold mb-2">Update Available</h3>
        <p className="text-sm text-gray-500 mb-1">
          FastZIP {update.version} is available for download.
        </p>
        {update.body && (
          <pre className="text-xs text-gray-400 mb-4 whitespace-pre-wrap max-h-32 overflow-auto">
            {update.body}
          </pre>
        )}
        <div className="flex gap-2 justify-end">
          <button
            onClick={() => setDismissed(true)}
            className="px-4 py-2 text-sm text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg"
          >
            Later
          </button>
          <a
            href={update.download_url}
            target="_blank"
            rel="noreferrer"
            className="px-4 py-2 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-700"
          >
            Download
          </a>
        </div>
      </div>
    </div>
  );
}
