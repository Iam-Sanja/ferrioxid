//! Stabile Job-Fehler. Der angezeigte Code enthält keinen Payload und kein Geheimnis.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Retry,
    Permanent,
    Message,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code}")]
pub struct JobError {
    pub kind: FailureKind,
    pub code: String,
    pub detail: String,
}

impl JobError {
    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn retry(code: &str) -> Self {
        let code = owned_code(code);
        Self {
            kind: FailureKind::Retry,
            detail: code.clone(),
            code,
        }
    }

    pub fn permanent(code: &str) -> Self {
        let code = owned_code(code);
        Self {
            kind: FailureKind::Permanent,
            detail: code.clone(),
            code,
        }
    }

    pub fn message(code: &str, detail: impl Into<String>) -> Self {
        Self {
            kind: FailureKind::Message,
            code: owned_code(code),
            detail: held_detail(detail.into()),
        }
    }

    pub fn is_retry(&self) -> bool {
        self.kind == FailureKind::Retry
    }
}

fn owned_code(code: &str) -> String {
    if is_safe_code(code) {
        code.to_owned()
    } else {
        "job.error".to_owned()
    }
}

pub fn payload_line(payload: &str, show: bool) -> String {
    if show {
        format!("payload {payload}")
    } else {
        format!("payload [redigiert, {} Bytes]", payload.len())
    }
}

fn held_detail(detail: String) -> String {
    let lower = detail.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "password",
        "secret",
        "token",
        "authorization",
        "cookie",
        "reset_url",
    ];
    if MARKERS.iter().any(|marker| lower.contains(marker)) {
        "Die Meldung wurde zurückgehalten, weil sie ein Geheimnis enthalten könnte.".to_owned()
    } else {
        detail
    }
}

fn is_safe_code(code: &str) -> bool {
    let mut chars = code.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| {
            char.is_ascii_lowercase() || char.is_ascii_digit() || char == '.' || char == '_'
        })
        && code.len() <= 64
}
