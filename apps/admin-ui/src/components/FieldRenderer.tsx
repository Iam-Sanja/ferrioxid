import { useEffect, useState } from "react";
import { request } from "../api/client";
import { translateError } from "../i18n/errors";
import { useI18n } from "../i18n";
import type { ApiError, Field } from "../types";

type Props = {
  model: string;
  field: Field;
  value: unknown;
  error?: string;
  onChange: (value: unknown) => void;
};

export function FieldRenderer({ model, field, value, error, onChange }: Props) {
  const { t } = useI18n();
  const id = `${model}-${field.name}`;
  const errorId = error ? `${id}-error` : undefined;
  if (field.foreign_key) {
    return <RelationSelect id={id} model={model} field={field} value={value} error={error} onChange={onChange} />;
  }
  if (field.type === "boolean") {
    return (
      <label className="switch" htmlFor={id}>
        <input id={id} type="checkbox" checked={Boolean(value)} disabled={field.readonly} onChange={(event) => onChange(event.target.checked)} />
        {field.label}
      </label>
    );
  }
  return (
    <div className="field">
      <label htmlFor={id}>
        {field.label}
        {field.required ? <span className="required-mark" aria-hidden="true">*</span> : null}
        {field.required ? <span className="sr-only"> {t("common.requiredMark")}</span> : null}
      </label>
      <input
        id={id}
        type={field.type === "integer" ? "text" : "text"}
        inputMode={field.type === "integer" ? "numeric" : undefined}
        required={field.required && !field.nullable}
        readOnly={field.readonly}
        aria-invalid={error ? true : undefined}
        aria-describedby={errorId}
        value={value === null || value === undefined ? "" : String(value)}
        onChange={(event) => onChange(event.target.value)}
      />
      {error ? <p id={errorId} className="field-error" role="alert">{error}</p> : null}
    </div>
  );
}

function RelationSelect({ id, model, field, value, error, onChange }: Props & { id: string }) {
  const { locale, t } = useI18n();
  const [options, setOptions] = useState<Array<{ id: string; label: string }>>([]);
  const [message, setMessage] = useState("");
  useEffect(() => {
    request("GET", `/admin/api/models/${model}/fields/${field.name}/options`)
      .then((payload) => setOptions((payload as { options: Array<{ id: string; label: string }> }).options))
      .catch((caught) => setMessage(isApiError(caught) ? translateError(locale, caught) : t("admin.optionsFailed")));
  }, [model, field.name, locale, t]);
  const errorId = error ? `${id}-error` : undefined;
  return (
    <div className="field">
      <label htmlFor={id}>
        {field.label}
        {field.required ? <span className="required-mark" aria-hidden="true">*</span> : null}
      </label>
      {message ? <p role="alert" className="mt-1 text-sm text-amber-800">{message}</p> : null}
      <select
        id={id}
        aria-invalid={error ? true : undefined}
        aria-describedby={errorId}
        className="mt-1 w-full"
        value={value === null || value === undefined ? "" : String(value)}
        onChange={(event) => onChange(event.target.value || null)}
      >
        <option value="">{field.nullable ? t("admin.none") : t("admin.choose")}</option>
        {options.map((option) => <option key={option.id} value={option.id}>{option.label}</option>)}
      </select>
      {error ? <p id={errorId} className="field-error" role="alert">{error}</p> : null}
    </div>
  );
}

function isApiError(value: unknown): value is ApiError {
  return Boolean(value && typeof value === "object" && "detail" in value && "status" in value);
}
