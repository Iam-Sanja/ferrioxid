# Localization in the admin

[For overview](index.md) · Architecture: [i18n.md](i18n.md)]

## Formate

The HTML admin formats numbers in `display_cell`. German grouped with point, English with comma. Fields on `_cents` are shown with the project currency and two decimal places. The stored value remains the whole cent number. Date values from the database remain the output text. There is no `Intl` call in the server.

## Language of the homepage

`negotiate_locale` in `crates/ferrioxid-api/src/pages.rs` checks in this order:

1. `?lang=` only if the value is `de` or `en`, optionally with region such as `de-DE`.
2. The first supported entry of `Accept-Language`, weighted with `q`.
3. `project.default_locale`.
4. `project.fallback_locale`, sonst `en`.

The value is written in the Tera context as `locale`. It is not attached to a file path. Tera escaped the texts used. Generated projects link `/ ?lang=de` and `/?lang=en`.

## CLI

`FERRUM_LOCALE=en` or `de` overwrites `default_locale`. Translated are the messages for generated project, generated model and start and stop of the development server. `ferrioxid dev` outputs the line `Ferrioxid lauscht` of the generated program unchanged. Migration status, `Angewendet` and `Keine Schemaänderung` remain unchanged.

## Tests

`examples/myshop/tests/admin_html.rs` checks the German registration, `Accept-Language: en` and the language change in English. `crates/ferrioxid-api/src/pages.rs` checks the English fallback and an invalid locale. Rejected branding colors are checked by the registry in `ferrioxid-admin`.
