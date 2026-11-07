import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, expect, it, vi } from "vitest";
import { fieldError, login, request, restoreSession } from "./api/client";
import { flattenKeys, placeholders, resolveMessage, translate } from "./i18n";
import { translateError } from "./i18n/errors";
import { formatCurrency, formatDate, formatNumber } from "./i18n/format";
import { I18nProvider } from "./i18n/provider";
import deAdmin from "./i18n/locales/de/admin.json";
import deAuth from "./i18n/locales/de/auth.json";
import deCommon from "./i18n/locales/de/common.json";
import deDirectory from "./i18n/locales/de/directory.json";
import deErrors from "./i18n/locales/de/errors.json";
import deValidation from "./i18n/locales/de/validation.json";
import enAdmin from "./i18n/locales/en/admin.json";
import enAuth from "./i18n/locales/en/auth.json";
import enCommon from "./i18n/locales/en/common.json";
import enDirectory from "./i18n/locales/en/directory.json";
import enErrors from "./i18n/locales/en/errors.json";
import enValidation from "./i18n/locales/en/validation.json";
import { Layout } from "./components/Layout";
import { AuthProvider } from "./auth/AuthProvider";
import { LoginPage } from "./auth/LoginPage";
import { DataTable } from "./components/DataTable";
import { DynamicForm } from "./components/DynamicForm";
import { FieldRenderer } from "./components/FieldRenderer";
import { Sidebar } from "./components/Sidebar";
import { ModelCreate } from "./pages/ModelCreate";
import { ModelDetail } from "./pages/ModelDetail";
import { ModelList } from "./pages/ModelList";
import { Directory } from "./pages/Directory";
import type { Field } from "./types";

const name: Field = {
  name: "name",
  label: "Name",
  type: "string",
  required: true,
  readonly: false,
  nullable: false,
  searchable: true,
  sortable: true,
  filterable: false,
  list_display: true,
  foreign_key: null,
};

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url === "/auth/me") return json({ detail: "unauthenticated" }, 401);
    if (url === "/auth/login") return json({ user: { id: "1", email: "staff@example.com", is_staff: true, is_superuser: false }, csrf_token: "fresh" });
    if (url === "/admin/api/models") return json({ models: [{ key: "products", label: "Produkte", list_blocked: false }] });
    if (url.endsWith("/schema")) {
      return json({ model: "products", label: "Produkte", list_blocked: false, actions: ["read", "create", "delete"], fields: [name] });
    }
    if (url.includes("/records")) return json({ id: "abc", name: "Alpha" }, 201);
    return json({ detail: "unerwartet" }, 500);
  }));
});

it("meldet ein Login und speichert das CSRF-Token nur im Speicher", async () => {
  const user = await login("staff@example.com", "secret");
  expect(user.email).toBe("staff@example.com");
  await request("POST", "/admin/api/models/products/records", { name: "A" });
  const calls = vi.mocked(fetch).mock.calls;
  const write = calls.find((call) => String(call[0]).includes("/records"));
  const headers = new Headers(write?.[1]?.headers);
  expect(headers.get("x-csrf-token")).toBe("fresh");
  expect(localStorage.getItem("csrf")).toBeNull();
});

it("zeigt die Anmeldung und danach die Navigation", async () => {
  render(
    <MemoryRouter initialEntries={["/login"]}>
      <AuthProvider>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route path="/" element={<Sidebar models={[{ key: "products", label: "Produkte", list_blocked: false }]} />} />
        </Routes>
      </AuthProvider>
    </MemoryRouter>,
  );
  fireEvent.change(screen.getByLabelText("E-Mail"), { target: { value: "staff@example.com" } });
  fireEvent.change(screen.getByLabelText("Passwort"), { target: { value: "secret" } });
  fireEvent.click(screen.getByRole("button", { name: "Anmelden" }));
  expect(await screen.findByRole("link", { name: "Produkte" })).toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "Benutzer und Rollen" })).not.toBeInTheDocument();
});

it("rendert Tabellen, leere Zustände und Formularvalidierung", async () => {
  const { rerender } = render(
    <MemoryRouter>
      <DataTable model="products" columns={[name]} rows={[]} page={1} hasMore={false} sort="id" onSort={() => undefined} onPage={() => undefined} />
    </MemoryRouter>,
  );
  expect(screen.getByText("Keine Datensätze vorhanden.")).toBeInTheDocument();
  rerender(
    <MemoryRouter>
      <DataTable model="products" columns={[name]} rows={[{ id: "abc", name: "Alpha" }]} page={1} hasMore={false} sort="name" onSort={() => undefined} onPage={() => undefined} />
    </MemoryRouter>,
  );
  expect(screen.getByRole("cell", { name: "Alpha" })).toBeInTheDocument();
  render(<DynamicForm model="products" fields={[name]} initial={{ name: "" }} submitLabel="Speichern" onSubmit={async () => undefined} />);
  fireEvent.submit(screen.getByRole("button", { name: "Speichern" }).closest("form")!);
  expect(screen.getByText("Feld ist erforderlich")).toBeInTheDocument();
  expect(fieldError("field:name: Feld ist erforderlich", "name")).toBe("Feld ist erforderlich");
});

