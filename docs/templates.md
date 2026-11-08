# Vorlagen

[For overview](index.md) · **IST files:** `crates/ferrioxid-api/src/pages.rs`, `crates/ferrioxid-cli/src/scaffold.rs`.]

## Aufgabe

Normal pages outside the admin are rendered on the server side with Tera. The admin templates are separate in `crates/ferrioxid-admin/templates` and are not loaded from the project folder. See [admin-templates.md](admin-templates.md)]. Tera corresponds to the Jinja and Django model: inheritance via `extends`, building blocks via `include`, variables in `{{ }}`. Form HTML from `ferrioxid::forms::render_form` is additionally available and escaped field values. The contact side of the generator and myshop uses this path. The style sheet is under `/static/css/app.css`.

## Responsible files

- `crates/ferrioxid-api/src/pages.rs`
- `crates/ferrioxid/src/lib.rs`
- `templates/` in the generated project

## Public interface

`ferrioxid::pages::Site::open` loads `**/*.html`. `render` creates a page. `home_context` fills project name, language, admin flag and the short standard texts. `mount_pages` depends on `/` and a 404 response to the router if `templates.enabled` is true. For each request, `negotiate_locale` selects the language. `message` provides the prepared texts for `de` and `en`, including Welcome, 404 and 500. An unknown locale falls back in English and is not used as a path.

## Konfiguration

```toml
[templates]
enabled = true
engine = "tera"
templates_dir = "templates"
```

## Datenfluss

The application loads the settings, opens the template directory and renders again every time it is called. A missing start page prevents the start. If the rendering fails, `errors/500.html` is attempted.

## Error cases

| Fall | Verhalten |
|---|---|
| `index.html` fehlt |Start with error message|
| unbekannte Engine |`ferrum.toml` is rejected|
| unbekannte Route | 404 aus `errors/404.html` |

## Sicherheitsregeln

`primary_color` must only be a hex value so that it can be inserted into the CSS of the template. Template paths remain relative and without `..`.

## Example

```html
{% extends "base.html" %}
{% block content %}
<h1>{{ project_name }}</h1>
<p>{{ running }}</p>
{% endblock %}
```

## Tests

`index_template_renders_the_project_name` renders a temporary project. The generator applies `templates/base.html`, `index.html` and `errors/`. A generated project must translate with `cargo check`.

## Limitations

Panics do not automatically become the 500 page. `ferrioxid dev` reloads templates at process start, not every time you save without restart. Changing a Rust file restarts the process and reloads the templates. `include` for partials is possible, but the generator does not yet create its own partial file. The basic template is the common layout.
