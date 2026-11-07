# Admin-Gestaltung

[Zur Übersicht](admin-native.md) · Vorlagen: [admin-templates.md](admin-templates.md)

## Aufgabe

`crates/ferrum-admin/static/css/admin.css` ist das einzige Stylesheet. Es wird unter `/admin/assets/admin.css` ausgeliefert. Es gibt keine CDN-Adresse und kein Tailwind im nativen Admin.

## Fläche

Die Seite ist eine Spalte aus Seitenleiste und Hauptbereich. Die Seitenleiste zeigt Übersicht, Modelle und, für Superuser, Benutzer, Rollen und Berechtigungen. Der Kopf zeigt Projektname, Seitentitel, Sprache, Darstellung und Abmelden. Darunter liegen Hinweis, Warnung und der Inhalt. Ein Sprunglink führt zu `#content`.

Unter 800 Pixel Breite stehen Seitenleiste und Hauptbereich untereinander. Tabellen bleiben in einem horizontalen Scrollbereich.

## Tokens

Helle Flächen nutzen eine helle Leinwand, eine Karte und eine ruhige Linie. `html[data-theme="dark"]` setzt die dunklen Tokens. `data-theme="system"` folgt `prefers-color-scheme`. `--brand` kommt aus der geprüften Primärfarbe. Statusfarben für Erfolg, Warnung und Gefahr stehen zusätzlich als Text, nicht nur als Fläche.

Buttons, Eingaben und Auswahllisten teilen sich eine Mindesthöhe von 2,5 rem. Der Fokus ist eine sichtbare Outline. Primäre Aktionen nutzen `--brand`. Löschen nutzt die Gefahrenfarbe. Abbrechen und Abmelden sind ruhige Nebenaktionen.

## Tabellen und Formulare

Tabellenköpfe verlinken die Sortierung. Ein Pfeil zeigt die aktuelle Richtung. Zeilen haben genug Abstand zum Lesen. Die Werkzeugleiste enthält Suche und Filter als GET-Formular. Die Seitennavigation ist ein Link auf die vorherige und nächste Seite.

Formulare setzen Label, Pflicht- oder Optionalhinweis, Feld und Fehlertext untereinander. Fehlermeldungen haben `role="alert"`. Hinweise nach dem Speichern haben `role="status"`. Pflichtfelder tragen ein sichtbares Sternchen und den Text aus dem Katalog.

## Barriere

Die Dokumentensprache steht in `lang`. Formularfelder haben Labels. Die Seitenleiste und die Seitennavigation haben Namen. ARIA beschränkt sich auf ungültige Felder, Hinweise und den Sprunglink. Status wie aktiv und inaktiv stehen als Wort, nicht nur als Farbe.

`admin.js` ist mit `defer` eingebunden und ändert keine Daten. Ohne Script bleiben alle Formulare und Links benutzbar.
