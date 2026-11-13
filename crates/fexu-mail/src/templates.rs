//! Tera-Vorlagen für Konto-Nachrichten. HTML wird escaped. Text bleibt roh.

use std::sync::OnceLock;
use tera::{Context, Tera};

static TEMPLATES: OnceLock<Tera> = OnceLock::new();

fn engine() -> &'static Tera {
    TEMPLATES.get_or_init(|| {
        let mut tera = Tera::default();
        tera.autoescape_on(vec![".html"]);
        tera.add_raw_templates([
            ("password_reset.text", "Jemand hat das Zurücksetzen des Passworts für dieses Konto angefordert.\nDer Link ist {{ minutes }} Minuten gültig:\n{{ url }}\nWenn du das nicht warst, kannst du diese Nachricht ignorieren.\n"),
            ("password_reset_en.text", "Someone asked to reset the password for this account.\nThe link is valid for {{ minutes }} minutes:\n{{ url }}\nIf this was not you, ignore this message.\n"),
            ("password_reset.html", "<p>Jemand hat das Zurücksetzen des Passworts für dieses Konto angefordert.</p><p>Der Link ist {{ minutes }} Minuten gültig.</p><p><a href=\"{{ url }}\">Passwort zurücksetzen</a></p><p>Wenn du das nicht warst, kannst du diese Nachricht ignorieren.</p>"),
            ("password_reset_en.html", "<p>Someone asked to reset the password for this account.</p><p>The link is valid for {{ minutes }} minutes.</p><p><a href=\"{{ url }}\">Reset password</a></p><p>If this was not you, ignore this message.</p>"),
            ("password_changed.text", "Das Passwort dieses Kontos wurde geändert. Bestehende Sitzungen sind nicht mehr gültig.\nWenn du das nicht warst, setze das Passwort erneut zurück.\n"),
            ("password_changed_en.text", "The password for this account was changed. Existing sessions are no longer valid.\nIf this was not you, reset the password again.\n"),
            ("password_changed.html", "<p>Das Passwort dieses Kontos wurde geändert. Bestehende Sitzungen sind nicht mehr gültig.</p><p>Wenn du das nicht warst, setze das Passwort erneut zurück.</p>"),
            ("password_changed_en.html", "<p>The password for this account was changed. Existing sessions are no longer valid.</p><p>If this was not you, reset the password again.</p>"),
            ("account_deactivated.text", "Dieses Konto wurde deaktiviert. Eine Anmeldung ist damit nicht mehr möglich.\n"),
            ("account_deactivated_en.text", "This account was deactivated. Signing in is no longer possible.\n"),
            ("account_deactivated.html", "<p>Dieses Konto wurde deaktiviert. Eine Anmeldung ist damit nicht mehr möglich.</p>"),
            ("account_deactivated_en.html", "<p>This account was deactivated. Signing in is no longer possible.</p>"),
            ("security_notice.text", "{{ summary }}\n"),
            ("security_notice_en.text", "{{ summary }}\n"),
            ("security_notice.html", "<p>{{ summary }}</p>"),
            ("security_notice_en.html", "<p>{{ summary }}</p>"),
        ])
        .expect("mail templates");
        tera
    })
}

pub fn render(
    stem: &str,
    extension: &str,
    locale: &str,
    context: &Context,
) -> Result<String, tera::Error> {
    let suffix = if locale == "en" { "_en" } else { "" };
    engine().render(&format!("{stem}{suffix}.{extension}"), context)
}

pub fn subject(kind: &str, locale: &str) -> &'static str {
    let english = locale == "en";
    match kind {
        "password_reset" if english => "Reset your password",
        "password_reset" => "Passwort zurücksetzen",
        "password_changed" if english => "Password changed",
        "password_changed" => "Passwort geändert",
        "account_deactivated" if english => "Account deactivated",
        "account_deactivated" => "Konto deaktiviert",
        "security_notice" if english => "Security notice",
        _ => "Sicherheitshinweis",
    }
}
