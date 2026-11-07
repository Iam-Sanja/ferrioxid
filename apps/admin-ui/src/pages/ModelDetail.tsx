import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { modelSchema, request } from "../api/client";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { useI18n } from "../i18n";
import type { Schema } from "../types";

export function ModelDetail() {
  const { t } = useI18n();
  const { model = "", id = "" } = useParams();
  const navigate = useNavigate();
  const [schema, setSchema] = useState<Schema | null>(null);
  const [record, setRecord] = useState<Record<string, unknown> | null>(null);
  const [open, setOpen] = useState(false);
  useEffect(() => {
    void Promise.all([
      modelSchema(model),
      request("GET", `/admin/api/models/${model}/records/${id}`) as Promise<Record<string, unknown>>,
    ]).then(([loaded, row]) => { setSchema(loaded); setRecord(row); });
  }, [model, id]);
  if (!schema || !record) return <p role="status">{t("admin.loadingDetail")}</p>;
  return (
    <section>
      <div className="mb-4 flex flex-wrap gap-3">
        <h2 className="text-3xl font-semibold">{schema.label}</h2>
        {schema.actions.includes("update") ? <Link className="btn btn-secondary" to={`/models/${model}/${id}/edit`}>{t("admin.edit")}</Link> : null}
        {schema.actions.includes("delete") ? <button className="btn btn-danger" type="button" onClick={() => setOpen(true)}>{t("admin.delete")}</button> : null}
      </div>
      <dl className="grid gap-3 rounded-2xl bg-white p-6 dark:bg-slate-900">
        {schema.fields.filter((field) => field.list_display || field.name === "id").map((field) => (
          <div key={field.name}><dt className="text-sm text-slate-500">{field.label}</dt><dd>{String(record[field.name] ?? "")}</dd></div>
        ))}
      </dl>
      {open ? (
        <ConfirmDialog
          title={t("admin.deleteTitle")}
          body={t("admin.deleteBody")}
          confirmLabel={t("admin.deleteForever")}
          onCancel={() => setOpen(false)}
          onConfirm={() => {
            void request("DELETE", `/admin/api/models/${model}/records/${id}`).then(() => navigate(`/models/${model}`));
          }}
        />
      ) : null}
    </section>
  );
}
