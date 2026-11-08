# Formulare

**Stand:** v0.19, IST. Die Cargo-Version bleibt `0.1.0`. `required_date`, `optional_date`, `required_time`, `optional_time`, `required_datetime_utc` und `required_datetime_local` prüfen die kanonischen Zeichenketten. Das lokale DateTime-Feld gilt als UTC.

## Verantwortlichkeit

`ferrioxid-forms` prüft URL-kodierte Eingaben und liefert ein typisiertes Rust-Objekt oder Feldfehler. Formulare sind nicht an eine Datenbanktabelle gebunden. Kontakt, Suche und Filter können damit beschrieben werden, ohne ein `FerrioxidModel` anzulegen.

## Crates und Dateien

- `crates/ferrioxid-forms/src/lib.rs`
- Gemeinsame Regeln: `crates/ferrioxid-core/src/validation.rs`
- Beispiel: `examples/myshop/src/contact.rs`
- Generator: `crates/ferrioxid-cli/src/scaffold.rs`, Funktion `contact_handlers`

Die Fassade exportiert das Modul als `ferrioxid::forms`.

## Öffentliche API

`FerrioxidForm::from_form` nimmt `FormValues` entgegen. `ContactForm` ist die mitgelieferte Referenz. Hilfen heißen `required_text`, `optional_text`, `required_i64`, `optional_i64`, `required_bool`, `optional_bool`, `required_uuid` und `optional_uuid`. `render_form` und `render_page` erzeugen HTML. `message` liefert deutsche und englische Texte zu stabilen Codes. `FormValues::parse_urlencoded` begrenzt den Body auf 65536 Byte und lehnt unbekannte Felder über `reject_unknown` ab. `csrf_token` ist erlaubt und kein Datenfeld.

Ein `#[derive(FerrioxidForm)]` ist nicht implementiert. Die Trait-Form ist die kompilierbare API.

## Datenfluss

Der Handler liest den Body, baut `FormValues` und ruft `from_form` auf. Gültige Werte sind typisiert. Fehler bleiben Codes pro Feld. `render_form` escaped Namen, Werte und Meldungen. Die Erfolgsseite des Kontaktbeispiels speichert die Nachricht nicht und versendet keine E-Mail.

## Konfiguration

Formulare haben keinen eigenen `ferrum.toml`-Abschnitt. Die maximale URL-kodierte Größe steht in `FormValues::parse_urlencoded`. Größere Requests werden vorher von der Body-Limit-Middleware abgelehnt, wenn `ferrioxid::web::prepare` oder `ferrioxid::middleware::apply` verwendet wird.

## Sicherheit

HTML wird escaped. Unbekannte Felder ergeben `validation.unknown_field` und werden nicht still übernommen. Die Prüfung läuft auf dem Server. Eine Sitzung, die ein CSRF-Cookie mitschickt, muss dasselbe HMAC-Token wie `x-csrf-token` im Feld `csrf_token` senden. Ohne Sitzung bleibt ein öffentliches Formular ohne zweiten CSRF-Mechanismus. Das Token kommt aus `AuthService::csrf_from_request`.

## Fehlerfälle

`validation.required`, `validation.min_length`, `validation.max_length`, `validation.min_value`, `validation.max_value`, `validation.email`, `validation.url`, `validation.invalid_type`, `validation.uuid`, `validation.unknown_field`, `validation.mismatch`, `form.csrf`. `ContactForm` setzt `validation.mismatch`, wenn der Name dem lokalen Teil der E-Mail entspricht. Unbekannte Codes ergeben in `message` den Text `missing message`.

## Beispiel

```rust
let values = ferrioxid::forms::FormValues::parse_urlencoded(&body)?;
let form = ferrioxid::forms::ContactForm::from_form(&values)?;
let _checked = form.name;
```

## Tests

`cargo test -p ferrioxid-forms`. Abgedeckt sind Pflichtfelder, Länge, E-Mail, URL, Zahl, UUID, optionale Felder, Kreuzvalidierung, unbekannte Felder und Escaping.

## Einschränkungen

Es gibt kein Formset, keine Dateifelder in der Form-API und kein Proc-Macro. Das Modellmakro ruft den gemeinsamen Kern noch nicht auf.

## Erweiterung

Ein späteres Derive kann dieselben Funktionen aus `ferrioxid-core` aufrufen. Datei- und Formset-Felder bleiben geplant.
