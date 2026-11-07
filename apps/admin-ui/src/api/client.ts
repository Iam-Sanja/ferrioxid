import type { ApiError, ModelLink, PublicUser, Schema } from "../types";

const csrf = { token: "" };

export function csrfToken(): string {
  return csrf.token;
}

export function rememberCsrf(token: string): void {
  csrf.token = token;
}

async function parse(response: Response): Promise<unknown> {
  if (response.status === 204) {
    return null;
  }
  const text = await response.text();
  if (!text) {
    return null;
  }
  return JSON.parse(text) as unknown;
}

export async function request(method: string, path: string, body?: unknown): Promise<unknown> {
  const headers: Record<string, string> = { Accept: "application/json" };
  if (body !== undefined) {
    headers["Content-Type"] = "application/json";
  }
  if (method !== "GET" && method !== "HEAD") {
    headers["x-csrf-token"] = csrf.token;
  }
  const response = await fetch(path, {
    method,
    credentials: "include",
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const payload = await parse(response);
  if (!response.ok) {
    const record = payload && typeof payload === "object" ? (payload as Record<string, unknown>) : {};
    const detail = typeof record.detail === "string" ? record.detail : response.statusText;
    const error: ApiError = {
      status: response.status,
      detail,
      code: typeof record.code === "string" ? record.code : undefined,
      field: typeof record.field === "string" ? record.field : undefined,
    };
    throw error;
  }
  return payload;
}

export async function login(email: string, password: string): Promise<PublicUser> {
  const payload = (await request("POST", "/auth/login", { email, password })) as {
    user: PublicUser;
    csrf_token: string;
  };
  rememberCsrf(payload.csrf_token);
  return payload.user;
}

export async function logout(): Promise<void> {
  await request("POST", "/auth/logout");
  rememberCsrf("");
}

export async function restoreSession(): Promise<PublicUser | null> {
  const me = await fetch("/auth/me", { credentials: "include" });
  if (!me.ok) {
    return null;
  }
  const user = (await me.json()) as PublicUser;
  const token = await fetch("/auth/csrf", { credentials: "include" });
  if (!token.ok) {
    return null;
  }
  const body = (await token.json()) as { csrf_token: string };
  rememberCsrf(body.csrf_token);
  return user;
}

export async function adminMe(): Promise<PublicUser> {
  return (await request("GET", "/admin/api/me")) as PublicUser;
}

export async function modelLinks(): Promise<ModelLink[]> {
  const payload = (await request("GET", "/admin/api/models")) as { models: ModelLink[] };
  return payload.models;
}

export async function modelSchema(key: string): Promise<Schema> {
  return (await request("GET", `/admin/api/models/${key}/schema`)) as Schema;
}

export function fieldError(detail: string, name: string): string | null {
  const prefix = `field:${name}:`;
  return detail.startsWith(prefix) ? detail.slice(prefix.length).trim() : null;
}