it("blockiert Erstellen ohne Berechtigung und bestätigt das Löschen", async () => {
  vi.mocked(fetch).mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url.endsWith("/schema")) return json({ model: "products", label: "Produkte", list_blocked: false, actions: ["read"], fields: [name] });
    if (url.includes("/records/")) return json({ id: "abc", name: "Alpha" });
    return json({ detail: "Fehler" }, 403);
  });
  render(
    <MemoryRouter initialEntries={["/models/products/new"]}>
      <Routes>
        <Route path="/models/:model/new" element={<ModelCreate />} />
      </Routes>
    </MemoryRouter>,
  );
  expect(await screen.findByRole("alert")).toHaveTextContent("nicht erlaubt");
  render(
    <MemoryRouter initialEntries={["/models/products/abc"]}>
      <Routes>
        <Route path="/models/:model/:id" element={<ModelDetail />} />
      </Routes>
    </MemoryRouter>,
  );
  expect(screen.queryByRole("button", { name: "Löschen" })).not.toBeInTheDocument();
});

it("zeigt den Löschdialog erst nach Bestätigung", async () => {
  vi.mocked(fetch).mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url.endsWith("/schema")) {
      return json({ model: "products", label: "Produkte", list_blocked: false, actions: ["read", "delete"], fields: [name] });
    }
    return json({ id: "abc", name: "Alpha" });
  });
  render(
    <MemoryRouter initialEntries={["/models/products/abc"]}>
      <Routes>
        <Route path="/models/:model/:id" element={<ModelDetail />} />
      </Routes>
    </MemoryRouter>,
  );
  fireEvent.click(await screen.findByRole("button", { name: "Löschen" }));
  expect(screen.getByText("Datensatz löschen?")).toBeInTheDocument();
});

it("stellt die Sitzung nach einem Reload her und behält dasselbe CSRF-Token", async () => {
  vi.mocked(fetch).mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url === "/auth/me") {
      return json({ id: "1", email: "staff@example.com", is_active: true, is_staff: true, is_superuser: false });
    }
    if (url === "/auth/csrf") return json({ csrf_token: "stable" });
    return json({ id: "1", email: "staff@example.com", is_active: true, is_staff: true, is_superuser: false });
  });
  expect((await restoreSession())?.email).toBe("staff@example.com");
  expect((await restoreSession())?.email).toBe("staff@example.com");
  await request("POST", "/admin/api/models/products/records", { name: "A" });
  const headers = new Headers(vi.mocked(fetch).mock.calls.at(-1)?.[1]?.headers);
  expect(headers.get("x-csrf-token")).toBe("stable");
  expect(localStorage.getItem("csrf")).toBeNull();
});

it("gibt Fehlerantworten als Status und Detail weiter", async () => {
  vi.mocked(fetch).mockImplementation(async () => json({ detail: "Access denied" }, 403));
  await expect(request("GET", "/admin/api/models")).rejects.toMatchObject({
    status: 403,
    detail: "Access denied",
  });
});

it("lädt Fremdschlüssel aus der Options-API", async () => {
  vi.mocked(fetch).mockImplementation(async () => json({ options: [{ id: "c1", label: "Buecher" }] }));
  render(
    <FieldRenderer
      model="products"
      field={{ ...name, name: "category_id", label: "Kategorie", foreign_key: "admin_categories.id", nullable: true, required: false }}
      value={null}
      onChange={() => undefined}
    />,
  );
  expect(await screen.findByRole("option", { name: "Buecher" })).toBeInTheDocument();
});

it("zeigt die Rollenverwaltung nach einem Superuser-Login", async () => {
  vi.mocked(fetch).mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url === "/auth/me") return json({ detail: "unauthenticated" }, 401);
    if (url === "/auth/login") {
      return json({
        user: { id: "1", email: "root@example.com", is_active: true, is_staff: true, is_superuser: true },
        csrf_token: "root",
      });
    }
    if (url === "/admin/api/me") {
      return json({ id: "1", email: "root@example.com", is_active: true, is_staff: true, is_superuser: true });
    }
    return json({ models: [] });
  });
  render(
    <MemoryRouter initialEntries={["/login"]}>
      <AuthProvider>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route path="/" element={<Sidebar models={[]} />} />
        </Routes>
      </AuthProvider>
    </MemoryRouter>,
  );
  expect(await screen.findByRole("button", { name: "Anmelden" })).toBeInTheDocument();
  await new Promise((resolve) => setTimeout(resolve, 0));
  fireEvent.change(screen.getByLabelText("E-Mail"), { target: { value: "root@example.com" } });
  fireEvent.change(screen.getByLabelText("Passwort"), { target: { value: "secret" } });
  fireEvent.click(screen.getByRole("button", { name: "Anmelden" }));
  expect(await screen.findByRole("link", { name: "Benutzer und Rollen" })).toBeInTheDocument();
});

