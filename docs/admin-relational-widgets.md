# Fremdschlüssel-Auswahl

[Zur Übersicht](admin.md) · Sicherheit: [admin-security.md](admin-security.md)

**Stand:** v0.17. Die Cargo-Version bleibt `0.1.0`.

Das Formular zeigt weiter ein `<select>` mit höchstens 100 lesbaren Zielen. Fehlt der aktuelle Wert in dieser Seite und darf der Benutzer ihn lesen, wird er vorne ergänzt. Darunter steht der Link „Weiter suchen“.

Die Auswahlseite ist GET:

- `/admin/models/{modell}/pick/{feld}`
- `/admin/models/{modell}/{id}/pick/{feld}`

Sie prüft Staff, das Schreibrecht des Ausgangsmodells und das Leserecht des Ziels. Die Suche nutzt die sichtbaren `searchable`-Spalten des Ziels. Die Seite nutzt `admin_per_page` des Ziels, gedeckelt bei 100, und denselben Query-Scope wie die Liste. Ein unbekannter Feldname, ein verborgenes oder nur lesbares Feld und ein Ziel ohne Anzeigetext sind 404. Ohne Leserecht ist die Antwort 403.

Ein Treffer verlinkt nur auf das Formular des Ausgangsmodells und setzt den Feldnamen auf die UUID aus der Datenbank. Andere Formulardaten bleiben dabei nicht erhalten. Es gibt keine `next`-Adresse aus der Anfrage und kein Pflicht-Skript.

Beim Speichern lehnt `ensure_relations` eine UUID ab, die der Benutzer am registrierten Ziel nicht lesen darf. Die Antwort bleibt das Formular mit dem Fehler am Feld. Ein nicht registriertes Ziel bleibt der Datenbank-Fremdschlüsselprüfung überlassen.

Nach einem Validierungsfehler bleibt eine lesbare Auswahl im Formular, weil die gesendeten Feldwerte erneut gerendert werden.
