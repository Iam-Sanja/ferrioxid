//! Typisierte Konto-Hinweise. Der Name kommt nicht aus einem HTTP-Parameter.

use crate::error::MailError;
use crate::message::{check_header, MailMessage, MailReceipt};
use crate::service::MailService;
use crate::templates;
use tera::Context;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    PasswordResetRequested {
        to: String,
        locale: String,
        reset_url: String,
        minutes: u32,
    },
    PasswordChanged {
        to: String,
        locale: String,
    },
    AccountDeactivated {
        to: String,
        locale: String,
    },
    SecurityNotice {
        to: String,
        locale: String,
        summary: String,
    },
}

impl Notice {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::PasswordResetRequested { .. } => "password_reset",
            Self::PasswordChanged { .. } => "password_changed",
            Self::AccountDeactivated { .. } => "account_deactivated",
            Self::SecurityNotice { .. } => "security_notice",
        }
    }

    fn locale(&self) -> &str {
        match self {
            Self::PasswordResetRequested { locale, .. }
            | Self::PasswordChanged { locale, .. }
            | Self::AccountDeactivated { locale, .. }
            | Self::SecurityNotice { locale, .. } => locale,
        }
    }

    fn to(&self) -> &str {
        match self {
            Self::PasswordResetRequested { to, .. }
            | Self::PasswordChanged { to, .. }
            | Self::AccountDeactivated { to, .. }
            | Self::SecurityNotice { to, .. } => to,
        }
    }
}

impl MailService {
    pub async fn notify(&self, notice: Notice) -> Result<MailReceipt, MailError> {
        let locale = if notice.locale() == "en" { "en" } else { "de" };
        let mut context = Context::new();
        match &notice {
            Notice::PasswordResetRequested {
                reset_url, minutes, ..
            } => {
                check_header(reset_url, 2048)?;
                context.insert("url", reset_url);
                context.insert("minutes", minutes);
            }
            Notice::SecurityNotice { summary, .. } => {
                check_header(summary, 200)?;
                context.insert("summary", summary);
            }
            Notice::PasswordChanged { .. } | Notice::AccountDeactivated { .. } => {}
        }
        let kind = notice.kind();
        let text =
            templates::render(kind, "text", locale, &context).map_err(|_| MailError::Invalid)?;
        let html =
            templates::render(kind, "html", locale, &context).map_err(|_| MailError::Invalid)?;
        let message = MailMessage {
            from: self.sender().to_owned(),
            to: notice.to().to_owned(),
            reply_to: self.reply_to().map(str::to_owned),
            subject: templates::subject(kind, locale).to_owned(),
            text,
            html: Some(html),
        };
        self.send(message).await
    }
}
