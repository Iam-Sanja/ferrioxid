import { useEffect, useState } from "react";
import { request } from "../api/client";
import type { Field } from "../types";

type Props = {
  model: string;
  field: Field;
  value: unknown;
  error?: string;
  onChange: (value: unknown) => void;
};

export function FieldRenderer({ model, field, value, error, onChange }: Props) {
  const id = `${model}-${field.name}`;
  if (field.foreign_key) {
    return <RelationSelect id={id} model={model} field={field} value={value} error={error} onChange={onChange} />;
  }
  if (field.type === "boolean") {
    return (
      <label className="flex items-center gap-2" htmlFor={id}>
        <input id={id} type="checkbox" checked={Boolean(value)} onChange={(event) => onChange(event.target.checked)} />
        {field.label}
      </label>
    );
  }
  return (
    <div>
      <label className="block text-sm font-medium" htmlFor={id}>{field.label}</label>
      <input
        id={id}
        className="mt-1 w-full rounded-lg border px-3 py-2 dark:bg-slate-800"
        type={field.type === "integer" ? "number" : "text"}
        required={field.required && !field.nullable}
        readOnly={field.readonly}
        value={value === null || value === undefined ? "" : String(value)}
        onChange={(event) => onChange(field.type === "integer" ? Number(event.target.value) : event.target.value)}
      />
      {error ? <p className="mt-1 text-sm text-red-700">{error}</p> : null}
    </div>
  );
}

function RelationSelect({ id, model, field, value, error, onChange }: Props & { id: string }) {
  const [options, setOptions] = useState<Array<{ id: string; label: string }>>([]);
  const [message, setMessage] = useState("");
  useEffect(() => {
    request("GET", `/admin/api/models/${model}/fields/${field.name}/options`)
      .then((payload) => setOptions((payload as { options: Array<{ id: string; label: string }> }).options))
      .catch((caught) => setMessage(caught && typeof caught === "object" && "detail" in caught ? String(caught.detail) : "Auswahl nicht verfügbar"));
  }, [model, field.name]);
  return (
    <div>
      <label className="block text-sm font-medium" htmlFor={id}>{field.label}</label>
      {message ? <p role="alert" className="mt-1 text-sm text-amber-800">{message}</p> : null}
      <select id={id} className="mt-1 w-full rounded-lg border px-3 py-2 dark:bg-slate-800" value={value === null || value === undefined ? "" : String(value)} onChange={(event) => onChange(event.target.value || null)}>
        <option value="">{field.nullable ? "Keine Auswahl" : "Bitte wählen"}</option>
        {options.map((option) => <option key={option.id} value={option.id}>{option.label}</option>)}
      </select>
      {error ? <p className="mt-1 text-sm text-red-700">{error}</p> : null}
    </div>
  );
}
