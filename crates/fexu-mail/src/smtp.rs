//! Ein SMTP-Versuch mit geprüfter TLS-Verbindung. Kein Klartext-Fallback und kein Retry.

use crate::error::MailError;
use crate::message::{validate, MailMessage};
use lettre::message::{header::ContentType, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::time::Duration;

#[derive(Clone)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub security: SmtpSecurity,
    pub username: String,
    pub password: String,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpSecurity {
    Starttls,
    Tls,
}

impl std::fmt::Debug for SmtpSettings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SmtpSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("security", &self.security)
            .field("username", &"[redacted]")
            .field("password", &"[redacted]")
            .field("timeout", &self.timeout)
            .finish()
    }
}

pub async fn deliver(settings: &SmtpSettings, message: &MailMessage) -> Result<(), MailError> {
    validate(message)?;
    let email = build(message)?;
    let transport = transport(settings)?;
    match tokio::time::timeout(settings.timeout, transport.send(email)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(_)) => {
            tracing::warn!(code = "mail.rejected", "smtp delivery was rejected");
            Err(MailError::Rejected)
        }
        Err(_) => {
            tracing::warn!(code = "mail.timeout", "smtp delivery timed out");
            Err(MailError::Timeout)
        }
    }
}

fn transport(settings: &SmtpSettings) -> Result<AsyncSmtpTransport<Tokio1Executor>, MailError> {
    let builder = match settings.security {
        SmtpSecurity::Starttls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.host)
                .map_err(|_| MailError::Unavailable)?
        }
        SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.host)
            .map_err(|_| MailError::Unavailable)?,
    };
    Ok(builder
        .port(settings.port)
        .credentials(Credentials::new(
            settings.username.clone(),
            settings.password.clone(),
        ))
        .timeout(Some(settings.timeout))
        .build())
}

fn build(message: &MailMessage) -> Result<Message, MailError> {
    let mut email = Message::builder()
        .from(mailbox(&message.from)?)
        .to(mailbox(&message.to)?)
        .subject(&message.subject);
    if let Some(reply) = &message.reply_to {
        email = email.reply_to(mailbox(reply)?);
    }
    match &message.html {
        Some(html) => email
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(message.text.clone()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html.clone()),
                    ),
            )
            .map_err(|_| MailError::Invalid),
        None => email
            .header(ContentType::TEXT_PLAIN)
            .body(message.text.clone())
            .map_err(|_| MailError::Invalid),
    }
}

fn mailbox(address: &str) -> Result<lettre::message::Mailbox, MailError> {
    format!("<{address}>")
        .parse()
        .map_err(|_| MailError::Invalid)
}
