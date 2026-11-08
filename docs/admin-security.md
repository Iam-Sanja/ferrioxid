# Admin-Sicherheit

[Zur Übersicht](admin.md) · Auth: [auth.md](auth.md) · Policies: [policies.md](policies.md)

## Zweck

Der Admin ist eine zweite Tür vor denselben Benutzern, Sitzungen und Rechten. Er lockert keine öffentliche Route und übernimmt keine anonyme Freigabe. `GET /admin/api/config` ist die einzige Admin-Route ohne Staff-Prüfung und enthält keine Secrets. Farben und Texte werden vor der Ausgabe geprüft. Das Verzeichnis bleibt Staff und Superuser. Passwort-Hashes verlassen den Server nicht. Fehlercodes ersetzen keine Statuscodes und machen falsche E-Mail und falsches Passwort nicht unterscheidbar.

Eine Kind-ID aus dem POST gilt nur, wenn der Datensatz in der Transaktion zum Elternobjekt gehört. Ein fremder oder manipulierter Fremdschlüssel wird ersetzt oder abgelehnt. Rechte werden in der Transaktion erneut gelesen. Siehe [admin-transactional-inlines.md](admin-transactional-inlines.md).

## Prüfung je Request

1. Sitzung über `AuthService::authenticate`.
2. Aktiver Benutzer.
3. `is_staff`.
4. Modellrecht `ressource.read|create|update|delete`.
5. Gebundene Objektregel für einen einzelnen Datensatz.
6. CSRF bei jeder Schreibmethode.
7. Bei `/admin/pages/{key}` zusätzlich der registrierte Code über `AuthService::holds`, bevor der Handler läuft.
8. Bei einer eigenen Sammelaktion das Modellrecht `update`, die Objektregel und die deklarierte Spaltenliste, bevor ein Wert geschrieben wird.

Ein aktiver Superuser überspringt Modellrechte, wenn die Ressource gesetzt ist. Ist `admin_permission` und `permissions` leer, lässt `permit` nur Superuser zu. Das Staff-Flag bleibt in beiden Fällen Pflicht.

## CSRF-Bootstrap

`GET /auth/csrf` funktioniert nur mit gültiger Sitzung. Origin oder Referer müssen erlaubt sein, wenn sie gesendet werden. Ein fehlender Origin ist erlaubt, weil keine offene CORS-Policy existiert. Der Aufruf berechnet dasselbe HMAC-Token erneut. Sitzungstoken und Ablauf bleiben. Gleichzeitige Tabs bleiben gültig. Details stehen in [csrf.md](csrf.md).

Die HTML-Anmeldung prüft zusätzlich `ferrioxid_login_csrf` gegen das Formularfeld. Die erlaubten Origins folgen der Bind-Adresse und nicht einem fest verdrahteten Port 3000. Der Host-Header wird dafür nicht gelesen. Logs enthalten den Grundcode, nicht das Cookie und nicht das Token.

Das Token steht nicht in `localStorage` und nicht in der URL. Das Sitzungscookie bleibt `HttpOnly`.

## Listen und Fremdschlüssel

Sortier-, Such- und Filterfelder stammen aus `AdminModelMeta`. Bezeichner müssen zusätzlich `^[A-Za-z_][A-Za-z0-9_]*$` und einer echten Spalte entsprechen. Werte sind gebundene Parameter.

Eine gebundene Objektregel filtert Liste, Detail, PATCH, DELETE, das Formular-Select und die Auswahlseite über `QueryScope`. `total` verwendet denselben Filter. Ein einzelner abgelehnter Datensatz ist 404. Eine ungebundene Regel bleibt 403 mit `object policy is not bound`. Das Select lädt höchstens 100 Zeilen. Die Auswahlseite nutzt `admin_per_page` des Ziels, gedeckelt bei 100. Eine UUID, die der Benutzer am registrierten Ziel nicht lesen darf, wird vor dem Schreiben abgelehnt. `save` darf nur `detail`, `edit` oder `new` sein. Die Zieladresse baut der Server. Siehe [query-scopes.md](query-scopes.md) und [admin-relational-widgets.md](admin-relational-widgets.md).

## Privilegieneskalation

Normale Modellrechte öffnen das Verzeichnis nicht. Superuser-Vergabe ist blockiert. Ein Benutzer kann die eigenen Schutzflags nicht entfernen. Sensible Namen (`password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash`) müssen im Admin verborgen sein und werden aus Antworten entfernt.

