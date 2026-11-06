# Admin-Sicherheit

[Zur Übersicht](admin.md) · Auth: [auth.md](auth.md) · Policies: [policies.md](policies.md)

## Zweck

Der Admin ist eine zweite Tür vor denselben Benutzern, Sitzungen und Rechten. Er lockert keine öffentliche Route und übernimmt keine anonyme Freigabe.

## Prüfung je Request

1. Sitzung über `AuthService::authenticate`.
2. Aktiver Benutzer.
3. `is_staff`.
4. Modellrecht `ressource.read|create|update|delete`.
5. Gebundene Objektregel für einen einzelnen Datensatz.
6. CSRF bei jeder Schreibmethode.

Ein aktiver Superuser überspringt Modellrechte, wenn die Ressource gesetzt ist. Ist `admin_permission` und `permissions` leer, lässt `permit` nur Superuser zu. Das Staff-Flag bleibt in beiden Fällen Pflicht.

## CSRF-Bootstrap

`GET /auth/csrf` funktioniert nur mit gültiger Sitzung. Origin oder Referer müssen erlaubt sein, wenn sie gesendet werden. Ein fehlender Origin ist erlaubt, weil keine offene CORS-Policy existiert. Der Aufruf rotiert nur den CSRF-Hash. Sitzungstoken und Ablauf bleiben. Der vorherige Token wird ungültig. Gleichzeitige Tabs können sich damit gegenseitig das Token entziehen.

Das Token steht nicht in `localStorage` und nicht in der URL. Das Sitzungscookie bleibt `HttpOnly`.

## Listen und Fremdschlüssel

Sortier-, Such- und Filterfelder stammen aus `AdminModelMeta`. Bezeichner müssen zusätzlich `^[A-Za-z_][A-Za-z0-9_]*$` und einer echten Spalte entsprechen. Werte sind gebundene Parameter.

Eine Objektregel hat keinen SQL-Filter. Die Liste antwortet deshalb 403 mit dem Text `object-level policies block list endpoints until a query scope exists`. Ein einzelner abgelehnter Datensatz ist 404. Fremdschlüssel-Auswahlen auf ein solches Ziel sind ebenfalls 403. Es werden nie alle Zeilen einer ungeschützten Liste ohne Limit geladen: das Maximum ist 25.

## Privilegieneskalation

Normale Modellrechte öffnen das Verzeichnis nicht. Superuser-Vergabe ist blockiert. Ein Benutzer kann die eigenen Schutzflags nicht entfernen. Sensible Namen (`password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash`) müssen im Admin verborgen sein und werden aus Antworten entfernt.

## Tests

Der SQLite-Test prüft anonyme Admin-Anfrage, fehlendes Staff, unbekanntes Modell, falsche Sortierung, falschen Filter, schreibgeschütztes `id`, Objektregel, fehlendes CSRF, altes CSRF nach Bootstrap und die Superuser-Sperre. Abgelaufene Sitzungen prüft der bestehende Auth-Test `expired_session_cookie_is_rejected`; der Admin nutzt dieselbe `authenticate`-Funktion.

## Grenzen

Das Login-Limit ist prozesslokal. Anonyme Demo-Schreibrouten außerhalb des Admins bleiben ohne CSRF, wie in v0.7 dokumentiert. Die Filterdekodierung ist nicht vollständig UTF-8.

## Später

Query-Scopes, mehrere gleichzeitige CSRF-Token und ein vom Superuser getrennter Verzeichnis-Administrator.
