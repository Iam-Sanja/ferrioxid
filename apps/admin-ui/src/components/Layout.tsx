import { useEffect, useState, type ReactNode } from "react";
import { modelLinks } from "../api/client";
import type { ModelLink } from "../types";
import { useI18n } from "../i18n";
import { Sidebar } from "./Sidebar";

export function Layout({ children }: { children: ReactNode }) {
  const [models, setModels] = useState<ModelLink[]>([]);
  const [dark, setDark] = useState(false);
  const { locale, setLocale, t } = useI18n();

  useEffect(() => {
    const stored = localStorage.getItem("ferrum-theme");
    const enabled = stored === "dark";
    setDark(enabled);
    document.documentElement.classList.toggle("dark", enabled);
    void modelLinks().then(setModels).catch(() => setModels([]));
  }, []);

  function toggleTheme() {
    const next = !dark;
    setDark(next);
    document.documentElement.classList.toggle("dark", next);
    localStorage.setItem("ferrum-theme", next ? "dark" : "light");
  }

  return (
    <div className="min-h-screen bg-slate-100 text-slate-900 dark:bg-slate-950 dark:text-slate-100 md:flex">
      <Sidebar models={models} />
      <div className="min-w-0 flex-1">
        <header className="flex items-center justify-end gap-2 border-b bg-white px-6 py-3 dark:border-slate-800 dark:bg-slate-900">
          <button type="button" className="btn btn-secondary" aria-label={t("common.locale")} onClick={() => setLocale(locale === "de" ? "en" : "de")}>
            {locale === "de" ? "EN" : "DE"}
          </button>
          <button type="button" className="btn btn-secondary" onClick={toggleTheme}>
            {dark ? t("common.themeLight") : t("common.themeDark")}
          </button>
        </header>
        <main className="p-4 md:p-8">{children}</main>
      </div>
    </div>
  );
}
