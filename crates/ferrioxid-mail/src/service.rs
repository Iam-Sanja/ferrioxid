//! Versandstrategie. Der Dienst schreibt den Nachrichtentext nicht ins Tracing.

use crate::error::MailError;
use crate::message::{validate, MailMessage, MailReceipt};
use crate::smtp::{self, SmtpSecurity, SmtpSettings};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Vollständige Mail-Form aus Konfiguration und Umgebungsvariablen.
#[derive(Clone, Debug)]
pub struct MailConfig {
    pub backend: String,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub from: String,
    pub reply_to: String,
    pub timeout_secs: u64,
    pub public_origin: String,
    pub reset_ttl_minutes: u32,
}

#[derive(Clone)]
pub struct MailService {
    transport: Arc<Transport>,
    from: String,
    reply_to: Option<String>,
    public_origin: String,
    reset_ttl_secs: i64,
}

enum Transport {
    Disabled,
    Console,
    Memory(MemoryInbox),
    Smtp(SmtpSettings),
}

#[derive(Clone)]
pub struct MemoryInbox {
    messages: Arc<Mutex<Vec<MailMessage>>>,
}

impl MemoryInbox {
    pub fn messages(&self) -> Vec<MailMessage> {
        self.messages
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl MailService {
    pub fn disabled() -> Self {
        Self::new(
            Transport::Disabled,
            "noreply@example.invalid".into(),
            None,
            String::new(),
            30 * 60,
        )
    }

    pub fn console(from: impl Into<String>, public_origin: impl Into<String>) -> Self {
        Self::new(
            Transport::Console,
            from.into(),
            None,
            public_origin.into(),
            30 * 60,
        )
    }

    pub fn memory(
        from: impl Into<String>,
        public_origin: impl Into<String>,
    ) -> (Self, MemoryInbox) {
        let inbox = MemoryInbox {
            messages: Arc::new(Mutex::new(Vec::new())),
        };
        (
            Self::new(
                Transport::Memory(inbox.clone()),
                from.into(),
                None,
                public_origin.into(),
                30 * 60,
            ),
            inbox,
        )
    }

    pub fn smtp(
        settings: SmtpSettings,
        from: impl Into<String>,
        reply_to: Option<String>,
        public_origin: impl Into<String>,
        reset_ttl_secs: i64,
    ) -> Self {
        Self::new(
            Transport::Smtp(settings),
            from.into(),
            reply_to,
            public_origin.into(),
            reset_ttl_secs,
        )
    }

    /// `ferrioxid.toml` liefert die Form. Das SMTP-Passwort kommt nur aus `FERRIOXID_SMTP_PASSWORD`.
    pub fn open(config: &MailConfig) -> Self {
        let backend =
            std::env::var("FERRIOXID_MAIL_BACKEND").unwrap_or_else(|_| config.backend.clone());
        let from = env_or("FERRIOXID_MAIL_FROM", &config.from);
        let reply = env_or("FERRIOXID_MAIL_REPLY_TO", &config.reply_to);
        let origin = env_or("FERRIOXID_PUBLIC_ORIGIN", &config.public_origin);
        let minutes =
            env_u32("FERRIOXID_RESET_TTL_MINUTES", config.reset_ttl_minutes).clamp(5, 1440);
        let reply_to = if reply.is_empty() { None } else { Some(reply) };
        let production = std::env::var("FERRIOXID_ENV").ok().as_deref() == Some("production");
        let service = match backend.as_str() {
            "console" if !production => {
                Self::console(from, origin).with_ttl(i64::from(minutes) * 60)
            }
            "memory" if !production => Self::memory(from, origin)
                .0
                .with_ttl(i64::from(minutes) * 60),
            "smtp" => {
                let host = env_or("FERRIOXID_SMTP_HOST", &config.host);
                let port = env_u16("FERRIOXID_SMTP_PORT", config.port);
                let security = env_or("FERRIOXID_SMTP_SECURITY", &config.security);
                let timeout =
                    env_u64("FERRIOXID_SMTP_TIMEOUT_SECS", config.timeout_secs).clamp(1, 30);
                let username = std::env::var("FERRIOXID_SMTP_USERNAME").unwrap_or_default();
                let password = std::env::var("FERRIOXID_SMTP_PASSWORD").unwrap_or_default();
                let mode = match security.as_str() {
                    "starttls" => Some(SmtpSecurity::Starttls),
                    "tls" => Some(SmtpSecurity::Tls),
                    _ => None,
                };
                match mode {
                    Some(mode)
                        if !host.is_empty()
                            && port > 0
                            && !from.is_empty()
                            && !username.is_empty()
                            && !password.is_empty()
                            && !origin.is_empty() =>
                    {
                        Self::smtp(
                            SmtpSettings {
                                host,
                                port,
                                security: mode,
                                username,
                                password,
                                timeout: Duration::from_secs(timeout),
                            },
                            from,
                            reply_to.clone(),
                            origin,
                            i64::from(minutes) * 60,
                        )
                    }
                    _ => {
                        tracing::warn!(
                            code = "mail.invalid",
                            "smtp mail is not active because host, security, sender, origin or credentials are incomplete"
                        );
                        Self::disabled()
                    }
                }
            }
            _ => {
                if production && matches!(backend.as_str(), "console" | "memory") {
                    tracing::warn!(
                        code = "mail.disabled",
                        "console and memory mail are refused in production"
                    );
                }
                Self::disabled()
            }
        };
        if let Some(reply_to) = reply_to {
            service.with_reply_to(reply_to)
        } else {
            service
        }
    }

    pub async fn send(&self, message: MailMessage) -> Result<MailReceipt, MailError> {
        validate(&message)?;
        match self.transport.as_ref() {
            Transport::Disabled => Err(MailError::Disabled),
            Transport::Console => {
                eprintln!(
                    "----- ferrioxid mail console -----\nTo: {}\nSubject: {}\n\n{}\n----- end -----",
                    message.to, message.subject, message.text
                );
                tracing::info!(
                    backend = "console",
                    "mail accepted by the console transport"
                );
                Ok(MailReceipt { backend: "console" })
            }
            Transport::Memory(inbox) => {
                inbox
                    .messages
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(message);
                tracing::info!(backend = "memory", "mail stored in memory");
                Ok(MailReceipt { backend: "memory" })
            }
            Transport::Smtp(settings) => {
                smtp::deliver(settings, &message).await?;
                tracing::info!(backend = "smtp", "smtp accepted the message");
                Ok(MailReceipt { backend: "smtp" })
            }
        }
    }

    pub fn backend_name(&self) -> &'static str {
        match self.transport.as_ref() {
            Transport::Disabled => "disabled",
            Transport::Console => "console",
            Transport::Memory(_) => "memory",
            Transport::Smtp(_) => "smtp",
        }
    }

    pub fn can_send(&self) -> bool {
        !matches!(self.transport.as_ref(), Transport::Disabled)
    }

    pub fn sender(&self) -> &str {
        &self.from
    }

    pub fn reply_to(&self) -> Option<&str> {
        self.reply_to.as_deref()
    }

    pub fn public_origin(&self) -> &str {
        &self.public_origin
    }

    pub fn reset_ttl_secs(&self) -> i64 {
        self.reset_ttl_secs
    }

    fn with_ttl(mut self, seconds: i64) -> Self {
        self.reset_ttl_secs = seconds;
        self
    }

    fn with_reply_to(mut self, reply_to: String) -> Self {
        self.reply_to = Some(reply_to);
        self
    }

    fn new(
        transport: Transport,
        from: String,
        reply_to: Option<String>,
        public_origin: String,
        reset_ttl_secs: i64,
    ) -> Self {
        Self {
            transport: Arc::new(transport),
            from,
            reply_to,
            public_origin,
            reset_ttl_secs,
        }
    }
}

fn env_or(name: &str, fallback: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn env_u16(name: &str, fallback: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn env_u32(name: &str, fallback: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn env_u64(name: &str, fallback: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}
