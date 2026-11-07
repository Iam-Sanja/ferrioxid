import { Link } from "react-router-dom";
import { formatCurrency, formatDateTime, formatNumber } from "../i18n/format";
import { useI18n } from "../i18n";
import type { Field } from "../types";

type Props = {
  model: string;
  columns: Field[];
  rows: Array<Record<string, unknown>>;
  page: number;
  pageSize?: number;
  total?: number;
  hasMore: boolean;
  sort: string;
  onSort: (field: string) => void;
  onPage: (page: number) => void;
  filtered?: boolean;
};

export function DataTable({ model, columns, rows, page, pageSize = 25, total, hasMore, sort, onSort, onPage, filtered = false }: Props) {
  const { locale, branding, t } = useI18n();
  if (rows.length === 0) {
    return <p className="empty-state">{filtered ? t("common.emptyResults") : t("common.emptyRecords")}</p>;
  }
  const from = (page - 1) * pageSize + 1;
  const to = from + rows.length - 1;
  return (
    <div className="data-table shadow-sm dark:bg-slate-900">
      <table className="min-w-full text-left text-sm">
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column.name} className="px-4 py-3" aria-sort={sort === column.name ? "ascending" : sort === `-${column.name}` ? "descending" : "none"}>
                {column.sortable || column.name === "id" ? (
                  <button type="button" className="font-semibold" onClick={() => onSort(column.name)} aria-label={`${t("admin.sort")} ${column.label}`}>
                    {column.label}{sort === column.name ? " ↑" : sort === `-${column.name}` ? " ↓" : ""}
                  </button>
                ) : column.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={String(row.id)} className="border-t dark:border-slate-800">
              {columns.map((column) => (
                <td key={column.name} className="px-4 py-3">
                  {column.name === "id" ? (
                    <Link className="font-medium text-teal-700 underline" to={`/models/${model}/${String(row.id)}`}>{String(row[column.name] ?? "")}</Link>
                  ) : display(column, row[column.name], locale, branding.currency, t)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-between gap-3 px-4 py-3">
        <button type="button" className="btn btn-secondary" disabled={page <= 1} onClick={() => onPage(page - 1)}>{t("common.back")}</button>
        <span>{total === undefined ? `${t("common.page")} ${page}` : `${from}–${to} ${t("common.pageOf")} ${formatNumber(locale, total)}`}</span>
        <button type="button" className="btn btn-secondary" disabled={!hasMore} onClick={() => onPage(page + 1)}>{t("common.next")}</button>
      </div>
    </div>
  );
}

function display(
  column: Field,
  value: unknown,
  locale: "de" | "en",
  currency: string,
  t: (key: string) => string,
): string {
  if (value === null || value === undefined || value === "") {
    return "";
  }
  if (column.name.endsWith("_cents") && typeof value === "number") {
    return formatCurrency(locale, value, currency);
  }
  if (column.type === "integer" && typeof value === "number") {
    return formatNumber(locale, value);
  }
  if (column.type === "boolean") {
    return value ? t("common.yes") : t("common.no");
  }
  if (typeof value === "string" && column.name.endsWith("_at") && !Number.isNaN(Date.parse(value))) {
    return formatDateTime(locale, new Date(value));
  }
  return String(value);
}
