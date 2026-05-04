import { createContext, useContext } from "react";
import { useSettingsStore } from "../state/settingsStore";

// All translations are loaded at startup via `get_translations` command.
// This context provides a simple `t(key)` function.
// For now, we default to English keys as the fallback.

export const I18nContext = createContext<Record<string, string>>({});

export function useI18n() {
  const translations = useContext(I18nContext);
  const language = useSettingsStore((s) => s.language);

  return {
    t: (key: string, fallback?: string): string => {
      return translations[key] || fallback || key;
    },
    language,
  };
}
