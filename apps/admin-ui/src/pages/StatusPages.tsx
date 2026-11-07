import { useI18n } from "../i18n";

export function Forbidden() {
  const { t } = useI18n();
  return (
    <main className="p-10">
      <h1 className="text-3xl font-semibold">403</h1>
      <p>{t("admin.forbiddenTitle")}</p>
      <p>{t("admin.forbiddenBody")}</p>
    </main>
  );
}

export function NotFound() {
  const { t } = useI18n();
  return (
    <main className="p-10">
      <h1 className="text-3xl font-semibold">404</h1>
      <p>{t("admin.notFoundTitle")}</p>
      <p>{t("admin.notFoundBody")}</p>
    </main>
  );
}
