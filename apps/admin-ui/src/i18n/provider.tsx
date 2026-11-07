import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  defaultBranding,
  I18nContext,
  isCurrency,
  isHexColor,
  isLocale,
  translate,
  type Locale,
  type PublicConfig,
} from "./index";

const storageKey = "ferrum-locale";

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => readLocale() ?? "de");
  const [branding, setBranding] = useState<PublicConfig>(defaultBranding);

  useEffect(() => {
    let active = true;
    void fetch("/admin/api/config", { credentials: "include", headers: { Accept: "application/json" } })
      .then(async (response) => (response.ok ? response.json() : null))
      .then((payload: unknown) => {
        if (!active || !payload || typeof payload !== "object") {
          return;
        }
        const next = sanitizeConfig(payload as Record<string, unknown>);
        setBranding(next);
        if (!readLocale()) {
          setLocaleState(next.default_locale);
        }
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = branding.site_title;
    if (isHexColor(branding.primary_color)) {
      document.documentElement.style.setProperty("--brand", branding.primary_color);
    }
  }, [locale, branding]);

  const value = useMemo(
    () => ({
      locale,
      branding,
      t: (key: string, params?: Record<string, string | number>) => translate(locale, key, params),
      setLocale: (next: Locale) => {
        localStorage.setItem(storageKey, next);
        setLocaleState(next);
      },
    }),
    [locale, branding],
  );
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

function readLocale(): Locale | null {
  const stored = localStorage.getItem(storageKey);
  return stored && isLocale(stored) ? stored : null;
}

function sanitizeConfig(payload: Record<string, unknown>): PublicConfig {
  const locale = typeof payload.default_locale === "string" && isLocale(payload.default_locale) ? payload.default_locale : "de";
  const fallback = typeof payload.fallback_locale === "string" && isLocale(payload.fallback_locale) ? payload.fallback_locale : "en";
  const color = typeof payload.primary_color === "string" && isHexColor(payload.primary_color) ? payload.primary_color : defaultBranding.primary_color;
  const currency = typeof payload.currency === "string" && isCurrency(payload.currency) ? payload.currency : defaultBranding.currency;
  return {
    site_title: label(payload.site_title, defaultBranding.site_title),
    site_header: label(payload.site_header, defaultBranding.site_header),
    app_name: label(payload.app_name, defaultBranding.app_name),
    primary_color: color,
    default_locale: locale,
    fallback_locale: fallback,
    supported_locales: ["de", "en"],
    currency,
  };
}

function label(value: unknown, fallback: string): string {
  if (typeof value !== "string") {
    return fallback;
  }
  const trimmed = value.trim();
  if (!trimmed || trimmed.length > 80 || /[<>]/.test(trimmed)) {
    return fallback;
  }
  return trimmed;
}
