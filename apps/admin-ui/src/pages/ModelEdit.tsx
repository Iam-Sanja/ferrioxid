import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { modelSchema, request } from "../api/client";
import { DynamicForm } from "../components/DynamicForm";
import type { Schema } from "../types";

export function ModelEdit() {
  const { model = "", id = "" } = useParams();
  const navigate = useNavigate();
  const [schema, setSchema] = useState<Schema | null>(null);
  const [record, setRecord] = useState<Record<string, unknown> | null>(null);
  useEffect(() => {
    void Promise.all([
      modelSchema(model),
      request("GET", `/admin/api/models/${model}/records/${id}`) as Promise<Record<string, unknown>>,
    ]).then(([loaded, row]) => {
      setSchema(loaded);
      setRecord(row);
    });
  }, [model, id]);
  if (!schema || !record) return <p>Datensatz wird geladen.</p>;
  return (
    <section>
      <h2 className="mb-4 text-3xl font-semibold">{schema.label} bearbeiten</h2>
      <DynamicForm model={model} fields={schema.fields} initial={record} submitLabel="Änderungen speichern" onSubmit={async (values) => {
        const writable = Object.fromEntries(schema.fields.filter((field) => !field.readonly).map((field) => [field.name, values[field.name]]));
        await request("PATCH", `/admin/api/models/${model}/records/${id}`, writable);
        navigate(`/models/${model}/${id}`);
      }} />
    </section>
  );
}
