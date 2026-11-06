import { Link } from "react-router-dom";
import type { Field } from "../types";

type Props = {
  model: string;
  columns: Field[];
  rows: Array<Record<string, unknown>>;
  page: number;
  hasMore: boolean;
  sort: string;
  onSort: (field: string) => void;
  onPage: (page: number) => void;
};

export function DataTable({ model, columns, rows, page, hasMore, sort, onSort, onPage }: Props) {
  if (rows.length === 0) {
    return <p className="rounded-xl bg-white p-6 dark:bg-slate-900">Keine Datensätze vorhanden.</p>;
  }
  return (
    <div className="overflow-x-auto rounded-xl bg-white shadow-sm dark:bg-slate-900">
      <table className="min-w-full text-left text-sm">
        <thead className="bg-slate-50 dark:bg-slate-800">
          <tr>
            {columns.map((column) => (
              <th key={column.name} className="px-4 py-3">
                {column.sortable || column.name === "id" ? (
                  <button type="button" onClick={() => onSort(column.name)} aria-pressed={sort === column.name}>{column.label}</button>
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
                  {column.name === "id" ? <Link className="text-teal-700 underline" to={`/models/${model}/${String(row.id)}`}>{String(row[column.name] ?? "")}</Link> : String(row[column.name] ?? "")}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-between px-4 py-3">
        <button type="button" disabled={page <= 1} onClick={() => onPage(page - 1)}>Zurück</button>
        <span>Seite {page}</span>
        <button type="button" disabled={!hasMore} onClick={() => onPage(page + 1)}>Weiter</button>
      </div>
    </div>
  );
}
