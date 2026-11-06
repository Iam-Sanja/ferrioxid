import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { modelSchema, request } from "../api/client";
import { DataTable } from "../components/DataTable";
import type { Schema } from "../types";

export function ModelList() {
  const { model = "" } = useParams();
  const [schema, setSchema] = useState<Schema | null>(null);
  const [rows, setRows] = useState<Array<Record<string, unknown>>>([]);
  const [page, setPage] = useState(1);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState("id");
  const [hasMore, setHasMore] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    modelSchema(model)
      .then(async (loaded) => {
        setSchema(loaded);
        if (loaded.list_blocked) {
          setError("Diese Liste ist blockiert, weil eine Objekt-Policy keinen SQL-Filter besitzt.");
          setRows([]);
          return;
        }
        const payload = (await request("GET", `/admin/api/models/${model}/records?page=${page}&page_size=25&sort=${sort}&q=${encodeURIComponent(query)}`)) as {
          data: Array<Record<string, unknown>>;
          has_more: boolean;
        };
        setRows(payload.data);
        setHasMore(payload.has_more);
        setError("");
      })
      .catch((caught) => setError(caught && typeof caught === "object" && "detail" in caught ? String(caught.detail) : "Liste konnte nicht geladen werden"))
      .finally(() => setLoading(false));
  }, [model, page, query, sort]);

  if (loading) return <p>Daten werden geladen.</p>;
  if (!schema) return <p role="alert">{error}</p>;
  const columns = schema.fields.filter((field) => field.list_display);
  return (
    <section>
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-3xl font-semibold">{schema.label}</h2>
        {schema.actions.includes("create") ? <Link className="rounded-lg bg-teal-700 px-4 py-2 text-white" to={`/models/${model}/new`}>Neu anlegen</Link> : null}
      </div>
      <input aria-label="Suche" className="mb-4 w-full max-w-md rounded-lg border px-3 py-2" value={query} onChange={(event) => { setPage(1); setQuery(event.target.value); }} placeholder="Suche" />
      {error ? <p role="alert" className="mb-4 rounded-lg bg-amber-50 p-4 text-amber-900">{error}</p> : (
        <DataTable model={model} columns={columns} rows={rows} page={page} hasMore={hasMore} sort={sort} onSort={setSort} onPage={setPage} />
      )}
    </section>
  );
}
