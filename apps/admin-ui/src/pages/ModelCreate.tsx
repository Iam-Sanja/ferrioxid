import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { modelSchema, request } from "../api/client";
import { DynamicForm } from "../components/DynamicForm";
import type { Schema } from "../types";

export function ModelCreate() {
  const { model = "" } = useParams();
  const navigate = useNavigate();
  const [schema, setSchema] = useState<Schema | null>(null);
  useEffect(() => { void modelSchema(model).then(setSchema); }, [model]);
  if (!schema) return <p>Formular wird geladen.</p>;
  if (!schema.actions.includes("create")) return <p role="alert">Erstellen ist für dieses Modell nicht erlaubt.</p>;
  const initial = Object.fromEntries(schema.fields.filter((field) => !field.readonly).map((field) => [field.name, field.type === "boolean" ? false : ""]));
  return (
    <section>
      <h2 className="mb-4 text-3xl font-semibold">{schema.label} anlegen</h2>
      <DynamicForm model={model} fields={schema.fields} initial={initial} submitLabel="Speichern" onSubmit={async (values) => {
        const created = (await request("POST", `/admin/api/models/${model}/records`, values)) as { id: string };
        navigate(`/models/${model}/${created.id}`);
      }} />
    </section>
  );
}
