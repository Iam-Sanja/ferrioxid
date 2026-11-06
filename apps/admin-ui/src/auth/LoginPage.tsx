import { FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";
import { login } from "../api/client";
import { useAuth } from "./AuthProvider";

export function LoginPage() {
  const navigate = useNavigate();
  const { setUser } = useAuth();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setError("");
    try {
      const user = await login(email, password);
      if (!user.is_staff) {
        setError("Dieses Konto hat keinen Admin-Zugang.");
        return;
      }
      setUser(user);
      navigate("/");
    } catch (caught) {
      setError(caught && typeof caught === "object" && "detail" in caught ? String(caught.detail) : "Anmeldung fehlgeschlagen");
    }
  }

  return (
    <main className="grid min-h-screen place-items-center bg-slate-100 px-4 dark:bg-slate-950">
      <form onSubmit={onSubmit} className="w-full max-w-md rounded-2xl bg-white p-8 shadow-xl dark:bg-slate-900">
        <p className="text-sm font-semibold uppercase tracking-wide text-teal-700">Ferrum</p>
        <h1 className="mt-2 text-3xl font-semibold text-slate-900 dark:text-white">Admin anmelden</h1>
        <label className="mt-6 block text-sm font-medium" htmlFor="email">E-Mail</label>
        <input id="email" className="mt-1 w-full rounded-lg border px-3 py-2 dark:bg-slate-800" value={email} onChange={(event) => setEmail(event.target.value)} autoComplete="username" />
        <label className="mt-4 block text-sm font-medium" htmlFor="password">Passwort</label>
        <input id="password" type="password" className="mt-1 w-full rounded-lg border px-3 py-2 dark:bg-slate-800" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="current-password" />
        {error ? <p role="alert" className="mt-4 text-sm text-red-700">{error}</p> : null}
        <button className="mt-6 w-full rounded-lg bg-teal-700 px-4 py-2 font-medium text-white" type="submit">Anmelden</button>
      </form>
    </main>
  );
}