it("kennt deutsche und englische Katalogtexte", () => {
  const german = {
    common: deCommon,
    auth: deAuth,
    admin: deAdmin,
    directory: deDirectory,
    validation: deValidation,
    errors: deErrors,
  };
  const english = {
    common: enCommon,
    auth: enAuth,
    admin: enAdmin,
    directory: enDirectory,
    validation: enValidation,
    errors: enErrors,
  };
  const germanKeys = flattenKeys(german).sort();
  const englishKeys = flattenKeys(english).sort();
  expect(englishKeys).toEqual(germanKeys);
  expect(new Set(germanKeys).size).toBe(germanKeys.length);
  for (const key of germanKeys) {
    expect(placeholders(resolveMessage("de", key))).toEqual(placeholders(resolveMessage("en", key)));
  }
  expect(translate("de", "common.emptyRecords")).toBe("Keine Datensätze vorhanden.");
  expect(translate("en", "common.search")).toBe("Search");
  expect(translate("de", "missing.key")).toBe("missing.key");
  expect(translate("de", "admin.dashboardHint", { email: "ada@example.com" })).toContain("ada@example.com");
  expect(translateError("de", { status: 422, code: "validation.required", detail: "Field is required", field: "name" }, "Name")).toBe("Feld ist erforderlich");
  expect(translateError("de", { status: 500, detail: "SELECT password_hash FROM users" })).toBe("Die Aktion ist fehlgeschlagen.");
  expect(formatNumber("de", 1234)).toBe("1.234");
  expect(formatCurrency("de", 123456, "EUR")).toContain("1.234,56");
  expect(formatCurrency("en", 123456, "EUR")).toContain("1,234.56");
  expect(formatDate("de", new Date(2026, 10, 7))).toBe("07.11.2026");
  expect(formatDate("en", new Date(2026, 10, 7))).toBe("11/07/2026");
});

it("zeigt Suche, Filter und die Trefferzahl", async () => {
  render(
    <MemoryRouter>
      <DataTable model="products" columns={[name]} rows={[{ id: "abc", name: "Alpha" }]} page={1} pageSize={25} total={40} hasMore sort="name" onSort={() => undefined} onPage={() => undefined} />
    </MemoryRouter>,
  );
  expect(screen.getByText(/von 40/)).toBeInTheDocument();
  vi.mocked(fetch).mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url.endsWith("/schema")) {
      return json({
        model: "products",
        label: "Produkte",
        list_blocked: false,
        actions: ["read"],
        fields: [{ ...name, filterable: true }],
      });
    }
    return json({ data: [], has_more: false, total: 0 });
  });
  render(
    <MemoryRouter initialEntries={["/models/products"]}>
      <Routes>
        <Route path="/models/:model" element={<ModelList />} />
      </Routes>
    </MemoryRouter>,
  );
  expect(await screen.findByLabelText("Suche")).toBeInTheDocument();
  expect(await screen.findByLabelText("Filter Name")).toBeInTheDocument();
  expect(await screen.findByText("Keine Datensätze vorhanden.")).toBeInTheDocument();
});

it("filtert das Benutzerverzeichnis und zeigt den leeren Zustand", async () => {
  vi.mocked(fetch).mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url.endsWith("/users")) return json({ users: [{ id: "1", email: "ada@example.com", is_active: true, is_staff: true, is_superuser: false }] });
    if (url.endsWith("/roles")) return json({ roles: [] });
    return json({ permissions: [] });
  });
  render(<Directory />);
  expect(await screen.findByText("ada@example.com")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Benutzer suchen"), { target: { value: "fehlt" } });
  expect(screen.getByText("Keine Benutzer vorhanden.")).toBeInTheDocument();
  expect(screen.getByText("Keine Rollen vorhanden.")).toBeInTheDocument();
});

it("wechselt die Sprache ohne Neuladen und übernimmt den Admin-Titel", async () => {
  localStorage.clear();
  vi.mocked(fetch).mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url === "/admin/api/config") {
      return json({
        site_title: "Ladenverwaltung",
        site_header: "Laden",
        app_name: "Laden",
        primary_color: "red;color:url(x)",
        default_locale: "de",
        fallback_locale: "en",
        currency: "EUR",
      });
    }
    if (url === "/auth/me") return json({ detail: "unauthenticated" }, 401);
    return json({ models: [] });
  });
  render(
    <I18nProvider>
      <MemoryRouter>
        <AuthProvider>
          <Layout><p>Inhalt</p></Layout>
        </AuthProvider>
      </MemoryRouter>
    </I18nProvider>,
  );
  expect(await screen.findByRole("link", { name: "Übersicht" })).toBeInTheDocument();
  await waitFor(() => expect(document.title).toBe("Ladenverwaltung"));
  expect(document.documentElement.style.getPropertyValue("--brand")).toBe("#0f172a");
  fireEvent.click(screen.getByRole("button", { name: "Sprache" }));
  expect(await screen.findByRole("link", { name: "Overview" })).toBeInTheDocument();
  expect(localStorage.getItem("ferrum-locale")).toBe("en");
  expect(document.documentElement.lang).toBe("en");
});
