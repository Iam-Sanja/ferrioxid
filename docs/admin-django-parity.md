# Django-Admin und Ferrum

[Zur Übersicht](index.md) · Roadmap: [roadmap.md](roadmap.md) · UX: [admin-ux-backlog.md](admin-ux-backlog.md)

**Stand:** v0.22. Die Cargo-Version bleibt `0.1.0`. Die Tabelle darunter bewertet den nativen Admin. Funktionsgleichheit ist das Ziel. Python-`ModelAdmin`, Django-Templates und Django-URLs werden nicht nachgebaut. Datum, Uhrzeit und UTC-Zeitpunkt sind Modelltypen. Die Datumshierarchie filtert nach dem UTC-Kalender. Verzeichnisänderungen und opt-in Eltern-Kind-Formulare sind jeweils eine Transaktion.

Statuswerte:

- **Ist:** im nativen Admin benutzbar und getestet oder im Code fest verdrahtet.
- **Teil:** vorhanden, aber enger oder unvollständig gegenüber dem genannten Django-Begriff.
- **Fehlt:** nicht vorhanden. Steht in der Roadmap, sobald eine Phase genannt ist.
- **Unpassend:** für Ferrum nicht vorgesehen, weil es Codekompatibilität oder eine JavaScript-Pflicht wäre.

## Listen

| Django | Ferrum | Status | Phase | Dateien |
|---|---|---|---|---|
| `list_display` | `#[admin(list_display)]` hat Vorrang. Ohne weitere Markierung: erstes sichtbares Textfeld, sonst ein anderes sichtbares Feld, plus `id` | Ist | — | `ferrum-macros`, `html.rs` `listed_fields`, `templates/models/list.html` |
| `list_display_links` | `#[admin(list_link)]`. Ohne Markierung die erste gelistete Spalte außer `id` | Ist | — | `list.html`, [admin-list-configuration.md](admin-list-configuration.md) |
| `list_filter` | `filterable` und registrierte `AdminFilter` | Ist | Bool, Fremdschlüssel, Zahl und feste Auswahlen. Datumsbereiche bleiben offen | [admin-custom-filters.md](admin-custom-filters.md) |
| `search_fields` | `searchable` auf String-Spalten, Parameter `q` | Ist | — | Makro, `html.rs` |
| `ordering` | `admin_ordering`, sonst `id`. Ein Query-`sort` hat Vorrang und wird gegen die Metadaten geprüft | Ist | — | `html.rs` `resolved_sort` |
| `list_per_page` | `admin_per_page`, 1 bis 100, HTML-Vorgabe 25. JSON bleibt bei 100 | Ist | — | `html.rs` |
| `list_max_show_all` | Es gibt kein „alle zeigen“. Die Seite ist gedeckelt | Ist | — | `html.rs` |
| `list_select_related` | Kein Join. Anzeigenamen kommen aus einer Abfrage je Zieltabelle und Seite | Teil | — | `ferrum-db` `query_labels` |
| `date_hierarchy` | `admin_date_hierarchy` auf `NaiveDate` oder `DateTime<Utc>` | Ist | UTC-Kalender, höchstens 100 Gruppen, Query-Scope bleibt | [admin-date-hierarchy.md](admin-date-hierarchy.md) |
| Aktionen auf mehreren Zeilen | Checkbox je Zeile, `scope=page` für die aktuelle Seite, registrierte Aktion, Bestätigungsseite | Ist | Löschen, Aktivieren, Deaktivieren und `AdminAction`. Alles oder nichts in einer Transaktion | [admin-bulk-actions.md](admin-bulk-actions.md) |
| Erweiterte Listenzeilen | Keine eingeblendeten Zusatzzeilen unter der Listenzeile | Fehlt | offen | Template |

## Formulare

| Django | Ferrum | Status | Phase | Dateien |
|---|---|---|---|---|
| `fields` | Sichtbare Felder kommen aus `AdminFieldMeta`, abzüglich verborgen und sensibel | Ist | — | `visible_fields` |
| `exclude` | `hidden` blendet aus und `reject_locked` lehnt POST ab | Teil | — | Makro, `html.rs` |
| `fieldsets` | `admin_fieldset` und `#[admin(fieldset)]`. Ohne Deklaration eine unbenannte Gruppe | Ist | — | [admin-fieldsets.md](admin-fieldsets.md) |
| `readonly_fields` | `readonly` und `id` sind Text, nicht eingebbar | Ist | — | `field.html`, `list.html` |
| `autocomplete_fields` | Select mit höchstens 100 lesbaren Zielen plus Auswahlseite mit Suche und Seiten | Ist | ohne Script | [admin-relational-widgets.md](admin-relational-widgets.md) |
| `raw_id_fields` | Keine freie Roh-ID-Eingabe. Eine unlesbare UUID wird beim Speichern abgelehnt | Teil | — | `ensure_relations` |
| `filter_horizontal` / `filter_vertical` | Keine Mehrfachauswahl mit zwei Listen | Unpassend als JavaScript-Widget. Eine Mehrfachauswahl ohne Script kann später eine Checkbox-Gruppe sein | v0.18, nur wenn Many-to-Many existiert | — |
| `formfield_overrides` | `AdminWidget` für Text, Textbereich, E-Mail, URL, Zahl und Nur-lesen | Teil | Fremdschlüssel bleiben das eingebaute Select | [admin-custom-widgets.md](admin-custom-widgets.md) |
| Vorbelegung | Kein Default aus der Admin-Konfiguration. Datenbank-Defaults bleiben in der Migration | Fehlt | v0.17 | Formular-Handler |
| Validierung | Feldregeln und `can_create` laufen serverseitig. Fehler stehen am Feld | Ist | — | `ferrum-forms`, `html.rs` |
| Validierung während der Eingabe | Kein Script prüft vor dem POST | Unpassend als Pflicht. Serverseitige Fehler bleiben die Abnahme | — | — |

