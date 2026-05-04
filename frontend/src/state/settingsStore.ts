import { create } from "zustand";
import type { AppLocale } from "../types";

interface SettingsState {
  language: string;
  theme: "light" | "dark" | "system";
  autoUpdate: boolean;
  autoStart: boolean;
  locales: AppLocale[];
  loading: boolean;

  setLanguage: (code: string) => void;
  setTheme: (theme: "light" | "dark" | "system") => void;
  setAutoUpdate: (enabled: boolean) => void;
  setAutoStart: (enabled: boolean) => void;
  setLocales: (locales: AppLocale[]) => void;
  setLoading: (v: boolean) => void;
}

export const useSettingsStore = create<SettingsState>()((set) => ({
  language: "en",
  theme: "system",
  autoUpdate: true,
  autoStart: false,
  locales: [],
  loading: true,

  setLanguage: (language) => set({ language }),
  setTheme: (theme) => set({ theme }),
  setAutoUpdate: (autoUpdate) => set({ autoUpdate }),
  setAutoStart: (autoStart) => set({ autoStart }),
  setLocales: (locales) => set({ locales }),
  setLoading: (loading) => set({ loading }),
}));
