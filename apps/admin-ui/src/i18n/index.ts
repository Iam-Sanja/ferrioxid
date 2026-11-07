import { createContext, useContext } from "react";
import deAdmin from "./locales/de/admin.json";
import deAuth from "./locales/de/auth.json";
import deCommon from "./locales/de/common.json";
import deDirectory from "./locales/de/directory.json";
import deErrors from "./locales/de/errors.json";
import deValidation from "./locales/de/validation.json";
import enAdmin from "./locales/en/admin.json";
import enAuth from "./locales/en/auth.json";
import enCommon from "./locales/en/common.json";
import enDirectory from "./locales/en/directory.json";
import enErrors from "./locales/en/errors.json";
import enValidation from "./locales/en/validation.json";

export type Locale = "de" | "en";

export type PublicConfig = {
  site_title: string;
  site_header: string;
  app_name: string;
  primary_color: string;
  default_locale: Locale;
  fallback_locale: Locale;
  supported_locales: Locale[];
  currency: string;
};

type Tree = { [key: string]: string | Tree };

const de = {
  common: deCommon,
  auth: deAuth,
  admin: deAdmin,
  directory: deDirectory,
  validation: deValidation,
  errors: deErrors,
} satisfies Tree;

const en = {
  common: enCommon,
  auth: enAuth,
  admin: enAdmin,
  directory: enDirectory,
  validation: enValidation,
  errors: enErrors,
} satisfies Tree;

const catalogs: Record<Locale, Tree> = { de, en };

export const defaultBranding: PublicConfig = {
  site_title: "Ferrum Admin",
  site_header: "Ferrum Verwaltung",
  app_name: "Ferrum",
  primary_color: "#0f172a",
  default_locale: "de",
  fallback_locale: "en",
  supported_locales: ["de", "en"],
  currency: "EUR",
};

export function translate(
  locale: Locale,
  key: string,
  params?: Record<string, string | number>,
): string {
  return interpolate(resolveMessage(locale, key), params);
}

export function resolveMessage(locale: Locale, key: string): string {
  return lookup(catalogs[locale], key) ?? lookup(catalogs.en, key) ?? key;
}

export function lookup(tree: Tree, key: string): string | undefined {
  let node: string | Tree | undefined = tree;
  for (const part of key.split(".")) {
    if (!node || typeof node === "string" || !(part in node)) {
      return undefined;
    }
    node = node[part];
  }
  return typeof node === "string" ? node : undefined;
}

export function flattenKeys(tree: Tree, prefix = ""): string[] {
  return Object.entries(tree).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof value === "string" ? [path] : flattenKeys(value, path);
  });
}

export function placeholders(template: string): string[] {
  return [...template.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
}

export function interpolate(template: string, params?: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (token, name: string) =>
    params && params[name] !== undefined ? String(params[name]) : token,
  );
}

export function isLocale(value: string): value is Locale {
  return value === "de" || value === "en";
}

export function isHexColor(value: string): boolean {
  return /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(value);
}

export function isCurrency(value: string): boolean {
  return /^[A-Z]{3}$/.test(value);
}

type I18nValue = {
  locale: Locale;
  t: (key: string, params?: Record<string, string | number>) => string;
  setLocale: (locale: Locale) => void;
  branding: PublicConfig;
};

const defaultValue: I18nValue = {
  locale: "de",
  t: (key, params) => translate("de", key, params),
  setLocale: () => undefined,
  branding: defaultBranding,
};

export const I18nContext = createContext<I18nValue>(defaultValue);

export function useI18n(): I18nValue {
  return useContext(I18nContext);
}
