# Migrations-Engine – Entwurf (NICHT implementiert)

[Zur Übersicht](index.md) · **Status:** ausschließlich SOLL. Es existiert **keine** `ferrum-migrations`-Crate in v0.1.

## Ziel und Grenze

Aktuell liefert `FerrumModel::schema_sql()` ein `CREATE TABLE IF NOT EXISTS`, das der `myshop --bootstrap`-Pfad explizit ausführt. Das erkennt **keinen** Schema-Drift, ändert bestehende Tabellen nicht und besitzt weder Versionierung noch Rollback. In keinem Kontext als Migration bezeichnen.

## Soll-Architektur

```text
Rust-Structs → Makro → normalisiertes Schema-Metadatenmodell
                              ↓
                       Schema Snapshot
                              ↓
                      Snapshotvergleich
                              ↓
                  Migration Plan + Review
                              ↓
               PostgreSQL Executor + Lock
                              ↓
                      Migration History
```

### 1. Metadaten und Snapshot

`ModelSchema` enthält Tabellen, Spalten (Name/Typ/Nullable/Default), Primär-/Fremdschlüssel, Indizes und Constraints. Serialisiertes Snapshot-Format mit eigener Version und deterministischer Sortierung. **SQL-Strings dürfen nicht der einzige Vergleichswert sein.**

### 2. Diff-Planer

Eindeutige Fälle: neue Tabelle, nullable Spalte, Index. Riskante Fälle: Drop, Rename, NOT NULL ohne Backfill, Typkonvertierung, PK/Constraint-Wechsel. Rename ohne ausdrückliche Migrationsabsicht nicht als Drop+Add interpretieren. Diff erzeugt Prüfhinweise und einen konkreten SQL-Entwurf.

### 3. Migration-Record

Versions-ID, Inhalt-Hash, Ausführungszeit, Status, ggf. Revisionsmetadaten. Bereits angewendete Migrationen dürfen inhaltlich nicht still verändert werden. PostgreSQL Advisory Lock oder gleichwertiger Mechanismus gegen konkurrierende Ausführung.

### 4. Executor

Transaktionsfähige Schritte transaktional ausführen; nicht-transaktionale PostgreSQL-Operationen explizit markieren. Fehler mit Wiederanlaufstrategie dokumentieren. Vor gefährlichen Änderungen ausdrückliche Freigabe und Backups verlangen. Kein Auto-Apply beim Start des HTTP-Servers.

### 5. CLI-Vertrag (geplant)

```bash
ferrum migrate make
ferrum migrate status
ferrum migrate up
ferrum migrate down
```

`down` nur, wenn sicher definiert; keine vorgetäuschte Reversibilität nach Datenverlust. `make` soll lesbare, versionierte Dateien unter einem festen Projektpfad erzeugen und reviewbar sein.

## Test-/Abnahmeplan

- Clean-DB: Snapshot → make → up → Schema entspricht Modell.
- Zweiter Lauf ohne Änderungen: leeres Diff.
- Neue Boolean-Spalte mit Default: DDL und vorhandene Rows prüfen.
- Rename/Drop/Type-Change: als riskant klassifizieren und nicht ohne Entscheidung ausführen.
- Zwei parallele Runner: nur eine Migration kann Lock übernehmen.
- Fehler mitten im Lauf: korrekter History-Status, keine unbemerkte Teilmigration.
- Manuelle Datenbankänderung: Drift melden, nicht still überschreiben.

## Abhängigkeiten

Benötigt reichhaltige Feldmetadaten aus [core.md](core.md) und [macros.md](macros.md), PostgreSQL-Abstraktion aus [database.md](database.md) und CLI aus [cli.md](cli.md). Erst diese Verträge festlegen; nicht parallel eine zweite Modelldefinitionssprache aufbauen.
