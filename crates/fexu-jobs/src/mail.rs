//! Gewöhnliche Hinweise als Jobs. Ein Passwort-Reset-Token wird hier nicht angenommen.

use crate::error::JobError;
use crate::queue::{EnqueueOutcome, Queue, When};
use crate::registry::{JobContext, JobHandler};
use async_trait::async_trait;
use fexu_mail::{MailError, MailService, Notice};
use serde::{Deserialize, Serialize};

pub const MAIL_HANDLER: &str = "fexu_mail_notice";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MailNotice {
    pub kind: String,
    pub to: String,
    pub locale: String,
    #[serde(default)]
    pub summary: String,
}

pub struct MailNoticeHandler {
    mail: MailService,
}

impl MailNoticeHandler {
    pub fn new(mail: MailService) -> Self {
        Self { mail }
    }
}

#[async_trait]
impl JobHandler for MailNoticeHandler {
    const NAME: &'static str = MAIL_HANDLER;
    const VERSION: u32 = 1;
    type Payload = MailNotice;

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(20)
    }

    async fn run(&self, payload: Self::Payload, _ctx: &JobContext) -> Result<(), JobError> {
        let notice = notice_from_payload(&payload)?;
        match self.mail.notify(notice).await {
            Ok(receipt) => {
                tracing::info!(
                    backend = receipt.backend,
                    code = "mail.accepted",
                    "mail backend accepted the notice"
                );
                Ok(())
            }
            Err(MailError::Timeout) => Err(JobError::retry("mail.timeout")),
            Err(MailError::Unavailable) => Err(JobError::retry("mail.unavailable")),
            Err(MailError::Disabled) => Err(JobError::permanent("mail.disabled")),
            Err(MailError::Invalid) => Err(JobError::permanent("mail.invalid")),
            Err(MailError::Rejected) => Err(JobError::permanent("mail.rejected")),
        }
    }
}

pub async fn enqueue_notice(
    queue: &Queue,
    notice: &Notice,
    when: When,
) -> Result<EnqueueOutcome, JobError> {
    let payload = match notice {
        Notice::PasswordResetRequested { .. } => {
            return Err(JobError::permanent("job.mail.reset_refused"));
        }
        Notice::PasswordChanged { to, locale } => MailNotice {
            kind: "password_changed".to_owned(),
            to: to.clone(),
            locale: locale.clone(),
            summary: String::new(),
        },
        Notice::AccountDeactivated { to, locale } => MailNotice {
            kind: "account_deactivated".to_owned(),
            to: to.clone(),
            locale: locale.clone(),
            summary: String::new(),
        },
        Notice::SecurityNotice {
            to,
            locale,
            summary,
        } => MailNotice {
            kind: "security_notice".to_owned(),
            to: to.clone(),
            locale: locale.clone(),
            summary: summary.clone(),
        },
    };
    queue.enqueue_json(MAIL_HANDLER, 1, &payload, when).await
}

fn notice_from_payload(payload: &MailNotice) -> Result<Notice, JobError> {
    if !fexu_core::is_email(&payload.to) {
        return Err(JobError::permanent("mail.invalid"));
    }
    let locale = if payload.locale == "en" {
        "en"
    } else if payload.locale == "de" {
        "de"
    } else {
        return Err(JobError::permanent("job.mail.kind"));
    };
    if payload.summary.contains(['\r', '\n']) || payload.summary.chars().count() > 200 {
        return Err(JobError::permanent("mail.invalid"));
    }
    match payload.kind.as_str() {
        "password_reset" => Err(JobError::permanent("job.mail.reset_refused")),
        "password_changed" => Ok(Notice::PasswordChanged {
            to: payload.to.clone(),
            locale: locale.to_owned(),
        }),
        "account_deactivated" => Ok(Notice::AccountDeactivated {
            to: payload.to.clone(),
            locale: locale.to_owned(),
        }),
        "security_notice" if !payload.summary.is_empty() => Ok(Notice::SecurityNotice {
            to: payload.to.clone(),
            locale: locale.to_owned(),
            summary: payload.summary.clone(),
        }),
        _ => Err(JobError::permanent("job.mail.kind")),
    }
}
