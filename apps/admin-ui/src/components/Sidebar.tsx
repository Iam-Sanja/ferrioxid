import { NavLink } from "react-router-dom";
import type { ModelLink } from "../types";
import { useAuth } from "../auth/AuthProvider";
import { useI18n } from "../i18n";

export function Sidebar({ models }: { models: ModelLink[] }) {
  const { user, signOut } = useAuth();
  const { branding, t } = useI18n();
  return (
    <aside className="flex w-full flex-col bg-ink text-slate-100 md:w-72">
      <div className="border-b border-white/10 px-5 py-6">
        <p className="text-xs uppercase tracking-[0.2em] text-teal-300">{branding.app_name}</p>
        <h1 className="text-2xl font-semibold">{branding.site_title}</h1>
      </div>
      <nav className="flex-1 space-y-1 px-3 py-4" aria-label={t("admin.navigation")}>
        <NavLink className={({ isActive }) => `nav-link${isActive ? " active" : ""}`} to="/">{t("common.overview")}</NavLink>
        {models.map((model) => (
          <NavLink key={model.key} className={({ isActive }) => `nav-link${isActive ? " active" : ""}`} to={`/models/${model.key}`}>
            {model.label}
          </NavLink>
        ))}
        {user?.is_superuser ? (
          <NavLink className={({ isActive }) => `nav-link${isActive ? " active" : ""}`} to="/directory">{t("directory.title")}</NavLink>
        ) : null}
      </nav>
      <div className="border-t border-white/10 px-5 py-4 text-sm">
        <p>{user?.email}</p>
        <button className="btn btn-secondary mt-3" type="button" onClick={() => void signOut()}>{t("auth.signOut")}</button>
      </div>
    </aside>
  );
}
