import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { modelLinks } from "../api/client";
import { useAuth } from "../auth/AuthProvider";
import type { ModelLink } from "../types";

export function Dashboard() {
  const { user } = useAuth();
  const [models, setModels] = useState<ModelLink[]>([]);
  useEffect(() => {
    void modelLinks().then(setModels);
  }, []);
  return (
    <section>
      <h2 className="text-3xl font-semibold">Übersicht</h2>
      <p className="mt-2 text-slate-600 dark:text-slate-300">Angemeldet als {user?.email}. Modelle erscheinen nur, wenn sie mit Admin-Metadaten registriert sind.</p>
      <div className="mt-6 grid gap-4 md:grid-cols-3">
        {models.map((model) => (
          <Link key={model.key} to={`/models/${model.key}`} className="rounded-2xl bg-white p-5 shadow-sm dark:bg-slate-900">
            <h3 className="text-xl font-medium">{model.label}</h3>
            <p className="mt-2 text-sm text-slate-500">{model.list_blocked ? "Listenansicht ist durch eine Objekt-Policy blockiert." : "Tabelle, Formular und Detailansicht"}</p>
          </Link>
        ))}
      </div>
    </section>
  );
}
