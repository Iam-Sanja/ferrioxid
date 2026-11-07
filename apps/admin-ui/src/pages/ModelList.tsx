import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { modelSchema, request } from "../api/client";
import { DataTable } from "../components/DataTable";
import { useI18n } from "../i18n";
import type { Schema } from "../types";

export function ModelList() {
  const { t } = useI18n();
  const { model = "" } = useParams();
  const [schema, setSchema] = useState<Schema | null>(null);
  const [rows, setRows] = useState<Array<Record<string, unknown>>>([]);
  const [page, setPage] = useState(1);
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<Record<string, string>>({});
  const [sort, setSort] = useState("id");
  const [hasMore, setHasMore] = useState(false);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const pageSize = 25;

  useEffect(() => {
    setLoading(true);
    const direction = sort.startsWith("-") ? "desc" : "asc";
    const field = sort.replace(/^-/, "");
    const filterQuery = Object.entries(filters)
      .filter(([, value]) => value !== "")
      .map(([name, value]) => `filter.${encodeURIComponent(name)}=${encodeURIComponent(value)}`)
      .join("&");
    modelSchema(model)
      .then(async (loaded) => {
        setSchema(loaded);
        if (loaded.list_blocked) {
          setError(t("admin.listBlockedError"));
          setRows([]);
          return;
        }
        const payload = (await request(
          "GET",
          `/admin/api/models/${model}/records?page=${page}&page_size=${pageSize}&sort=${field}&dir=${direction}&q=${encodeURIComponent(query)}${filterQuery ? `&${filterQuery}` : ""}`,
        )) as { data: Array<Record<string, unknown>>; has_more: boolean; total?: number };
        setRows(payload.data);
        setHasMore(payload.has_more);
        setTotal(payload.total ?? payload.data.length);
        setError("");
      })
      .catch((caught) => setError(caught && typeof caught === "object" && "detail" in caught ? String((caught as { detail?: string }).detail ?? t("admin.listFailed")) : t("admin.listFailed")))
      .finally(() => setLoading(false));
  }, [model, page, query, sort, filters, t]);

  function toggleSort(field: string) {
    setPage(1);
    setSort((current) => (current === field ? `-${field}` : field));
  }

  if (loading) return <p role="status">{t("common.loading")}</p>;
  if (!schema) return <p role="alert">{error}</p>;
  const columns = schema.fields.filter((field) => field.list_display);
  const filterFields = schema.fields.filter((field) => field.filterable);
  return (
    <section>
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-3xl font-semibold">{schema.label}</h2>
        {schema.actions.includes("create") ? <Link className="btn btn-primary" to={`/models/${model}/new`}>{t("admin.newRecord")}</Link> : null}
      </div>
      <div className="mb-4 grid gap-3 md:grid-cols-2">
        <input aria-label={t("common.search")} className="search-input" value={query} onChange={(event) => { setPage(1); setQuery(event.target.value); }} placeholder={t("common.search")} />
        {filterFields.map((field) => (
          <input
            key={field.name}
            aria-label={`${t("common.filter")} ${field.label}`}
            className="search-input"
            value={filters[field.name] ?? ""}
            placeholder={`${t("common.filter")}: ${field.label}`}
            onChange={(event) => {
              setPage(1);
              setFilters((current) => ({ ...current, [field.name]: event.target.value }));
            }}
          />
        ))}
      </div>
      {error ? <p role="alert" className="status-note mb-4">{error}</p> : (
        <DataTable model={model} columns={columns} rows={rows} page={page} pageSize={pageSize} total={total} hasMore={hasMore} sort={sort} onSort={toggleSort} onPage={setPage} filtered={query !== "" || Object.values(filters).some((value) => value !== "")} />
      )}
    </section>
  );
}
