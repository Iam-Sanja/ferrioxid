import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { modelSchema, request } from "../api/client";
import type { Schema } from "../types";

export function ModelDetail() {
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
  if (!schema || !record) return <p>Detail wird geladen.</p>;
  return (
    <section>
      <div className="mb-4 flex gap-3">
        <h2 className="text-3xl font-semibold">{schema.label}</h2>
        {schema.actions.includes("update") ? <Link className="rounded-lg border px-3 py-2" to={`/models/${model}/${id}/edit`}>Bearbeiten</Link> : null}
        {schema.actions.includes("delete") ? <button className="rounded-lg bg-red-700 px-3 py-2 text-white" type="button" onClick={() => setOpen(true)}>Löschen</button> : null}
      </div>
      <dl className="grid gap-3 rounded-2xl bg-white p-6 dark:bg-slate-900">
        {schema.fields.filter((field) => field.list_display || field.name === "id").map((field) => (
          <div key={field.name}><dt className="text-sm text-slate-500">{field.label}</dt><dd>{String(record[field.name] ?? "")}</dd></div>
        ))}
      </dl>
      {open ? (
        <dialog open className="fixed inset-0 grid place-items-center bg-black/40" aria-labelledby="delete-title">
          <form method="dialog" className="rounded-2xl bg-white p-6 dark:bg-slate-900">
            <h3 id="delete-title" className="text-xl font-semibold">Datensatz löschen?</h3>
            <p className="mt-2">Diese Aktion kann nicht rückgängig gemacht werden.</p>
            <div className="mt-4 flex gap-3">
              <button type="button" onClick={() => setOpen(false)}>Abbrechen</button>
              <button type="button" className="rounded-lg bg-red-700 px-3 py-2 text-white" onClick={() => {
                void request("DELETE", `/admin/api/models/${model}/records/${id}`).then(() => navigate(`/models/${model}`));
              }}>Endgültig löschen</button>
            </div>
          </form>
        </dialog>
      ) : null}
    </section>
  );
}
