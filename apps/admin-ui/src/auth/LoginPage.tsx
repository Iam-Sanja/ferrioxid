import { FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";
import { login } from "../api/client";
import { translateError } from "../i18n/errors";
import { useI18n } from "../i18n";
import type { ApiError } from "../types";
import { useAuth } from "./AuthProvider";

export function LoginPage() {
  const navigate = useNavigate();
  const { setUser } = useAuth();
  const { branding, locale, setLocale, t } = useI18n();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setError("");
    try {
      const user = await login(email, password);
      if (!user.is_staff) {
        setError(t("auth.noAdmin"));
        return;
      }
      setUser(user);
      navigate("/");
    } catch (caught) {
      setError(isApiError(caught) ? translateError(locale, caught) : t("auth.failed"));
    }
  }

  return (
    <main className="grid min-h-screen place-items-center bg-slate-100 px-4 dark:bg-slate-950">
      <form onSubmit={onSubmit} className="w-full max-w-md rounded-2xl bg-white p-8 shadow-xl dark:bg-slate-900">
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm font-semibold uppercase tracking-wide text-teal-700">{branding.app_name}</p>
          <button type="button" className="btn btn-secondary" aria-label={t("common.locale")} onClick={() => setLocale(locale === "de" ? "en" : "de")}>
            {locale === "de" ? "EN" : "DE"}
          </button>
        </div>
        <h1 className="mt-2 text-3xl font-semibold text-slate-900 dark:text-white">{t("auth.adminTitle")}</h1>
        <div className="field mt-6">
          <label htmlFor="email">{t("auth.email")}</label>
          <input id="email" value={email} onChange={(event) => setEmail(event.target.value)} autoComplete="username" />
        </div>
        <div className="field mt-4">
          <label htmlFor="password">{t("auth.password")}</label>
          <input id="password" type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="current-password" />
        </div>
        {error ? <p role="alert" className="field-error mt-4">{error}</p> : null}
        <button className="btn btn-primary mt-6 w-full" type="submit">{t("auth.signIn")}</button>
      </form>
    </main>
  );
}

function isApiError(value: unknown): value is ApiError {
  return Boolean(value && typeof value === "object" && "detail" in value && "status" in value);
}
