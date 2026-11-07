import { FormEvent, useEffect, useMemo, useState } from "react";
import { request } from "../api/client";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { translateError } from "../i18n/errors";
import { useI18n } from "../i18n";
import type { ApiError } from "../types";

type UserRow = {
  id: string;
  email: string;
  is_active: boolean;
  is_staff: boolean;
  is_superuser: boolean;
  roles?: string[];
};

type Pending = { title: string; body: string; action: () => Promise<void> } | null;

export function Directory() {
  const { locale, t } = useI18n();
  const [users, setUsers] = useState<UserRow[]>([]);
  const [roles, setRoles] = useState<Array<{ name: string; permissions: string[] }>>([]);
  const [message, setMessage] = useState("");
  const [notice, setNotice] = useState("");
  const [loading, setLoading] = useState(true);
  const [userQuery, setUserQuery] = useState("");
  const [roleQuery, setRoleQuery] = useState("");
  const [permissionQuery, setPermissionQuery] = useState("");
  const [roleName, setRoleName] = useState("");
  const [grantRole, setGrantRole] = useState("");
  const [grantCode, setGrantCode] = useState("");
  const [assignmentEmail, setAssignmentEmail] = useState("");
  const [assignmentRole, setAssignmentRole] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [sortDesc, setSortDesc] = useState(false);
  const [page, setPage] = useState(1);
  const [pending, setPending] = useState<Pending>(null);
  const pageSize = 8;

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

  useEffect(() => {
    void reload()
      .catch((caught) => setMessage(isApiError(caught) ? translateError(locale, caught) : t("directory.unavailable")))
      .finally(() => setLoading(false));
  }, [locale, t]);

  async function run(action: () => Promise<void>, success: string): Promise<boolean> {
    try {
      setMessage("");
      setNotice("");
      await action();
      await reload();
      setNotice(success);
      return true;
    } catch (caught) {
      setMessage(isApiError(caught) ? translateError(locale, caught) : t("directory.rejected"));
      return false;
    }
  }

  const visibleUsers = useMemo(() => {
    const filtered = users.filter((user) => user.email.toLowerCase().includes(userQuery.toLowerCase()));
    const sorted = [...filtered].sort((left, right) => left.email.localeCompare(right.email));
    return sortDesc ? sorted.reverse() : sorted;
  }, [users, userQuery, sortDesc]);
  const pages = Math.max(1, Math.ceil(visibleUsers.length / pageSize));
  const currentPage = Math.min(page, pages);
  const pageUsers = visibleUsers.slice((currentPage - 1) * pageSize, currentPage * pageSize);
  const selectedUser = users.find((user) => user.id === selected) ?? null;
  const visibleRoles = roles.filter((role) => role.name.toLowerCase().includes(roleQuery.toLowerCase()));

  async function createRole(event: FormEvent) {
    event.preventDefault();
    const created = await run(async () => {
      await request("POST", "/admin/api/directory/roles", { name: roleName });
      setRoleName("");
    }, t("directory.roleCreated"));
    if (!created) setNotice("");
  }

  return (
    <section className="space-y-8">
      <h2 className="text-3xl font-semibold">{t("directory.title")}</h2>
      {loading ? <p role="status">{t("common.loading")}</p> : null}
      {message ? <p role="alert" className="status-note">{message}</p> : null}
      {notice ? <p role="status" className="status-note">{notice}</p> : null}
      <p className="text-sm text-slate-600">{t("directory.superuserHint")}</p>
      <input aria-label={t("directory.searchUsers")} className="search-input max-w-md" value={userQuery} onChange={(event) => { setPage(1); setUserQuery(event.target.value); }} placeholder={t("directory.searchUsers")} />
      <div className="data-table">
        <table className="w-full text-sm">
          <thead>
            <tr>
              <th><button type="button" onClick={() => setSortDesc((value) => !value)}>{t("directory.email")}{sortDesc ? " ↓" : " ↑"}</button></th>
              <th>{t("directory.active")}</th>
              <th>{t("directory.staff")}</th>
              <th>{t("directory.roles")}</th>
              <th>{t("directory.hint")}</th>
            </tr>
          </thead>
          <tbody>
            {pageUsers.map((user) => (
              <tr key={user.id}>
                <td><button type="button" className="underline" onClick={() => setSelected(user.id)}>{user.email}</button></td>
                <td>
                  {user.is_active ? t("directory.statusActive") : t("directory.statusInactive")}
                  {user.is_superuser ? null : (
                    <button type="button" className="ml-2 underline" onClick={() => setPending({
                      title: user.is_active ? t("directory.confirmDeactivate") : t("directory.activate"),
                      body: user.email,
                      action: () => request("POST", `/admin/api/directory/users/${user.id}/active`, { active: !user.is_active }).then(() => undefined),
                    })}>
                      {user.is_active ? t("directory.deactivate") : t("directory.activate")}
                    </button>
                  )}
                </td>
                <td>
                  {user.is_staff ? t("common.yes") : t("common.no")}
                  {user.is_superuser ? null : (
                    <button type="button" className="ml-2 underline" onClick={() => setPending({
                      title: user.is_staff ? t("directory.confirmStaff") : t("directory.staffGrant"),
                      body: user.email,
                      action: () => request("POST", `/admin/api/directory/users/${user.id}/staff`, { staff: !user.is_staff }).then(() => undefined),
                    })}>
                      {user.is_staff ? t("directory.staffRevoke") : t("directory.staffGrant")}
                    </button>
                  )}
                </td>
                <td>{user.roles && user.roles.length > 0 ? user.roles.join(", ") : t("directory.noRoles")}</td>
                <td>{user.is_superuser ? t("directory.superuserNote") : t("directory.normal")}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="flex items-center gap-3">
        <button type="button" className="btn btn-secondary" disabled={currentPage <= 1} onClick={() => setPage(currentPage - 1)}>{t("common.back")}</button>
        <span>{currentPage} {t("common.pageOf")} {pages}</span>
        <button type="button" className="btn btn-secondary" disabled={currentPage >= pages} onClick={() => setPage(currentPage + 1)}>{t("common.next")}</button>
      </div>
      {visibleUsers.length === 0 ? <p className="empty-state">{t("directory.emptyUsers")}</p> : null}
      {selectedUser ? (
        <section className="rounded-2xl bg-white p-5 dark:bg-slate-900" aria-label={t("directory.userDetail")}>
          <h3 className="text-xl font-semibold">{t("directory.userDetail")}</h3>
          <p>{selectedUser.email}</p>
          <p>{selectedUser.is_active ? t("directory.statusActive") : t("directory.statusInactive")}</p>
          <p>{t("directory.roles")}: {selectedUser.roles?.join(", ") || t("directory.noRoles")}</p>
          <div className="mt-3 flex flex-wrap gap-2">
            {(selectedUser.roles ?? []).map((role) => (
              <button key={role} type="button" className="btn btn-secondary" onClick={() => setPending({
                title: t("directory.confirmUnassign"),
                body: `${selectedUser.email} · ${role}`,
                action: () => request("DELETE", "/admin/api/directory/assignments", { email: selectedUser.email, role }).then(() => undefined),
              })}>
                {t("directory.revokeRole", { role })}
              </button>
            ))}
          </div>
        </section>
      ) : null}
      <input aria-label={t("directory.filterRoles")} className="search-input max-w-md" value={roleQuery} onChange={(event) => setRoleQuery(event.target.value)} placeholder={t("directory.filterRoles")} />
      <input aria-label={t("directory.searchPermissions")} className="search-input max-w-md" value={permissionQuery} onChange={(event) => setPermissionQuery(event.target.value)} placeholder={t("directory.searchPermissions")} />
      <form onSubmit={createRole} className="flex flex-wrap gap-2">
        <input aria-label={t("directory.roleName")} className="search-input max-w-xs" value={roleName} onChange={(event) => setRoleName(event.target.value)} />
        <button className="btn btn-primary" type="submit">{t("directory.createRole")}</button>
      </form>
      <ul className="space-y-2">
        {visibleRoles.map((role) => {
          const codes = role.permissions.filter((code) => code.toLowerCase().includes(permissionQuery.toLowerCase()));
          if (permissionQuery && codes.length === 0) return null;
          return (
            <li key={role.name}>
              {role.name}: {codes.join(", ") || t("directory.noPermissions")}
              {codes.map((code) => (
                <button key={code} type="button" className="ml-2 underline" onClick={() => setPending({
                  title: t("directory.confirmRevoke"),
                  body: code,
                  action: () => request("DELETE", `/admin/api/directory/roles/${role.name}/permissions/${code}`).then(() => undefined),
                })}>
                  {t("directory.revoke", { code })}
                </button>
              ))}
            </li>
          );
        })}
      </ul>
      {visibleRoles.length === 0 ? <p className="empty-state">{t("directory.emptyRoles")}</p> : null}
      <form className="flex flex-wrap gap-2" onSubmit={(event) => { event.preventDefault(); void run(async () => { await request("POST", `/admin/api/directory/roles/${grantRole}/permissions/${grantCode}`); setGrantCode(""); }, t("directory.permissionGranted")); }}>
        <input aria-label={t("directory.roleForPermission")} className="search-input max-w-xs" value={grantRole} onChange={(event) => setGrantRole(event.target.value)} placeholder={t("directory.roleName")} />
        <input aria-label={t("directory.permissionCode")} className="search-input max-w-xs" value={grantCode} onChange={(event) => setGrantCode(event.target.value)} placeholder="products.read" />
        <button className="btn btn-secondary" type="submit">{t("directory.grantPermission")}</button>
      </form>
      <form className="flex flex-wrap gap-2" onSubmit={(event) => { event.preventDefault(); void run(() => request("POST", "/admin/api/directory/assignments", { email: assignmentEmail, role: assignmentRole }).then(() => undefined), t("directory.assigned")); }}>
        <input aria-label={t("directory.assignmentEmail")} className="search-input max-w-xs" value={assignmentEmail} onChange={(event) => setAssignmentEmail(event.target.value)} placeholder={t("auth.email")} />
        <input aria-label={t("directory.assignmentRole")} className="search-input max-w-xs" value={assignmentRole} onChange={(event) => setAssignmentRole(event.target.value)} placeholder={t("directory.roleName")} />
        <button className="btn btn-secondary" type="submit">{t("directory.assignRole")}</button>
      </form>
      {pending ? (
        <ConfirmDialog
          title={pending.title}
          body={pending.body}
          confirmLabel={t("common.confirm")}
          onCancel={() => setPending(null)}
          onConfirm={() => {
            const action = pending.action;
            setPending(null);
            void run(action, t("directory.changed"));
          }}
        />
      ) : null}
    </section>
  );
}

function isApiError(value: unknown): value is ApiError {
  return Boolean(value && typeof value === "object" && "detail" in value && "status" in value);
}
