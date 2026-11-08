//! Opt-in local receipt for one document check.
//!
//! The file is written only when `--receipt` is set. It records the check
//! result, the tool version, and the change share. It does not contain source
//! text, and it is not sent anywhere.

use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::Path,
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

use rewrite_types::RewriteRecord;
use serde::Serialize;

use super::{replace, report::FidelityReport};
use crate::{
    contract::{CommandName, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode},
    failure::RunFailure,
};

/// Receipt contract version. Version 1 nests the rewrite record at schema 2.
pub(super) const SCHEMA_VERSION: u32 = 1;

/// Process limit copied from the provenance policy. It is not a product promise.
pub(super) const NOTICE: &str = "The report is evidence of Retonr's process. It is not an authorship certificate, legal opinion, publisher approval, or proof of compliance.";

/// Facts recorded beside the rewrite record.
pub(super) struct ReceiptFacts {
    pub(super) fidelity: FidelityReport,
    pub(super) changed_token_pct: u8,
    pub(super) changed_char_pct: u8,
    pub(super) created_at: SystemTime,
}

#[derive(Serialize)]
struct EditReceipt<'a> {
    schema_version: u32,
    notice: &'static str,
    product: &'static str,
    product_version: &'static str,
    commit: &'static str,
    created_at: String,
    record: &'a RewriteRecord,
    edit_level_effective: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_level_requested: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_level_note: Option<&'static str>,
    changed_token_pct: u8,
    changed_char_pct: u8,
}

/// Rejects a receipt path that is not a new file, or that the check also uses.
pub(super) fn validate(
    destination: &replace::Destination,
    receipt: Option<&Path>,
    trace: Option<&Path>,
    command: CommandName,
) -> Result<(), RunFailure> {
    let Some(receipt) = receipt else {
        return Ok(());
    };
    if super::is_standard_stream(receipt) {
        return Err(usage("a receipt must be a new file"));
    }
    replace::require_new_file_path(receipt, command)?;
    if trace.is_some_and(|trace| replace::same_planned_path(trace, receipt)) {
        return Err(usage("a receipt path must differ from the trace"));
    }
    if destination.reserves_path(receipt, command)? {
        return Err(usage("a receipt path must differ from the output"));
    }
    Ok(())
}

/// Directory checks report many pairs and do not write one document receipt.
pub(super) fn directory_rejected() -> RunFailure {
    usage("a directory check does not write a receipt")
}

/// Writes one receipt. The caller has already rejected an occupied path.
pub(super) fn write(
    path: &Path,
    record: &RewriteRecord,
    facts: &ReceiptFacts,
) -> Result<(), RunFailure> {
    let Some(created_at) = format_utc(facts.created_at) else {
        return Err(RunFailure::operational(CommandName::Check));
    };
    let receipt = EditReceipt {
        schema_version: SCHEMA_VERSION,
        notice: NOTICE,
        product: "retonr",
        product_version: env!("CARGO_PKG_VERSION"),
        commit: env!("RETONR_COMMIT"),
        created_at,
        record,
        edit_level_effective: facts.fidelity.edit_level_effective,
        edit_level_requested: facts.fidelity.edit_level_requested,
        edit_level_note: facts.fidelity.note,
        changed_token_pct: facts.changed_token_pct,
        changed_char_pct: facts.changed_char_pct,
    };
    let mut bytes = crate::render::to_safe_pretty_json(&receipt)
        .map_err(|_| RunFailure::operational(CommandName::Check))?;
    bytes.push(b'\n');
    write_new_file(path, &bytes)
}

fn usage(message: &'static str) -> RunFailure {
    RunFailure {
        command: CommandName::Check,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
        exit_code: ExitCode::from(EXIT_USAGE),
        message,
    }
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), RunFailure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                RunFailure::output_exists_for(CommandName::Check)
            } else {
                RunFailure::operational(CommandName::Check)
            }
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| RunFailure::operational(CommandName::Check))
}

pub(super) fn format_utc(time: SystemTime) -> Option<String> {
    let seconds = i64::try_from(time.duration_since(UNIX_EPOCH).ok()?.as_secs()).ok()?;
    let days = seconds.div_euclid(86_400);
    let tod = u32::try_from(seconds.rem_euclid(86_400)).ok()?;
    let (year, month, day) = civil_from_days(days)?;
    let hour = tod / 3_600;
    let minute = (tod % 3_600) / 60;
    let second = tod % 60;
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn civil_from_days(days: i64) -> Option<(i32, u32, u32)> {
    let z = days.checked_add(719_468)?;
    let era = z.div_euclid(146_097);
    let doe = u64::try_from(z.checked_sub(era.checked_mul(146_097)?)?).ok()?;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = i64::from(u32::try_from(yoe).ok()?).checked_add(era.checked_mul(400)?)?;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year = year.checked_add(1)?;
    }
    Some((
        i32::try_from(year).ok()?,
        u32::try_from(month).ok()?,
        u32::try_from(day).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::format_utc;

    #[test]
    fn utc_format_matches_known_instants() {
        let cases = [
            (0, "1970-01-01T00:00:00Z"),
            (86_400, "1970-01-02T00:00:00Z"),
            (1_582_934_400, "2020-02-29T00:00:00Z"),
            (1_609_459_200, "2021-01-01T00:00:00Z"),
            (1_700_000_000, "2023-11-14T22:13:20Z"),
        ];
        for (seconds, expected) in cases {
            let time = UNIX_EPOCH + Duration::from_secs(seconds);
            assert_eq!(format_utc(time).as_deref(), Some(expected));
        }
    }
}
