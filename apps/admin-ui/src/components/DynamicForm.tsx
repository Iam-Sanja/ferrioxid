import { FormEvent, useState } from "react";
import { fieldError } from "../api/client";
import { translateError } from "../i18n/errors";
import { useI18n } from "../i18n";
import type { ApiError, Field } from "../types";
import { FieldRenderer } from "./FieldRenderer";

type Props = {
  model: string;
  fields: Field[];
  initial: Record<string, unknown>;
  submitLabel: string;
  onSubmit: (values: Record<string, unknown>) => Promise<void>;
};

export function DynamicForm({ model, fields, initial, submitLabel, onSubmit }: Props) {
  const { locale, t } = useI18n();
  const editable = fields.filter((field) => !field.readonly);
  const [values, setValues] = useState(initial);
  const [failure, setFailure] = useState<ApiError | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setFailure(null);
    const missing = editable.find((field) => field.required && !field.nullable && (values[field.name] === "" || values[field.name] === undefined));
    if (missing) {
      setFailure({
        status: 422,
        code: "validation.required",
        field: missing.name,
        detail: `field:${missing.name}: ${t("validation.required")}`,
      });
      return;
    }
    const payload = Object.fromEntries(editable.map((field) => {
      const value = values[field.name];
      if (field.type === "integer" && typeof value === "string" && value !== "") {
        return [field.name, Number(value)];
      }
      return [field.name, value === "" && field.nullable ? null : value];
    }));
    try {
      await onSubmit(payload);
    } catch (caught) {
      setFailure(readError(caught, t("admin.saveFailed")));
    }
  }

  return (
    <form onSubmit={submit} className="space-y-4 rounded-2xl bg-white p-6 shadow-sm dark:bg-slate-900">
      {editable.map((field) => {
        const message = messageFor(locale, failure, field);
        return (
          <FieldRenderer
            key={field.name}
            model={model}
            field={field}
            value={values[field.name]}
            error={message ?? undefined}
            onChange={(value) => setValues((current) => ({ ...current, [field.name]: value }))}
          />
        );
      })}
      {failure && !failure.field && !failure.detail.startsWith("field:") ? (
        <p role="alert" className="field-error">{translateError(locale, failure)}</p>
      ) : null}
      <button className="btn btn-primary" type="submit">{submitLabel}</button>
    </form>
  );
}

function messageFor(locale: "de" | "en", failure: ApiError | null, field: Field): string | null {
  if (!failure) {
    return null;
  }
  if (failure.field === field.name || failure.detail.startsWith(`field:${field.name}:`)) {
    return translateError(locale, failure.field ? failure : { ...failure, code: failure.code ?? "validation.field", field: field.name }, field.label);
  }
  return fieldError(failure.detail, field.name);
}

function readError(caught: unknown, fallback: string): ApiError {
  if (caught && typeof caught === "object" && "detail" in caught) {
    const error = caught as ApiError;
    return { status: error.status ?? 0, detail: error.detail || fallback, code: error.code, field: error.field };
  }
  return { status: 0, detail: fallback };
}
