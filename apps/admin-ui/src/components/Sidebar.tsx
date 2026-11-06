import { NavLink } from "react-router-dom";
import type { ModelLink } from "../types";
import { useAuth } from "../auth/AuthProvider";

export function Sidebar({ models }: { models: ModelLink[] }) {
  const { user, signOut } = useAuth();
  return (
    <aside className="flex w-full flex-col bg-ink text-slate-100 md:w-72">
      <div className="border-b border-white/10 px-5 py-6">
        <p className="text-xs uppercase tracking-[0.2em] text-teal-300">Ferrum</p>
        <h1 className="text-2xl font-semibold">Admin</h1>
      </div>
      <nav className="flex-1 space-y-1 px-3 py-4" aria-label="Modelle">
        <NavLink className="block rounded-lg px-3 py-2 hover:bg-white/10" to="/">Übersicht</NavLink>
        {models.map((model) => (
          <NavLink key={model.key} className="block rounded-lg px-3 py-2 hover:bg-white/10" to={`/models/${model.key}`}>
            {model.label}
          </NavLink>
        ))}
        {user?.is_superuser ? (
          <NavLink className="block rounded-lg px-3 py-2 hover:bg-white/10" to="/directory">Benutzer und Rollen</NavLink>
        ) : null}
      </nav>
      <div className="border-t border-white/10 px-5 py-4 text-sm">
        <p>{user?.email}</p>
        <button className="mt-2 underline" type="button" onClick={() => void signOut()}>Abmelden</button>
      </div>
    </aside>
  );
}
