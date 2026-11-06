import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, expect, it, vi } from "vitest";
import { fieldError, login, request, restoreSession } from "./api/client";
import { AuthProvider } from "./auth/AuthProvider";
import { LoginPage } from "./auth/LoginPage";
import { DataTable } from "./components/DataTable";
import { DynamicForm } from "./components/DynamicForm";
import { FieldRenderer } from "./components/FieldRenderer";
import { Sidebar } from "./components/Sidebar";
import { ModelCreate } from "./pages/ModelCreate";
import { ModelDetail } from "./pages/ModelDetail";
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
