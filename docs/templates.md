# Vorlagen

[Zur Übersicht](index.md) · **IST-Dateien:** `crates/ferrioxid-api/src/pages.rs`, `crates/ferrioxid-cli/src/scaffold.rs`.

## Aufgabe

Normale Seiten außerhalb des Admins werden serverseitig mit Tera gerendert. Die Admin-Vorlagen liegen getrennt in `crates/ferrioxid-admin/templates` und werden nicht aus dem Projektordner geladen. Siehe [admin-templates.md](admin-templates.md). Tera entspricht dem Jinja- und Django-Modell: Vererbung über `extends`, Bausteine über `include`, Variablen in `{{ }}`. Formular-HTML aus `ferrioxid::forms::render_form` ist zusätzlich verfügbar und escaped Feldwerte. Die Kontaktseite des Generators und von myshop nutzt diesen Weg. Das Stylesheet liegt unter `/static/css/app.css`.

## Zuständige Dateien

- `crates/ferrioxid-api/src/pages.rs`
- `crates/ferrioxid/src/lib.rs`
- `templates/` im generierten Projekt

## Öffentliche Schnittstelle

`ferrioxid::pages::Site::open` lädt `**/*.html`. `render` erzeugt eine Seite. `home_context` füllt Projektname, Sprache, Admin-Flag und die kurzen Standardtexte. `mount_pages` hängt `/` und eine 404-Antwort an den Router, wenn `templates.enabled` wahr ist. Pro Anfrage wählt `negotiate_locale` die Sprache. `message` liefert die vorbereiteten Texte für `de` und `en`, darunter Willkommen, 404 und 500. Eine unbekannte Locale fällt auf Englisch zurück und wird nicht als Pfad verwendet.

## Konfiguration

```toml
[templates]
enabled = true
engine = "tera"
templates_dir = "templates"
```

## Datenfluss

Die Anwendung lädt die Einstellungen, öffnet das Vorlagenverzeichnis und rendert bei jedem Aufruf neu. Eine fehlende Startseite verhindert den Start. Schlägt das Rendern fehl, wird `errors/500.html` versucht.

## Fehlerfälle

| Fall | Verhalten |
|---|---|
| `index.html` fehlt | Start mit Fehlermeldung |
| unbekannte Engine | `ferrum.toml` wird abgelehnt |
| unbekannte Route | 404 aus `errors/404.html` |

## Sicherheitsregeln

`primary_color` darf nur ein Hex-Wert sein, damit er in das CSS der Vorlage eingesetzt werden kann. Vorlagenpfade bleiben relativ und ohne `..`.

## Beispiel

```html
{% extends "base.html" %}
{% block content %}
<h1>{{ project_name }}</h1>
<p>{{ running }}</p>
{% endblock %}
```

## Tests

`index_template_renders_the_project_name` rendert ein temporäres Projekt. Der Generator legt `templates/base.html`, `index.html` und `errors/` an. Ein generiertes Projekt muss mit `cargo check` übersetzen.

## Einschränkungen

Panics werden nicht automatisch zur 500-Seite. `ferrioxid dev` lädt Vorlagen beim Prozessstart neu, nicht bei jedem Speichern ohne Neustart. Eine Änderung einer Rust-Datei startet den Prozess neu und lädt die Vorlagen damit erneut. `include` für Partials ist möglich, der Generator legt aber noch keine eigene Partial-Datei an. Die Basisvorlage ist das gemeinsame Layout.
