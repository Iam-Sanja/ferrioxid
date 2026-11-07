---
name: Ferrum
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
  control: "0.6rem"
  notice: "0.7rem"
  table: "0.8rem"
  card: "1rem"
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
    height: "2.5rem"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "0.35rem 0.7rem"
    height: "2.5rem"
  button-danger:
    backgroundColor: "{colors.crimson}"
    textColor: "{colors.sidebar-ink}"
    rounded: "{rounded.control}"
    padding: "0.35rem 0.7rem"
    height: "2.5rem"
  input:
    backgroundColor: "{colors.card}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "0.35rem 0.7rem"
    height: "2.5rem"
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

# Design System: Ferrum

## Overview

**Creative North Star: "Die Nachtschicht"**

Der Admin ist die Schicht, nicht die Bühne. Hell und Dunkel sind zwei gleichwertige Arbeitsmodi. Wer umschaltet, wechselt die Beleuchtung des gleichen Tisches, nicht das Möbel. Die Fläche bleibt zurückhaltend und bestimmt: Listen, Formulare und eine klare Hauptaktion, ohne Werbegeste und ohne die graue Feldkasten-Haut eines klassischen Django-Admins.

Die Dichte ist die einer Verwaltung, nicht die eines Dashboards mit Kacheln um ihrer selbst willen. Die Marke sitzt an der Tür. Im Hellen ist das die Projektfarbe, mit Schiefertür als eingebautem Standard. Im Dunkeln bleiben Seitenleiste und Primärbutton auf festen Nachtflächen, bis ein Kontrastnachweis die Projektfarbe auch dort trägt. Diese Erweiterung ist beschlossen und nicht gebaut.

**Key Characteristics:**

- Hell und Dunkel sind Arbeitsmodi, kein nachträgliches Theme.
- Eine Systemschrift, keine Display-Schrift.
- Flächen liegen im Ruhezustand flach. Tiefe kommt aus Ton, nicht aus Schattenwurf.
- Fokus ist ein Ring, nicht eine Farbfläche.
- Kernaktionen bleiben ohne Script sichtbar und benutzbar.

## Colors

Die Palette ist Schiefer und Teal. Schiefer trägt die Tür und die Hauptaktion. Teal trägt Fokus und Links. Gefahr und Erfolg sind Zustände, keine zweite Marke.

### Primary

- **Schiefertür** (`slate-door`): Standard für Seitenleiste und gefüllten Primärbutton im Hellen. Die Projektfarbe aus der Konfiguration ersetzt genau diese Rolle im Hellen, indem sie `--brand` setzt. Seitenleiste und Aktion folgen dort dieser Variable.

### Secondary

- **Signal-Teal** (`signal-teal`): Links, Fokusring der Arbeitsfläche und der einzige chromatische Akzent neben der Tür. Im Dunkeln wird daraus **Nacht-Teal** (`night-teal`), heller, damit der Ring auf Nachtpapier steht.

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

Unter 800px wird das Raster eine Spalte. Die Seitenleiste wird eine umbrechende Linkliste unter dem Kopf, kein Menü, das ein Script öffnen müsste. Werkzeugleiste und ihre Kontrollen nehmen die volle Breite. Definitionslisten und die Kopfzeile stapeln.

**The Open Stair Rule.** Was auf dem großen Schirm nebeneinander steht, stapelt unter 800px. Es verschwindet nicht hinter einem Schalter.

## Elevation & Depth

Die Schicht ist im Ruhezustand flach. Im Hellen hebt eine Karte sich mit einem einzigen Hauch vom Papier. Im Dunkeln ist dieser Schatten aus. Tiefe entsteht dort aus dem Tonunterschied von Nachtpapier, Nachtkarte und Nachtseite.

### Shadow Vocabulary

- **Ruhehauch** (`box-shadow: 0 1px 2px rgba(15, 23, 42, .06)`): Nur Karten im Hellen.
- **Keine Hebung** (`box-shadow: none`): Alle Flächen im Dunkeln, und jede Fläche im Hellen, die keine Karte ist.

**The Flat Rest Rule.** Eine Fläche wirft im Ruhezustand keinen Schatten, außer der Karte im Hellen. Hover erzeugt keine zusätzliche Hebung.

## Shapes

Ecken sind durchgehend gerundet, in vier Stufen und ohne scharfe Abschlüsse. Kontrollen, also Feld, Auswahl und Button, nutzen 0.6rem. Ein Hinweis nutzt 0.7rem, die Tabellenfläche 0.8rem, die Karte 1rem. Linien sind 1px und durchgehend, nicht gestrichelt. Kontrollen sind mindestens 2.5rem hoch. Die Checkbox ist die Ausnahme: 1.2rem im Quadrat.

## Components

### Buttons

- **Shape:** Kontrollrundung (0.6rem), Mindesthöhe 2.5rem, Innenabstand 0.35rem 0.7rem.
- **Primary:** Im Hellen Schiefertür beziehungsweise die Projektfarbe, Schrift weiß. Im Dunkeln Nachtaktion mit Nachtaktion-Tinte. Es gibt keinen eigenen Hover-Ton.
- **Hover / Focus:** Fokus ist ein 3px Ring in Signal-Teal, 2px außerhalb der Kante. Die Seitenleiste nutzt dafür Weiß, weil der Teal auf der Tür untergeht.
- **Ghost:** Transparente Fläche, Text in Tinte. Zweite Aktion, nicht die Hauptaktion.
- **Danger:** Karmesin mit Gefahren-Tinte (`danger-ink`) im Hellen. Im Dunkeln Nachtgefahr mit Nacht-Gefahren-Tinte (`night-danger-ink`), weil Weiß auf Nachtgefahr unter 4,5:1 liegt.

### Cards / Containers

- **Corner Style:** Kartenrundung (1rem).
- **Background:** Karte, im Dunkeln Nachtkarte.
- **Shadow Strategy:** Ruhehauch nur im Hellen. Siehe Elevation.
- **Border:** 1px Linie.
- **Internal Padding:** 1rem. Die Überschrift darin hat 0.6rem Luft nach unten.

### Inputs / Fields

- **Style:** Wie der Ghost-Button in der Form, aber mit Kartenfläche und Tintenschrift. Label sitzt im Raster darüber.
- **Focus:** Derselbe 3px Teal-Ring wie bei Buttons.
- **Error / Disabled:** Der Fehlertext ist Karmesin unter dem Feld. Ein eigener Disabled-Ton ist nicht definiert.

### Navigation

Die Tür ist eine Spalte mit dem Anwendungsnamen in fetter Schrift. Der Anwendungskopf sitzt darunter, nicht über der Seitenüberschrift. Links sind mindestens 2,75rem hoch. Der aktuelle Eintrag ist unterstrichen und in Schriftstärke 650, ohne eigene Farbfläche. Unter 800px werden die Links eine Zeile, die umbricht. Auf der Sperrseite und bei der Anmeldung fehlt die Tür.

### Hinweis

Erfolg sitzt über dem Inhalt: Waldgrün zu etwa 18 Prozent in die Karte gemischt, Rand in Waldgrün, Ecke 0.7rem, Innenabstand 0.7rem 0.9rem. Der Text bleibt Tinte. Fehler sind Text in Karmesin, ohne eigene Fläche.

## Do's and Don'ts

### Do

- **Do** Hell und Dunkel als dieselben Rollen behandeln: Papier, Karte, Tinte, Linie, Tür, Aktion, Teal, Gefahr, Erfolg.
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
