---
name: Fexu
description: Serverseitiger Admin für die Nachtschicht, hell und dunkel gleichwertig.
colors:
  slate-door: "#0f172a"
  cool-paper: "#f4f7fb"
  ink: "#142033"
  muted: "#526074"
  line: "#6d7b90"
  divider: "#d5deea"
  card: "#ffffff"
  sidebar-ink: "#ffffff"
  signal-teal: "#0f766e"
  violet: "#6d28d9"
  violet-ink: "#5b21b6"
  night-violet: "#ddd6fe"
  crimson: "#9f1239"
  forest: "#166534"
  night-paper: "#0b1220"
  night-card: "#172234"
  night-line: "#8aa0bd"
  night-divider: "#2c3b52"
  danger-ink: "#ffffff"
  night-danger-ink: "#0f172a"
  night-ink: "#e8eef8"
  night-muted: "#b7c3d6"
  night-sidebar: "#1b2838"
  night-sidebar-ink: "#f8fafc"
  night-action: "#e8eef8"
  night-action-ink: "#0f172a"
  night-teal: "#5eead4"
  night-danger: "#fb7185"
  night-ok: "#86efac"
typography:
  headline:
    fontFamily: "\"Segoe UI\", sans-serif"
    fontSize: "1.6rem"
    lineHeight: 1.5
  title:
    fontFamily: "\"Segoe UI\", sans-serif"
    fontSize: "1.05rem"
    lineHeight: 1.5
  body:
    fontFamily: "\"Segoe UI\", sans-serif"
    lineHeight: 1.5
  label:
    fontFamily: "\"Segoe UI\", sans-serif"
    fontSize: "0.92rem"
    lineHeight: 1.5
rounded:
  control: "0.45rem"
  notice: "0.55rem"
  table: "0.6rem"
  card: "0.75rem"
spacing:
  tight: "0.25rem"
  control: "0.6rem"
  field: "0.8rem"
  card: "1rem"
  page: "1.25rem"
  section: "1.75rem"
components:
  button-primary:
    backgroundColor: "{colors.slate-door}"
    textColor: "{colors.sidebar-ink}"
    rounded: "{rounded.control}"
    padding: "0.35rem 0.7rem"
    height: "2rem"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "0.35rem 0.7rem"
    height: "2rem"
  button-danger:
    backgroundColor: "{colors.crimson}"
    textColor: "{colors.sidebar-ink}"
    rounded: "{rounded.control}"
    padding: "0.35rem 0.7rem"
    height: "2rem"
  input:
    backgroundColor: "{colors.card}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "0.35rem 0.7rem"
    height: "2rem"
  card:
    backgroundColor: "{colors.card}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "1rem"
  sidebar:
    backgroundColor: "{colors.slate-door}"
    textColor: "{colors.sidebar-ink}"
    padding: "1.25rem"
    width: "16rem"
---

# Design System: Fexu

## Overview

**Creative North Star: "Die Nachtschicht"**

Der Admin ist die Schicht, nicht die Bühne. Hell und Dunkel sind zwei gleichwertige Arbeitsmodi. Wer umschaltet, wechselt die Beleuchtung des gleichen Tisches, nicht das Möbel. Die Fläche bleibt zurückhaltend und bestimmt: Listen, Formulare und eine klare Hauptaktion, ohne Werbegeste und ohne die graue Feldkasten-Haut eines klassischen Django-Admins.

Die Dichte ist die einer Verwaltung. Die Marke sitzt an der Schiefertür. Die Arbeitsaktion ist Violett, im Hellen `#6d28d9` mit weißer Schrift, im Dunkeln `#ddd6fe` mit dunkler Schrift. Die Projektfarbe setzt weiterhin `--brand` und färbt nur die Tür im Hellen, wenn der Kontrast hält. Die Nacht-Tür bleibt fest.

**Key Characteristics:**

- Hell und Dunkel sind Arbeitsmodi, kein nachträgliches Theme.
- Eine Systemschrift, keine Display-Schrift.
- Flächen liegen im Ruhezustand flach. Tiefe kommt aus Ton, nicht aus Schattenwurf.
- Fokus ist ein Ring, nicht eine Farbfläche.
- Kernaktionen bleiben ohne Script sichtbar und benutzbar.

## Colors

Die Palette ist Schiefer und Violett. Schiefer trägt die Tür. Violett trägt die Hauptaktion, Links und den Fokusring. Gefahr und Erfolg sind Zustände, keine zweite Marke.

### Primary

- **Schiefertür** (`slate-door`): Seitenleiste im Hellen. Die Projektfarbe ersetzt genau diese Rolle im Hellen über `--brand`. Die Nacht-Tür folgt der Projektfarbe nicht.

### Secondary

