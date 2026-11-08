# Projektgenerator

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrioxid-cli/src/scaffold.rs`.

## Aufgabe

`ferrioxid new` legt ein eigenständiges Ferrioxid-Projekt an. Es kompiliert außerhalb des Ferrioxid-Workspaces, nutzt SQLite und registriert ein Beispielmodell. Fullstack-Projekte übergeben Branding und Währung an `AdminRegistry::with_branding` und brauchen für den Admin kein Node.js. Die Startseite enthält feste Sprachlinks und einen lokalisierten 500-Text. Bereits erzeugte Projekte übernehmen das nicht automatisch. Seit v0.12 legt der Generator `static/css/app.css`, `static/js/app.js` und `media/.gitkeep` an. `media/*` ist ignoriert. `ferrum.toml` enthält `[static]` und `[storage]`. Fullstack-Projekte haben `/contact`. Die API-Vorlage montiert das Formular nicht.

## Zuständige Dateien

- `crates/ferrioxid-cli/src/scaffold.rs`
- `crates/ferrioxid-cli/src/main.rs`
- `crates/ferrioxid-cli/tests/generator.rs`

## Öffentliche Schnittstelle

```powershell
ferrioxid new myproject
ferrioxid new myapi --template api
ferrioxid new myshop --template fullstack --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
```

Das Standardtemplate ist `fullstack`. `--ferrioxid-path` zeigt auf das lokale Ferrioxid-Repository, solange die Crates nicht veröffentlicht sind. Ohne das Argument verwendet die CLI den Pfad, der beim Übersetzen von `ferrioxid-cli` bekannt war.

## Konfiguration

Das erzeugte `ferrum.toml` enthält Projekt, Datenbank, Server, Admin, Auth, Vorlagen und Branding. Dazu kommt `templates/` mit `base.html`, `index.html` und `errors/`. Die Datenbank-URL ist `sqlite://db.sqlite3?mode=rwc`. Der Server ist `127.0.0.1:3000`. Secrets stehen nicht in der Datei. `.env.example` nennt `DATABASE_URL`, `BIND_ADDRESS` und auskommentierte Hinweise auf `FERRUM_CSRF_SECRET` und `FERRUM_PASSWORD`.

## Datenfluss

1. Der Projektname wird geprüft.
2. Das Zielverzeichnis darf noch nicht existieren.
3. Der Ferrioxid-Pfad wird kanonisiert und muss `crates/ferrioxid/Cargo.toml` enthalten.
4. Unter Windows entfernt die CLI das Präfix `\\?\`, damit Cargo den Pfad als Abhängigkeit akzeptiert.
5. Dateien werden neu geschrieben. Eine vorhandene Datei wird nicht ersetzt.
6. `0001_initial.json` entsteht über denselben Siegelweg wie andere Migrationen.

## Fehlerfälle

| Fall | Verhalten |
|---|---|
| Name beginnt nicht mit einem Buchstaben, enthält Pfad- oder Shellzeichen oder ist reserviert | Abbruch, nichts wird angelegt |
| Zielverzeichnis existiert | Abbruch, Inhalt bleibt |
| `--ferrioxid-path` enthält kein Ferrioxid-Crate | Abbruch |

## Sicherheitsregeln

Keine Shell-Interpolation des Namens. Kein `0.0.0.0`. Kein eingebautes Passwort. Kein Überschreiben. Der Abhängigkeitspfad zeigt auf `crates/ferrioxid`, nicht auf einzelne interne Dateien.

## Beispiel

```powershell
ferrioxid new demo --template fullstack --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
cd demo
ferrioxid migrate up
ferrioxid admin create-user --email admin@example.com --staff --superuser
ferrioxid dev
```

`create-user` fragt das Passwort verdeckt ab. Für Automationen liest die CLI `FERRUM_PASSWORD`, mindestens 12 Zeichen. Es gibt keinen Standardwert.

## Tests

`crates/ferrioxid-cli/tests/generator.rs` erzeugt API- und Fullstack-Projekte in einem temporären Verzeichnis, lehnt `../escape` und ein vorhandenes Ziel ab und prüft `cargo check`.

## Einschränkungen

Versionierte crates.io-Abhängigkeiten sind geplant und nicht der Standard. `Cargo.lock` schreibt Cargo beim ersten Übersetzen, nicht der Generator. Ein generiertes Modell nach `ferrioxid generate model` erhält keine eigene Migration. Die legt `ferrioxid migrate make` an.
