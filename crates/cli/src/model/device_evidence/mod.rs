//! Optional fitr device-measurement evidence. It is not a qualification.

use std::path::PathBuf;

use super::{ModelFailure, ModelOutput, ModelSuccess};
use crate::failure::RunFailure;

mod inspect;
mod report;

#[cfg(test)]
mod tests;

const SCHEMA: &str = "fitr.retonr.evidence.v1";
const KIND: &str = "device_measurement";
const MAXIMUM_EVIDENCE_BYTES: usize = 64 * 1024;
const MAXIMUM_SHORT_TEXT_BYTES: usize = 256;
const MAXIMUM_DETAIL_TEXT_BYTES: usize = 1_024;
const MAXIMUM_NEEDS: usize = 128;
const MAXIMUM_SERVES: usize = 128;
const DISCLAIMER: &str = "This is a fitr measurement of one model on one device. It is not a retonr qualification, activation, or license decision.";
const ALLOWED_NEED_STATES: [&str; 6] = ["PASS", "FAIL", "SKIP", "n/a", "BLKD", "INCONCLUSIVE"];

/// Path to one fitr.retonr.evidence.v1 document.
#[derive(Debug, clap::Args)]
pub(crate) struct DeviceEvidenceArgs {
    /// Evidence file, or - for standard input.
    #[arg(value_name = "EVIDENCE")]
    pub(crate) source: PathBuf,
}

pub(crate) fn run(args: &DeviceEvidenceArgs) -> Result<ModelSuccess, ModelFailure> {
    let report = inspect::inspect(&args.source).map_err(ModelFailure::from_run)?;
    Ok(ModelSuccess {
        output: ModelOutput::device_evidence(&report),
        exit_code: std::process::ExitCode::SUCCESS,
    })
}

impl ModelFailure {
    fn from_run(error: RunFailure) -> Self {
        Self {
            command: error.command,
            body: error.body,
            exit_code: error.exit_code,
            message: error.message,
        }
    }
}
