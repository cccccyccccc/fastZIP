import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "../state/settingsStore";
import { I18nContext } from "../hooks/useI18n";

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const language = useSettingsStore((s) => s.language);
  const [translations, setTranslations] = useState<Record<string, string>>({});

  useEffect(() => {
    invoke("get_translations", { code: language })
      .then((t) => setTranslations(t as Record<string, string>))
      .catch(() => setTranslations({}));
  }, [language]);

  return (
    <I18nContext.Provider value={translations}>
      {children}
    </I18nContext.Provider>
  );
}
