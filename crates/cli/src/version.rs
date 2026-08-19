//! Dedicated product and machine-contract version command.

use std::process::ExitCode;

use crate::contract::CommandName;
use crate::identity::ProductIdentity;
use crate::model::ModelOutput;

/// Completes the dedicated `version` command.
pub(crate) fn run() -> (CommandName, ModelOutput, ExitCode) {
    let identity = ProductIdentity::current();
    let output = ModelOutput {
        value: serde_json::to_value(&identity).expect("product identity serializes"),
        text: identity.text(),
        findings: false,
    };
    (CommandName::Version, output, ExitCode::SUCCESS)
}
