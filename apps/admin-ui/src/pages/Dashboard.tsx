import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { modelLinks } from "../api/client";
import { useAuth } from "../auth/AuthProvider";
import { useI18n } from "../i18n";
import type { ModelLink } from "../types";

export function Dashboard() {
  const { user } = useAuth();
  const { t } = useI18n();
  const [models, setModels] = useState<ModelLink[]>([]);
  useEffect(() => {
    void modelLinks().then(setModels);
  }, []);
  return (
    <section>
      <h2 className="text-3xl font-semibold">{t("common.overview")}</h2>
      <p className="mt-2 text-slate-600 dark:text-slate-300">{t("admin.dashboardHint", { email: user?.email ?? "" })}</p>
      <div className="mt-6 grid gap-4 md:grid-cols-3">
        {models.map((model) => (
          <Link key={model.key} to={`/models/${model.key}`} className="rounded-2xl bg-white p-5 shadow-sm dark:bg-slate-900">
            <h3 className="text-xl font-medium">{model.label}</h3>
            <p className="mt-2 text-sm text-slate-500">{model.list_blocked ? t("admin.listBlocked") : t("admin.listReady")}</p>
          </Link>
        ))}
      </div>
    </section>
  );
}
