//! Eine Nachricht. Adressen und Betreff werden vor dem Versand geprüft.

use crate::error::MailError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailMessage {
    pub from: String,
    pub to: String,
    pub reply_to: Option<String>,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailReceipt {
    pub backend: &'static str,
}

pub fn validate(message: &MailMessage) -> Result<(), MailError> {
    check_address(&message.from)?;
    check_address(&message.to)?;
    if let Some(reply) = &message.reply_to {
        check_address(reply)?;
    }
    check_header(&message.subject, 120)?;
    if message.subject.trim().is_empty() || message.text.trim().is_empty() {
        return Err(MailError::Invalid);
    }
    if message.text.len() > 20_000 {
        return Err(MailError::Invalid);
    }
    if message
        .html
        .as_ref()
        .is_some_and(|html| html.len() > 40_000)
    {
        return Err(MailError::Invalid);
    }
    Ok(())
}

pub fn check_address(value: &str) -> Result<(), MailError> {
    check_header(value, 254)?;
    let Some((local, domain)) = value.split_once('@') else {
        return Err(MailError::Invalid);
    };
    if local.is_empty()
        || domain.len() < 3
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return Err(MailError::Invalid);
    }
    Ok(())
}

pub fn check_header(value: &str, max: usize) -> Result<(), MailError> {
    if value.is_empty()
        || value.len() > max
        || value
            .chars()
            .any(|char| char.is_control() || char == '\r' || char == '\n')
    {
        return Err(MailError::Invalid);
    }
    Ok(())
}
