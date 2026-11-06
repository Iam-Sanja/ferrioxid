import { FormEvent, useEffect, useState } from "react";
import { request } from "../api/client";

type UserRow = { id: string; email: string; is_active: boolean; is_staff: boolean; is_superuser: boolean };

export function Directory() {
  const [users, setUsers] = useState<UserRow[]>([]);
  const [roles, setRoles] = useState<Array<{ name: string; permissions: string[] }>>([]);
  const [message, setMessage] = useState("");
  const [roleName, setRoleName] = useState("");
  const [grantRole, setGrantRole] = useState("");
  const [grantCode, setGrantCode] = useState("");
  const [assignmentEmail, setAssignmentEmail] = useState("");
  const [assignmentRole, setAssignmentRole] = useState("");

  async function reload() {
    const userPayload = (await request("GET", "/admin/api/directory/users")) as { users: UserRow[] };
    const rolePayload = (await request("GET", "/admin/api/directory/roles")) as { roles: Array<{ name: string }> };
    const detailed = await Promise.all(rolePayload.roles.map(async (role) => {
      const permissions = (await request("GET", `/admin/api/directory/roles/${role.name}/permissions`)) as { permissions: string[] };
      return { name: role.name, permissions: permissions.permissions };
    }));
    setUsers(userPayload.users);
    setRoles(detailed);
  }

  useEffect(() => { void reload().catch((caught) => setMessage(String((caught as { detail?: string }).detail ?? "Verzeichnis nicht verfügbar"))); }, []);

  async function run(action: () => Promise<void>) {
    try {
      setMessage("");
      await action();
      await reload();
    } catch (caught) {
      setMessage(caught && typeof caught === "object" && "detail" in caught ? String(caught.detail) : "Aktion abgelehnt");
    }
  }

  async function createRole(event: FormEvent) {
    event.preventDefault();
    await run(async () => {
      await request("POST", "/admin/api/directory/roles", { name: roleName });
      setRoleName("");
    });
  }

  return (
    <section className="space-y-8">
      <h2 className="text-3xl font-semibold">Benutzer und Rollen</h2>
      {message ? <p role="alert">{message}</p> : null}
      <p className="text-sm text-slate-600">Superuser-Rechte können hier nicht vergeben werden. Passwort-Hashes werden nicht übertragen.</p>
      <table className="w-full bg-white text-sm dark:bg-slate-900">
        <thead><tr><th>E-Mail</th><th>Aktiv</th><th>Staff</th><th>Hinweis</th></tr></thead>
        <tbody>
          {users.map((user) => (
            <tr key={user.id}>
              <td>{user.email}</td>
              <td>{user.is_active ? "ja" : "nein"}{user.is_superuser ? null : <button type="button" className="ml-2 underline" onClick={() => void run(() => request("POST", `/admin/api/directory/users/${user.id}/active`, { active: !user.is_active }).then(() => undefined))}>{user.is_active ? "Deaktivieren" : "Aktivieren"}</button>}</td>
              <td>{user.is_staff ? "ja" : "nein"}{user.is_superuser ? null : <button type="button" className="ml-2 underline" onClick={() => void run(() => request("POST", `/admin/api/directory/users/${user.id}/staff`, { staff: !user.is_staff }).then(() => undefined))}>{user.is_staff ? "Staff entziehen" : "Staff geben"}</button>}</td>
              <td>{user.is_superuser ? "Superuser, nur über die Kommandozeile änderbar" : "normal"}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <form onSubmit={createRole} className="flex gap-2">
        <input aria-label="Rollenname" className="rounded-lg border px-3 py-2" value={roleName} onChange={(event) => setRoleName(event.target.value)} />
        <button className="rounded-lg bg-teal-700 px-3 py-2 text-white" type="submit">Rolle anlegen</button>
      </form>
      <ul>
        {roles.map((role) => (
          <li key={role.name}>
            {role.name}: {role.permissions.join(", ") || "keine Berechtigungen"}
            {role.permissions.map((code) => (
              <button key={code} type="button" className="ml-2 underline" onClick={() => void run(() => request("DELETE", `/admin/api/directory/roles/${role.name}/permissions/${code}`).then(() => undefined))}>
                {code} entziehen
              </button>
            ))}
          </li>
        ))}
      </ul>
      <form className="flex flex-wrap gap-2" onSubmit={(event) => { event.preventDefault(); void run(async () => { await request("POST", `/admin/api/directory/roles/${grantRole}/permissions/${grantCode}`); setGrantCode(""); }); }}>
        <input aria-label="Rolle für Berechtigung" className="rounded-lg border px-3 py-2" value={grantRole} onChange={(event) => setGrantRole(event.target.value)} placeholder="Rolle" />
        <input aria-label="Berechtigungscode" className="rounded-lg border px-3 py-2" value={grantCode} onChange={(event) => setGrantCode(event.target.value)} placeholder="products.read" />
        <button className="rounded-lg border px-3 py-2" type="submit">Berechtigung zuweisen</button>
      </form>
      <form className="flex flex-wrap gap-2" onSubmit={(event) => { event.preventDefault(); void run(() => request("POST", "/admin/api/directory/assignments", { email: assignmentEmail, role: assignmentRole }).then(() => undefined)); }}>
        <input aria-label="E-Mail für Rollenzuordnung" className="rounded-lg border px-3 py-2" value={assignmentEmail} onChange={(event) => setAssignmentEmail(event.target.value)} placeholder="E-Mail" />
        <input aria-label="Rolle zuordnen" className="rounded-lg border px-3 py-2" value={assignmentRole} onChange={(event) => setAssignmentRole(event.target.value)} placeholder="Rolle" />
        <button className="rounded-lg border px-3 py-2" type="submit">Rolle zuordnen</button>
      </form>
    </section>
  );
}
