//! Operatorausgabe. Das Lease-Token bleibt immer verborgen.

use crate::error::payload_line;
use crate::queue::JobRecord;

pub fn describe_job(job: &JobRecord, show_payload: bool) -> String {
    format!(
        "id {id}\nhandler {handler}\nversion {version}\nstatus {status}\nattempts {attempts}/{max}\ncreated_at {created}\navailable_at {available}\nnext_attempt_at {next}\nlease_owner {owner}\nlease_expires_at {expires}\nerror_code {error}\nfinished_at {finished}\n{payload}",
        id = job.id,
        handler = job.handler,
        version = job.payload_version,
        status = job.status.as_str(),
        attempts = job.attempts,
        max = job.max_attempts,
        created = job.created_at,
        available = job.available_at,
        next = optional(job.next_attempt_at),
        owner = job.lease_owner.clone().unwrap_or_else(|| "-".to_owned()),
        expires = optional(job.lease_expires_at),
        error = job.error_code.clone().unwrap_or_else(|| "-".to_owned()),
        finished = optional(job.finished_at),
        payload = payload_line(&job.payload, show_payload),
    )
}

fn optional(value: Option<i64>) -> String {
    value
        .map(|item| item.to_string())
        .unwrap_or_else(|| "-".to_owned())
}
