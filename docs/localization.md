# Localization in the admin

[Overview](index.md) · Architecture: [i18n.md](i18n.md)

## Formats

The HTML admin formats numbers in `display_cell`: German uses periods for digit grouping, English uses commas. Fields ending in `_cents` use the project currency and two decimal places; stored values remain integer cents. Date values retain their database representation. The server does not call `Intl`.

## Language of the homepage

`negotiate_locale` in `crates/ferrioxid-api/src/pages.rs` checks in this order:

1. `?lang=` only if the value is `de` or `en`, optionally with region such as `de-DE`.
2. The first supported entry of `Accept-Language`, weighted with `q`.
3. `project.default_locale`.
4. `project.fallback_locale`, otherwise `en`.

The selected language is added to the Tera context as `locale`; it never becomes a file path. Tera escapes rendered text. Generated projects link to `/?lang=de` and `/?lang=en`.

## CLI

`FERRUM_LOCALE=en` or `de` overrides `default_locale` for the CLI. Project-generation, model-generation and development-server start/stop messages are localized. `ferrioxid dev` passes through the generated program’s `Ferrioxid lauscht` line unchanged. Migration status messages, including `Angewendet` and `Keine Schemaänderung`, remain unchanged.

## Tests

`examples/myshop/tests/admin_html.rs` checks the German login page, `Accept-Language: en` and switching to English. Tests in `crates/ferrioxid-api/src/pages.rs` check English fallback and invalid locales. The admin registry tests reject invalid branding colors.
