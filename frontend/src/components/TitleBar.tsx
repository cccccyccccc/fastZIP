import { getCurrentWindow } from "@tauri-apps/api/window";
import { useState } from "react";

export default function TitleBar() {
  const [maximized, setMaximized] = useState(false);
  const win = getCurrentWindow();

  const toggleMaximize = async () => {
    await win.toggleMaximize();
    setMaximized(await win.isMaximized());
  };

  return (
    <div
      data-tauri-drag-region
      className="h-9 bg-gray-100 dark:bg-gray-800 flex items-center justify-between select-none shrink-0 border-b border-gray-200 dark:border-gray-700"
    >
      <div className="flex items-center gap-2 pl-3">
        <span className="text-sm font-semibold text-gray-700 dark:text-gray-200">
          FastZIP
        </span>
      </div>
      <div className="flex h-full" data-tauri-drag-region="false">
        <button
          onClick={() => win.minimize()}
          className="w-10 h-full hover:bg-gray-200 dark:hover:bg-gray-700 flex items-center justify-center text-sm text-gray-500 dark:text-gray-400"
        >
          &#x2014;
        </button>
        <button
          onClick={toggleMaximize}
          className="w-10 h-full hover:bg-gray-200 dark:hover:bg-gray-700 flex items-center justify-center text-sm text-gray-500 dark:text-gray-400"
        >
          {maximized ? "⧉" : "□"}
        </button>
        <button
          onClick={() => win.close()}
          className="w-10 h-full hover:bg-red-600 flex items-center justify-center text-sm text-gray-500 dark:text-gray-400 hover:text-white"
        >
          &#x2715;
        </button>
      </div>
    </div>
  );
}
