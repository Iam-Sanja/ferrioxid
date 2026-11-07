import type { Locale } from "./index";

const intlLocale: Record<Locale, string> = { de: "de-DE", en: "en-US" };

export function formatNumber(locale: Locale, value: number): string {
  return new Intl.NumberFormat(intlLocale[locale], { maximumFractionDigits: 0 }).format(Math.trunc(value));
}

export function formatDecimal(locale: Locale, value: number, digits = 2): string {
  return new Intl.NumberFormat(intlLocale[locale], {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

export function formatDate(locale: Locale, value: Date): string {
  return new Intl.DateTimeFormat(intlLocale[locale], { day: "2-digit", month: "2-digit", year: "numeric" }).format(value);
}

export function formatTime(locale: Locale, value: Date): string {
  return new Intl.DateTimeFormat(intlLocale[locale], { hour: "2-digit", minute: "2-digit" }).format(value);
}

export function formatDateTime(locale: Locale, value: Date): string {
  return `${formatDate(locale, value)} ${formatTime(locale, value)}`;
}

/** Stellt Integer-Cents dar. Der gespeicherte Wert bleibt eine ganze Zahl. */
export function formatCurrency(locale: Locale, cents: number, currency: string): string {
  const negative = cents < 0;
  const absolute = Math.abs(Math.trunc(cents));
  const major = Math.trunc(absolute / 100);
  const minor = absolute % 100;
  const amount = Number(`${negative ? "-" : ""}${major}.${String(minor).padStart(2, "0")}`);
  return new Intl.NumberFormat(intlLocale[locale], { style: "currency", currency }).format(amount);
}
