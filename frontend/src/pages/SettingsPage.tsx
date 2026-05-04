import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "../state/settingsStore";
import { useUIStore } from "../state/uiStore";
import type { AppLocale, BackendStatus } from "../types";

export default function SettingsPage() {
  const {
    language, theme, autoUpdate, autoStart, locales, loading,
    setLanguage, setTheme, setAutoUpdate, setAutoStart, setLocales, setLoading,
  } = useSettingsStore();
  const addToast = useUIStore((s) => s.addToast);

  useEffect(() => {
    (async () => {
      try {
        const [lang, th, au, as_, locs] = await Promise.all([
          invoke<string>("get_language"),
          invoke<string>("get_theme"),
          invoke<boolean>("get_auto_update_enabled"),
          invoke<boolean>("get_autostart_enabled"),
          invoke<AppLocale[]>("get_supported_locales"),
        ]);
        setLanguage(lang);
        setTheme(th as "light" | "dark" | "system");
        setAutoUpdate(au);
        setAutoStart(as_);
        setLocales(locs);
      } catch (e) {
        addToast(`Failed to load settings: ${e}`, "error");
      }
      setLoading(false);
    })();
  }, []);

  const changeLanguage = async (code: string) => {
    try {
      await invoke("set_language", { code });
      setLanguage(code);
      addToast("Language changed", "success");
    } catch (e) {
      addToast(`Failed: ${e}`, "error");
    }
  };

  const changeTheme = async (t: string) => {
    try {
      await invoke("set_theme", { theme: t });
      setTheme(t as "light" | "dark" | "system");
    } catch (e) {
      addToast(`Failed: ${e}`, "error");
    }
  };

  const toggleAutoUpdate = async () => {
    const next = !autoUpdate;
    try {
      await invoke("set_auto_update_enabled", { enabled: next });
      setAutoUpdate(next);
    } catch (e) {
      addToast(`Failed: ${e}`, "error");
    }
  };

  const toggleAutoStart = async () => {
    const next = !autoStart;
    try {
      await invoke("set_autostart_enabled", { enabled: next });
      setAutoStart(next);
      addToast(next ? "Added to startup" : "Removed from startup", "success");
    } catch (e) {
      addToast(`Failed: ${e}`, "error");
    }
  };

  if (loading) return <p className="text-gray-400 text-sm">Loading settings...</p>;

  return (
    <div className="flex flex-col h-full gap-6 max-w-lg">
      <h2 className="text-xl font-bold">Settings</h2>

      <div>
        <label className="block text-sm font-medium mb-1">Language</label>
        <select
          value={language}
          onChange={(e) => changeLanguage(e.target.value)}
          className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
        >
          {locales.map((l) => (
            <option key={l.code} value={l.code}>{l.name_en}</option>
          ))}
        </select>
      </div>

      <div>
        <label className="block text-sm font-medium mb-1">Theme</label>
        <div className="flex gap-2">
          {(["light", "dark", "system"] as const).map((t) => (
            <button
              key={t}
              onClick={() => changeTheme(t)}
              className={`px-4 py-2 rounded-lg text-sm border transition-colors ${
                theme === t
                  ? "border-blue-500 bg-blue-50 dark:bg-blue-900 text-blue-700 dark:text-blue-300"
                  : "border-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-800"
              }`}
            >
              {t.charAt(0).toUpperCase() + t.slice(1)}
            </button>
          ))}
        </div>
      </div>

      <div className="space-y-3">
        <label className="flex items-center justify-between cursor-pointer">
          <span className="text-sm">Auto-update check on startup</span>
          <button
            onClick={toggleAutoUpdate}
            className={`w-10 h-6 rounded-full transition-colors ${autoUpdate ? "bg-blue-600" : "bg-gray-300 dark:bg-gray-600"}`}
          >
            <span className={`block w-5 h-5 rounded-full bg-white transition-transform ${autoUpdate ? "ml-4.5" : "ml-0.5"}`} />
          </button>
        </label>

        <label className="flex items-center justify-between cursor-pointer">
          <span className="text-sm">Start with Windows</span>
          <button
            onClick={toggleAutoStart}
            className={`w-10 h-6 rounded-full transition-colors ${autoStart ? "bg-blue-600" : "bg-gray-300 dark:bg-gray-600"}`}
          >
            <span className={`block w-5 h-5 rounded-full bg-white transition-transform ${autoStart ? "ml-4.5" : "ml-0.5"}`} />
          </button>
        </label>
      </div>

      <BackendStatusPanel />
    </div>
  );
}

function BackendStatusPanel() {
  const [statuses, setStatuses] = useState<BackendStatus[]>([]);
  const addToast = useUIStore((s) => s.addToast);

  useEffect(() => {
    invoke<BackendStatus[]>("get_backend_statuses")
      .then(setStatuses)
      .catch((e) => addToast(`Failed: ${e}`, "error"));
  }, []);

  if (statuses.length === 0) return null;

  return (
    <div>
      <h3 className="text-sm font-medium mb-2">Backends</h3>
      <div className="space-y-2">
        {statuses.map((s) => (
          <div key={s.kind} className="flex items-center gap-2 text-sm">
            <span className={`w-2 h-2 rounded-full ${s.available ? "bg-green-500" : "bg-red-500"}`} />
            <span className="font-medium">{s.label}</span>
            <span className="text-gray-500">&mdash; {s.detail}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
