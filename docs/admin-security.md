# Admin-Sicherheit

[Zur Übersicht](admin.md) · Auth: [auth.md](auth.md) · Policies: [policies.md](policies.md)

## Zweck

Der Admin ist eine zweite Tür vor denselben Benutzern, Sitzungen und Rechten. Er lockert keine öffentliche Route und übernimmt keine anonyme Freigabe. `GET /admin/api/config` ist die einzige Admin-Route ohne Staff-Prüfung und enthält keine Secrets. Farben und Texte werden vor der Ausgabe geprüft. Das Verzeichnis bleibt Staff und Superuser. Passwort-Hashes verlassen den Server nicht. Fehlercodes ersetzen keine Statuscodes und machen falsche E-Mail und falsches Passwort nicht unterscheidbar.

## Prüfung je Request

1. Sitzung über `AuthService::authenticate`.
2. Aktiver Benutzer.
3. `is_staff`.
4. Modellrecht `ressource.read|create|update|delete`.
5. Gebundene Objektregel für einen einzelnen Datensatz.
6. CSRF bei jeder Schreibmethode.

Ein aktiver Superuser überspringt Modellrechte, wenn die Ressource gesetzt ist. Ist `admin_permission` und `permissions` leer, lässt `permit` nur Superuser zu. Das Staff-Flag bleibt in beiden Fällen Pflicht.

## CSRF-Bootstrap

`GET /auth/csrf` funktioniert nur mit gültiger Sitzung. Origin oder Referer müssen erlaubt sein, wenn sie gesendet werden. Ein fehlender Origin ist erlaubt, weil keine offene CORS-Policy existiert. Der Aufruf berechnet dasselbe HMAC-Token erneut. Sitzungstoken und Ablauf bleiben. Gleichzeitige Tabs bleiben gültig. Details stehen in [csrf.md](csrf.md).

Die HTML-Anmeldung prüft zusätzlich `ferrum_login_csrf` gegen das Formularfeld. Die erlaubten Origins folgen der Bind-Adresse und nicht einem fest verdrahteten Port 3000. Der Host-Header wird dafür nicht gelesen. Logs enthalten den Grundcode, nicht das Cookie und nicht das Token.

Das Token steht nicht in `localStorage` und nicht in der URL. Das Sitzungscookie bleibt `HttpOnly`.

## Listen und Fremdschlüssel

Sortier-, Such- und Filterfelder stammen aus `AdminModelMeta`. Bezeichner müssen zusätzlich `^[A-Za-z_][A-Za-z0-9_]*$` und einer echten Spalte entsprechen. Werte sind gebundene Parameter.

Eine gebundene Objektregel filtert Liste, Detail, PATCH, DELETE und die Fremdschlüssel-Auswahl über `QueryScope`. `total` verwendet denselben Filter. Ein einzelner abgelehnter Datensatz ist 404. Eine ungebundene Regel bleibt 403 mit `object policy is not bound`. Die Auswahl lädt höchstens 25 Zeilen und nur innerhalb des Scopes. Siehe [query-scopes.md](query-scopes.md).

## Privilegieneskalation

Normale Modellrechte öffnen das Verzeichnis nicht. Superuser-Vergabe ist blockiert. Ein Benutzer kann die eigenen Schutzflags nicht entfernen. Sensible Namen (`password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash`) müssen im Admin verborgen sein und werden aus Antworten entfernt.

## Tests

Der JSON-Test prüft anonyme Admin-Anfrage, fehlendes Staff, unbekanntes Modell, falsche Sortierung, falschen Filter, schreibgeschütztes `id`, Objekt-Scope, fehlendes CSRF, dasselbe Token in zwei Abrufen, ein sitzungsfremdes Token und die Superuser-Sperre. `admin_html.rs` prüft dieselben Regeln über Formulare, dazu offene Weiterleitungen und escaptes HTML. Assets prüft `admin_assets_reject_traversal_and_keep_the_api`.

## Grenzen

Das Login-Limit ist prozesslokal. Anonyme Demo-Schreibrouten außerhalb des Admins bleiben ohne CSRF, wie in v0.7 dokumentiert. Die Filterdekodierung ist nicht vollständig UTF-8.

## Später

Ein vom Superuser getrennter Verzeichnis-Administrator. OAuth, MFA und einzeln widerrufbare CSRF-Tokens je Tab sind geplant und nicht implementiert.
