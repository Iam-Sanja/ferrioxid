import { FormEvent, useState } from "react";
import { fieldError } from "../api/client";
import type { Field } from "../types";
import { FieldRenderer } from "./FieldRenderer";

type Props = {
  model: string;
  fields: Field[];
  initial: Record<string, unknown>;
  submitLabel: string;
  onSubmit: (values: Record<string, unknown>) => Promise<void>;
};

export function DynamicForm({ model, fields, initial, submitLabel, onSubmit }: Props) {
  const editable = fields.filter((field) => !field.readonly);
  const [values, setValues] = useState(initial);
  const [error, setError] = useState("");

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError("");
    const missing = editable.find((field) => field.required && !field.nullable && (values[field.name] === "" || values[field.name] === undefined));
    if (missing) {
      setError(`field:${missing.name}: Feld ist erforderlich`);
      return;
    }
    const payload = Object.fromEntries(editable.map((field) => {
      const value = values[field.name];
      return [field.name, value === "" && field.nullable ? null : value];
    }));
    try {
      await onSubmit(payload);
    } catch (caught) {
      setError(caught && typeof caught === "object" && "detail" in caught ? String(caught.detail) : "Speichern fehlgeschlagen");
    }
  }

  return (
    <form onSubmit={submit} className="space-y-4 rounded-2xl bg-white p-6 shadow-sm dark:bg-slate-900">
      {editable.map((field) => (
        <FieldRenderer
          key={field.name}
          model={model}
          field={field}
          value={values[field.name]}
          error={fieldError(error, field.name) ?? undefined}
          onChange={(value) => setValues((current) => ({ ...current, [field.name]: value }))}
        />
      ))}
      {error && !error.startsWith("field:") ? <p role="alert" className="text-sm text-red-700">{error}</p> : null}
      <button className="rounded-lg bg-teal-700 px-4 py-2 text-white" type="submit">{submitLabel}</button>
    </form>
  );
}