- **Violett** (`violet`): Gefüllte Hauptaktion im Hellen, weiße Schrift. **Violett-Tinte** (`violet-ink`) trägt Links und den Fokusring. Im Dunkeln wird die Aktion **Nachtviolett** (`night-violet`) mit dunkler Schrift. Der Fokusring ist dieselbe helle Fläche.

### Neutral

- **Kühles Papier** (`cool-paper`): Seitengrund im Hellen. **Karte** (`card`) ist die weiße Arbeitsfläche darauf.
- **Tinte** (`ink`) ist der Text. **Gedämpft** (`muted`) trägt Meta, leere Zustände und Hinweise. **Linie** (`line`) ist die Kontrollkante und hält mindestens 3:1 zur Fläche. **Trennung** (`divider`) trennt Tabellenzeilen und Karten, ohne selbst eine Kontrolle zu sein.
- **Nachtpapier** (`night-paper`), **Nachtkarte** (`night-card`), **Nachtlinie** (`night-line`), **Nachttrennung** (`night-divider`), **Nachttinte** (`night-ink`) und **Nachtgedämpft** (`night-muted`) sind dieselben Rollen im Dunkeln. Nachtlinie ist hell genug für die Kontrollkante auf der Nachtkarte.
- **Nachtseite** (`night-sidebar`) und **Nachtschrift** (`night-sidebar-ink`) sind die Tür im Dunkeln. Sie folgen nicht der Projektfarbe.
- **Nachtaktion** (`night-action`) mit **Nachtaktion-Tinte** (`night-action-ink`) ist der gefüllte Primärbutton im Dunkeln: helle Fläche, dunkle Schrift.

Gefahr ist **Karmesin** (`crimson`) im Hellen und **Nachtgefahr** (`night-danger`) im Dunkeln. Erfolg ist **Waldgrün** (`forest`) und **Nachtbereit** (`night-ok`). Hinweise mischen Erfolg zu etwa 18 Prozent in die Kartenfläche und setzen dieselbe Farbe als Rand.

**The Equal Shift Rule.** Hell und Dunkel bilden dieselben Rollen ab. Ein Modus ist nicht die eingefärbte Variante des anderen.

**The Contrast Gate Rule.** Die Projektfarbe darf Tür und Primärbutton in beiden Modi färben, sobald der Kontrast hält. Bis dieser Nachweis im Code steht, färbt sie nur den hellen Modus. Die Nachtflächen bleiben fest.

## Typography

**Body Font:** Segoe UI (mit sans-serif)

**Character:** Eine Systemschrift für die ganze Schicht. Überschriften sind größer, nicht aus einer anderen Familie. Es gibt keine Display-Schrift, und es soll keine zweite Schrift dazukommen.

### Hierarchy

- **Headline** (1.6rem, Zeilenhöhe 1.5): Seitentitel. Die Schriftstärke setzt das System nicht; die Überschrift behält das Gewicht der Browser-Voreinstellung.
- **Title** (1.05rem, Zeilenhöhe 1.5): Überschrift in einer Karte.
- **Body** (Zeilenhöhe 1.5): Tabellen, Fließtext, Meldungen. Die Größe ist die des Dokuments, nicht eine eigene Skala.
- **Label** (0.92rem, Zeilenhöhe 1.5): Feldbeschriftung, direkt über der Kontrolle, Abstand 0.25rem.

**The One Face Rule.** Segoe UI bleibt die einzige Familie. Eine dekorative Überschriftenschrift würde die Schicht zur Werbefläche machen.

## Layout

Die Seite ist ein Raster aus Tür und Tisch: Seitenleiste 16rem, Inhalt nimmt den Rest, Mindesthöhe der Ansicht. Ohne Seitenleiste, also Anmeldung und die Staff-Sperre, wird die Spalte eine und der Inhalt höchstens 36rem, zentriert.

Der Tisch hat 1.25rem Abstand oben und seitlich 1.5rem, unten 3rem. Eine Werkzeugleiste sitzt 1.25rem über der Tabelle. Suche und Filter bleiben beisammen, der Neueintrag steht am anderen Ende. Unabhängige Abschnitte beginnen 1.75rem unter dem Vorherigen. Formulare in einer Karte sind höchstens 40rem breit und stapeln Felder mit 0.8rem Abstand.

Unter 800px wird das Raster eine Spalte. Die Seitenleiste bleibt offen und wird eine senkrechte Liste. Sie braucht kein Script. Werkzeugleiste und ihre Kontrollen nehmen die volle Breite. Definitionslisten und die Kopfzeile stapeln.

**The Open Stair Rule.** Was auf dem großen Schirm nebeneinander steht, stapelt unter 800px. Es verschwindet nicht hinter einem Schalter.

## Elevation & Depth