## Relationen

| Fähigkeit | Status | Phase | Hinweis |
|---|---|---|---|
| Belongs-To | Ist | — | Select bis 100 plus Auswahlseite. Unlesbare IDs werden nicht gespeichert. Liste und Detail zeigen den Metadaten-Namen |
| Listenfilter | Ist | — | Bool und Fremdschlüssel sind Auswahlen, Zahlen sind `type="number"`, Text bleibt Suche. Alles serverseitig |
| Has-Many | Ist | — | `admin_inline` zeigt die Kindliste mit Leserecht und Seiten. Anlegen ist ein eigenes Formular |
| Inline-Formulare | Teil | — | Ohne `form` bleibt das eigene Anlegeformular. Mit `form` werden Elternobjekt und eine Has-Many-Ebene gemeinsam gespeichert. Kein Many-to-Many, keine Verschachtelung, höchstens 50 Kinder |
| Autovervollständigung | Ist | — | GET-Auswahlseite, keine beliebige ID, kein Pflicht-Skript |
| Berechtigte Auswahl | Ist | — | Ohne Leserecht keine Optionen |
| Relationale Änderung | Teil | v0.18 | Ein Fremdschlüssel kann gesetzt werden. Kaskaden bleiben die der Migration |

## Aktionen und Lebenszyklus

| Fähigkeit | Status | Phase |
|---|---|---|
| Speichern und Redirect auf das Detail | Ist | — |
| Speichern und weiter bearbeiten | Ist | `save=edit`, nur nach erfolgreichem Schreiben |
| Speichern und neu anlegen | Ist | `save=new`. Ein anderer Wert ist 422 und schreibt nicht |
| Löschen mit Bestätigungsseite | Ist | GET löscht nicht |
| Benutzerdefinierte Admin-Aktionen | Ist | `Delete`, `SetBool` und `ActionHandler`. Der HTTP-Name führt nur registrierte Aktionen aus |
| Änderungshistorie | Teil | Superuser sieht alle Ereignisse. Anderes Staff nur eigene. Zusammenfassung ohne sensible Felder |
| Audit-Log | Teil | Modellschreibung und Verzeichnisänderungen teilen die Transaktion mit ihrem Audit. `operation_id` gruppiert Eltern- und Kindzeilen. Die Historienseite zeigt acht Zeichen. CLI-Auth bleibt ohne dieses Audit |
| Objektbezogene Berechtigung | Ist | `ObjectPolicy`, nicht lesbare Sätze sind 404 wie fehlende Sätze |
| Eigene Admin-Hooks | Teil | `ViewHandler` und `ActionHandler`. Keine Pfade aus der Anfrage |

## Erweiterbarkeit

| Fähigkeit | Status | Phase | Abgrenzung |
|---|---|---|---|
| Eigene Admin-Views | Ist | v0.22 | `/admin/pages/{key}`, Staff, Recht, CSRF. Kein offener Pfad |
| Eigene Templates | Teil | v0.22 | Registrierte Namen zusätzlich zu den eingebetteten. Kein Pfad aus der Anfrage |
| Modellbezogene Einstellungen | Ist | — | Zusätzlich `admin_inline`. Keine freie Python-Klasse |
| Eigene Widgets | Teil | v0.22 | Text, Textbereich, E-Mail, URL, Zahl, Nur-lesen. Ohne Script |
| Eigene Filter | Ist | v0.22 | Feste Spalte, gebundener Wert, Query-Scope bleibt |
| Eigene Aktionen | Ist | v0.22 | `try_action` und `ActionHandler`. POST, CSRF, Recht, Bestätigung, keine Ausführung über GET |
| Wiederverwendbare Bausteine | Teil | v0.15 | Heute Tera-Blöcke. Keine React-Komponenten |

## Benutzer und Rechte

| Fähigkeit | Status | Hinweis |
|---|---|---|
| `is_staff` als Admin-Tür | Ist | Ohne Staff kein Modell und kein Verzeichnis |
| `is_superuser` überspringt Modellrechte | Ist | Ersetzt Staff nicht |
| `is_active` | Ist | Inaktive Sitzungen enden bei der nächsten Prüfung |
| Benutzer anlegen, E-Mail ändern, Passwort setzen | Ist | Eigene HTML-Routen. Superuser bleibt CLI. Kein Hard-Delete |
| Rollen und Rechte | Ist | Anlegen, Geben, Nehmen, Zuweisen. Passwort und Deaktivierung widerrufen Sitzungen |
| Superuser vergeben | Unpassend im Admin | Bleibt CLI. Der Admin antwortet 403 auf Superuser-Mutation |
| Selbstsperre | Ist | Eigenes Deaktivieren und eigener Staff-Entzug sind blockiert |

## Was Ferrum nicht übernimmt

- Dieselbe Klassen- und Methodensignatur wie `django.contrib.admin`.
- Django-Templates, Django-Forms oder das Django-Design.
- Ein Admin, der ohne Server-HTML nur mit JavaScript funktioniert.
- `AdminSite` zur Laufzeit aus Benutzerdateien nachladen.
- Eine Allowlist, die `Origin: null` allgemein akzeptiert. Die Ausnahme `Sec-Fetch-Site: same-origin` aus v0.13.1 bleibt die einzige.
