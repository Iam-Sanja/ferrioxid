import type { ApiError } from "../types";
import { translate, type Locale } from "./index";

export function translateError(
  locale: Locale,
  error: ApiError,
  fieldLabel?: string,
): string {
  if (error.code) {
    const key = `errors.${error.code}`;
    const translated = translate(locale, key, { field: fieldLabel ?? error.field ?? "" });
    if (translated !== key) {
      return translated;
    }
  }
  if (!safeDetail(error.detail)) {
    return translate(locale, "errors.unknown");
  }
  return error.detail || translate(locale, "errors.unknown");
}

function safeDetail(detail: string): boolean {
  const lower = detail.toLowerCase();
  return (
    detail.length <= 240 &&
    !lower.includes("password_hash") &&
    !lower.includes("select ") &&
    !lower.includes("stack")
  );
}