Die Schicht ist im Ruhezustand flach. Im Hellen hebt eine Karte sich mit einem einzigen Hauch vom Papier. Im Dunkeln ist dieser Schatten aus. Tiefe entsteht dort aus dem Tonunterschied von Nachtpapier, Nachtkarte und Nachtseite.

### Shadow Vocabulary

- **Ruhehauch** (`box-shadow: 0 1px 2px rgba(15, 23, 42, .06)`): Nur Karten im Hellen.
- **Keine Hebung** (`box-shadow: none`): Alle Flächen im Dunkeln, und jede Fläche im Hellen, die keine Karte ist.

**The Flat Rest Rule.** Eine Fläche wirft im Ruhezustand keinen Schatten, außer der Karte im Hellen. Hover erzeugt keine zusätzliche Hebung.

## Shapes

Ecken sind durchgehend gerundet. Kontrollen nutzen 0.45rem, ein Hinweis 0.55rem, die Tabelle 0.6rem, die Karte 0.75rem. Linien sind 1px. Kontrollen sind 2rem hoch. Die Checkbox bleibt 1.2rem. Unter 800px sind die Türlinks 2.75rem hoch.

## Components

### Buttons

- **Shape:** Kontrollrundung (0.45rem), Höhe 2rem, Innenabstand 0.2rem 0.65rem, Schrift 0.84rem.
- **Primary:** Im Hellen Violett mit weißer Schrift. Im Dunkeln Nachtviolett mit dunkler Schrift.
- **Hover / Focus:** Fokus ist ein 3px Ring in Violett-Tinte, 2px außerhalb der Kante. Auf der Tür ist der Ring weiß.
- **Ghost:** Transparente Fläche, Text in Tinte. Zweite Aktion.
- **Danger:** Karmesin mit Gefahren-Tinte im Hellen. Im Dunkeln Nachtgefahr mit Nacht-Gefahren-Tinte.

### Cards / Containers

- **Corner Style:** Kartenrundung (1rem).
- **Background:** Karte, im Dunkeln Nachtkarte.
- **Shadow Strategy:** Ruhehauch nur im Hellen. Siehe Elevation.
- **Border:** 1px Linie.
- **Internal Padding:** 1rem. Die Überschrift darin hat 0.6rem Luft nach unten.

### Inputs / Fields

- **Style:** Wie der Ghost-Button in der Form, aber mit Kartenfläche und Tintenschrift. Label sitzt im Raster darüber.
- **Focus:** Derselbe 3px Violett-Ring wie bei Buttons.
- **Error / Disabled:** Der Fehlertext ist Karmesin unter dem Feld. Deaktivierte Kontrollen bleiben sichtbar und sinken auf 45 Prozent Deckkraft.

### Navigation

Die Tür zeigt den Anwendungsnamen, dann Übersicht, die registrierten Apps mit ihren Modellen, für Superuser Benutzer und Rollen, und unter System die Historie. Der aktuelle Eintrag hat Schriftstärke 650 und eine violette Fläche, keine Unterstreichung. Modelle stehen eingerückt unter dem App-Namen. Unter 800px bleibt die Liste senkrecht und offen. Anmeldung und Staff-Sperre haben keine Tür.

### Hinweis

Erfolg sitzt über dem Inhalt: Waldgrün zu etwa 18 Prozent in die Karte gemischt, Rand in Waldgrün, Ecke 0.7rem, Innenabstand 0.7rem 0.9rem. Der Text bleibt Tinte. Fehler sind Text in Karmesin, ohne eigene Fläche.

## Do's and Don'ts

### Do

- **Do** Hell und Dunkel als dieselben Rollen behandeln: Papier, Karte, Tinte, Linie, Tür, Violett, Gefahr, Erfolg.
- **Do** den Fokusring 3px stark und 2px außerhalb der Kante zeichnen. Auf der Tür ist der Ring weiß.
- **Do** unter 800px stapeln, statt Aktionen zu verstecken.
- **Do** Status mit Worten tragen. Farbe allein unterscheidet Staff, Superuser und Aktiv nicht.
- **Do** Segoe UI für Titel, Label und Tabelle verwenden.

### Don't

- **Don't** eine Display-Schrift oder eine zweite Familie einführen.
- **Don't** die Fläche als Werbebild bauen: kein Hero, keine großen Bilder, keine Dekorflächen.
- **Don't** die graue Feldkasten-Haut eines klassischen Django-Admins nachbauen.
- **Don't** die Projektfarbe im Dunkeln auf Tür oder Primärbutton legen, bevor der Kontrast geprüft ist.
- **Don't** im Dunkeln den Ruhehauch der hellen Karte weiterverwenden.
- **Don't** eine Kernaktion nur nach einem Script zeigen oder nur über Hover erreichbar machen.
