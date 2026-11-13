# Templates

[Overview](index.md) · **Implemented files:** `crates/fexu-api/src/pages.rs`, `crates/fexu-cli/src/scaffold.rs`.

## Purpose

Normal pages outside the admin are rendered on the server side with Tera. The admin templates are separate in `crates/fexu-admin/templates` and are not loaded from the project folder. See [admin-templates.md](admin-templates.md). Tera corresponds to the Jinja and Django model: inheritance via `extends`, building blocks via `include`, variables in `{{ }}`. Form HTML from `fexu::forms::render_form` is additionally available and escaped field values. The contact side of the generator and myshop uses this path. The style sheet is under `/static/css/app.css`.

## Responsible files

- `crates/fexu-api/src/pages.rs`
- `crates/fexu/src/lib.rs`
- `templates/` in the generated project

## Public interface

`fexu::pages::Site::open` loads `**/*.html`, and `render` renders a template. `home_context` supplies the project name, locale, admin flag and standard text. When `templates.enabled` is true, `mount_pages` adds `/`, an HTML 404 fallback and a 405 fallback. `mount_pages_with` also registers `url()` on those pages. `negotiate_locale` selects the language for each request. `message` provides German and English text, including welcome, 404 and 500 messages. Unknown locales fall back to English and never become file paths.

## Configuration

```toml
[templates]
enabled = true
engine = "tera"
templates_dir = "templates"
```

## Data flow

The application loads the settings, opens the template directory and renders again every time it is called. A missing start page prevents the start. If the rendering fails, `errors/500.html` is attempted.

## Error cases

| Case | Behavior |
|---|---|
| Missing `index.html` | Startup fails with an error message |
| Unknown template engine | Reject `fexu.toml` |
| Unknown route | 404 rendered from `errors/404.html` |

## Security rules

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

`index_template_renders_the_project_name` renders a temporary project. The generator writes `templates/base.html`, `index.html` and `errors/`. Generated projects must compile with `cargo check`.

## Shared app templates

An app publishes templates with `Registration::export_templates`. A dependent app calls `Registration::import_templates(source)` before its own `templates()` call. The source must be listed in `depends_on` and must already have exported. Imported names stay under `{source}/`. `admin/`, `base.html` and `login.html` are rejected. Two apps may both have `page.html` because the names are `shop/page.html` and `cards/page.html`. A template that imports a file the source did not export fails when that app's catalog is built, with the missing template name in the error. Project `templates/base.html` is unchanged.

## Limitations

Panics do not automatically become the 500 page. `fexu dev` reloads templates at process start, not every time you save without restart. Changing a Rust file restarts the process and reloads the templates. `include` for partials is possible, but the generator does not yet create its own partial file. The basic template is the common layout.