## Abmelden

`POST /admin/logout` prüft die Sitzung und das CSRF-Feld. `is_staff` ist dafür keine Bedingung. Die Sitzung wird widerrufen und das Session-Cookie geleert. Die Antwort ist 303 auf `/admin/login`. Ein fehlendes oder fremdes Token lässt die Sitzung bestehen.

`require_get` unterscheidet:

- keine Sitzung: Weiterleitung zur Anmeldung
- Sitzung ohne Staff: HTML 403 mit dem Text „Die Verwaltung verlangt einen Staff-Zugang.“ und Abmeldeformular, ohne Seitenleiste und ohne Modelldaten
- Staff: die angeforderte Seite
- Staff ohne Modellrecht oder ohne Superuser für das Verzeichnis: HTML 403 „Diese Aktion ist nicht erlaubt.“ mit Abmeldung

Die JSON-API unter `/admin/api` antwortet weiter mit Klartext und ist kein Logout-Formular. `html_admin_works_without_javascript` prüft die HTML-Fälle.

Fehlerseiten mit Sitzung lösen `is_superuser` über dieselbe Authentifizierung wie die übrige Fläche. Die Navigation leitet daraus keine Rechte ab. Ein Staff-Superuser sieht den Verzeichnislink auch auf einer 403-Seite. Ein Staff ohne Superuser sieht ihn nicht. Ohne Staff bleibt die Seitenleiste weg, das Abmeldeformular bleibt.

HTML-Verzeichnisaktionen, die das Häkchen zeigen, rufen `require_confirmation` auf. Gültig ist nur `confirm=1`. Die JSON-Verzeichnis-API hat dieses Feld nicht. Sie bleibt an Sitzung, Staff-Superuser und CSRF gebunden.

## Tests

Der JSON-Test prüft anonyme Admin-Anfrage, fehlendes Staff, unbekanntes Modell, falsche Sortierung, falschen Filter, schreibgeschütztes `id`, Objekt-Scope, fehlendes CSRF, dasselbe Token in zwei Abrufen, ein sitzungsfremdes Token und die Superuser-Sperre. `admin_html.rs` prüft dieselben Regeln über Formulare, dazu offene Weiterleitungen und escaptes HTML. Assets prüft `admin_assets_reject_traversal_and_keep_the_api`.

## Grenzen

Das Login-Limit ist prozesslokal. Anonyme Demo-Schreibrouten außerhalb des Admins bleiben ohne CSRF, wie in v0.7 dokumentiert. Die Filterdekodierung ist nicht vollständig UTF-8.

## Entwürfe, Sammelaktionen und Historie

Ein Formularentwurf gehört zu `session_id` und `user_id`. Die ID ist eine UUID. Der Payload enthält nur erlaubte Modellfelder, nie Passwort, Hash, Token oder CSRF. Ein fremder Benutzer und eine abgelaufene ID lesen nichts. Der endgültige POST validiert und prüft die Policy erneut.

Eine Sammelaktion akzeptiert nur einen registrierten Namen. Die erste POST-Anfrage speichert die IDs und schreibt keine Modellzeile. `confirm=1` fehlt: keine Änderung. Vor dem Schreiben liest die Transaktion jede ID mit Schreib-Scope und Objektregel. Eine fehlende, fremde oder doppelt verweigerte ID bricht die ganze Auswahl ab. `delete` und `SetBool` laufen in derselben Transaktion wie ihre Audit-Zeilen. Ein Fremdschlüsselfehler rollt alles zurück. Die Auswahl ist auf `max_ids` gedeckelt, eingebaute Aktionen auf 100. `scope=page` ignoriert gesendete IDs und liest die aktuelle Seite erneut.

Die Historie speichert keine sensiblen Werte. Ein Staff ohne Superuser sieht nur Ereignisse mit der eigenen `actor_id`. Verzeichnisereignisse entstehen nach dem Auth-Schreibvorgang und teilen sich keine Transaktion mit ihm, weil der Auth-Dienst den Pool nutzt.

## Später

Ein vom Superuser getrennter Verzeichnis-Administrator. OAuth, MFA und einzeln widerrufbare CSRF-Tokens je Tab sind geplant und nicht implementiert.
