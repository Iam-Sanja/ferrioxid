import { useEffect, useState, type ReactNode } from "react";
import { modelLinks } from "../api/client";
import type { ModelLink } from "../types";
import { Sidebar } from "./Sidebar";

export function Layout({ children }: { children: ReactNode }) {
  const [models, setModels] = useState<ModelLink[]>([]);
  const [dark, setDark] = useState(false);

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
        <header className="flex items-center justify-end border-b bg-white px-6 py-3 dark:border-slate-800 dark:bg-slate-900">
          <button type="button" className="rounded-lg border px-3 py-1" onClick={toggleTheme}>
            {dark ? "Hell" : "Dunkel"}
          </button>
        </header>
        <main className="p-4 md:p-8">{children}</main>
      </div>
    </div>
  );
}
