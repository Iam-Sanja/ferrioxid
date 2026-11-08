//! E-Mail-Versand für Ferrioxid. Tests nutzen das Memory-Backend und öffnen keine SMTP-Verbindung.

mod error;
mod message;
mod notify;
mod service;
mod smtp;
mod templates;

pub use error::MailError;
pub use message::{MailMessage, MailReceipt};
pub use notify::Notice;
pub use service::{MailConfig, MailService, MemoryInbox};
pub use smtp::{SmtpSecurity, SmtpSettings};

#[cfg(test)]
mod tests {
    use super::{MailError, MailMessage, MailService, Notice};

    #[tokio::test]
    async fn memory_backend_keeps_the_message_without_a_network() {
        let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
        let receipt = mail
            .notify(Notice::PasswordResetRequested {
                to: "person@example.com".into(),
                locale: "de".into(),
                reset_url: "http://127.0.0.1:3000/auth/password-reset/confirm?token=abc".into(),
                minutes: 30,
            })
            .await
            .expect("stored");
        assert_eq!(receipt.backend, "memory");
        let stored = inbox.messages();
        assert_eq!(stored.len(), 1);
        assert!(stored[0].text.contains("abc"));
        assert!(
            stored[0].html.as_deref().unwrap().contains("&amp;")
                || stored[0].html.as_deref().unwrap().contains("abc")
        );
        assert!(!format!("{receipt:?}").contains("abc"));
    }

    #[tokio::test]
    async fn header_injection_and_disabled_backend_fail_closed() {
        let mail = MailService::disabled();
        let error = mail
            .send(MailMessage {
                from: "noreply@example.com".into(),
                to: "person@example.com".into(),
                reply_to: None,
                subject: "Reset\r\nBcc: evil@example.com".into(),
                text: "body".into(),
                html: None,
            })
            .await
            .expect_err("rejected");
        assert_eq!(error, MailError::Invalid);
        let disabled = mail
            .notify(Notice::PasswordChanged {
                to: "person@example.com".into(),
                locale: "en".into(),
            })
            .await
            .expect_err("disabled");
        assert_eq!(disabled, MailError::Disabled);
        assert!(!disabled.to_string().contains("password"));
    }

    #[test]
    fn smtp_debug_redacts_credentials() {
        let settings = super::SmtpSettings {
            host: "smtp.example.com".into(),
            port: 587,
            security: super::SmtpSecurity::Starttls,
            username: "mailer".into(),
            password: "super-secret-value".into(),
            timeout: std::time::Duration::from_secs(5),
        };
        let rendered = format!("{settings:?}");
        assert!(rendered.contains("smtp.example.com"));
        assert!(!rendered.contains("super-secret-value"));
        assert!(!rendered.contains("mailer"));
    }
}
