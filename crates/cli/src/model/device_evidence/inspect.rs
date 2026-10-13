use std::path::Path;

use serde::Deserialize;

use super::report::{DeviceEvidenceReport, DeviceSummary, NeedObservation};
use super::{
    ALLOWED_NEED_STATES, DISCLAIMER, KIND, MAXIMUM_DETAIL_TEXT_BYTES, MAXIMUM_EVIDENCE_BYTES,
    MAXIMUM_NEEDS, MAXIMUM_SERVES, MAXIMUM_SHORT_TEXT_BYTES, SCHEMA,
};
use crate::contract::{
    CommandName, EXIT_COMPATIBILITY, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode,
    STANDARD_STREAM_PATH, read_input_bounded,
};
use crate::failure::RunFailure;

pub(super) fn inspect(source: &Path) -> Result<DeviceEvidenceReport, RunFailure> {
    refuse_symlink(source)?;
    let bytes = read_input_bounded(source, MAXIMUM_EVIDENCE_BYTES)
        .map_err(|error| RunFailure::input_read(CommandName::ModelDeviceEvidence, &error))?;
    let raw: RawEvidence = serde_json::from_slice(&bytes).map_err(|_| invalid_evidence())?;
    if raw.schema != SCHEMA {
        return Err(unsupported_evidence());
    }
    if raw.kind != KIND {
        return Err(unsupported_evidence());
    }
    if !honest_disclaimer(&raw.disclaimer) {
        return Err(unsupported_evidence());
    }
    if raw.needs.len() > MAXIMUM_NEEDS || raw.serves.len() > MAXIMUM_SERVES {
        return Err(invalid_evidence());
    }
    let model = required_text(&raw.model, MAXIMUM_SHORT_TEXT_BYTES)?;
    let quant = optional_text(raw.quant, MAXIMUM_SHORT_TEXT_BYTES)?;
    let family = optional_text(raw.family, MAXIMUM_SHORT_TEXT_BYTES)?;
    let param_size = optional_text(raw.param_size, MAXIMUM_SHORT_TEXT_BYTES)?;
    let level = optional_text(raw.level, MAXIMUM_SHORT_TEXT_BYTES)?;
    let device_key = optional_text(raw.device_key, MAXIMUM_SHORT_TEXT_BYTES)?;
    let profile = optional_text(raw.profile, MAXIMUM_SHORT_TEXT_BYTES)?;
    let use_for = optional_text(raw.use_for, MAXIMUM_DETAIL_TEXT_BYTES)?;
    let plumbing = optional_text(raw.plumbing, MAXIMUM_DETAIL_TEXT_BYTES)?;
    let device = DeviceSummary {
        os: optional_text(raw.device.os, MAXIMUM_SHORT_TEXT_BYTES)?,
        gpu: optional_text(raw.device.gpu, MAXIMUM_SHORT_TEXT_BYTES)?,
        gpu_backend: optional_text(raw.device.gpu_backend, MAXIMUM_SHORT_TEXT_BYTES)?,
        runtime: optional_text(raw.device.runtime, MAXIMUM_SHORT_TEXT_BYTES)?,
        ram_gb: optional_positive_number(raw.device.ram_gb)?,
        vram_gb: optional_positive_number(raw.device.vram_gb)?,
        inference_device: optional_text(raw.device.inference_device, MAXIMUM_SHORT_TEXT_BYTES)?,
    };
    let mut needs: Vec<NeedObservation> = raw
        .needs
        .into_iter()
        .map(|(name, observation)| {
            if !ALLOWED_NEED_STATES.contains(&observation.state.as_str()) {
                return Err(unsupported_need_state(&name, &observation.state));
            }
            Ok(NeedObservation {
                name: required_text(&name, MAXIMUM_SHORT_TEXT_BYTES)?,
                state: observation.state,
                why: optional_text(observation.why, MAXIMUM_DETAIL_TEXT_BYTES)?,
            })
        })
        .collect::<Result<_, _>>()?;
    needs.sort_by(|left, right| left.name.cmp(&right.name));
    let mut unique_serves = std::collections::BTreeSet::new();
    let serves = raw
        .serves
        .into_iter()
        .map(|value| required_text(&value, MAXIMUM_SHORT_TEXT_BYTES))
        .collect::<Result<Vec<_>, _>>()?;
    if serves
        .iter()
        .any(|value| !unique_serves.insert(value.as_str()))
    {
        return Err(invalid_evidence());
    }
    Ok(DeviceEvidenceReport {
        schema: SCHEMA,
        kind: KIND,
        disclaimer: DISCLAIMER,
        qualified: false,
        qualification: "absent",
        model,
        quant,
        family,
        param_size,
        level,
        repeats: raw.repeats.map(|value| value.to_string()),
        device_key,
        profile,
        device,
        needs,
        serves,
        use_for,
        plumbing,
    })
}

fn refuse_symlink(source: &Path) -> Result<(), RunFailure> {
    if source.as_os_str() == STANDARD_STREAM_PATH {
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(source)
        .map_err(|error| RunFailure::input_read(CommandName::ModelDeviceEvidence, &error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RunFailure {
            command: CommandName::ModelDeviceEvidence,
            body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
            exit_code: std::process::ExitCode::from(EXIT_USAGE),
            message: "device evidence must be a regular file",
        });
    }
    Ok(())
}

fn honest_disclaimer(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("not") && lower.contains("qualification") && lower.contains("activation")
}

fn required_text(value: &str, maximum_bytes: usize) -> Result<String, RunFailure> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > maximum_bytes
        || crate::render::contains_terminal_effect(value)
    {
        return Err(invalid_evidence());
    }
    Ok(trimmed.to_owned())
}

fn optional_text(
    value: Option<String>,
    maximum_bytes: usize,
) -> Result<Option<String>, RunFailure> {
    let Some(text) = value else {
        return Ok(None);
    };
    if crate::render::contains_terminal_effect(&text) {
        return Err(invalid_evidence());
    }
    if text.trim().is_empty() {
        return Ok(None);
    }
    required_text(&text, maximum_bytes).map(Some)
}

fn optional_positive_number(value: Option<f64>) -> Result<Option<String>, RunFailure> {
    value
        .map(|number| {
            if number.is_finite() && number > 0.0 {
                Ok(number_string(number))
            } else {
                Err(invalid_evidence())
            }
        })
        .transpose()
}

fn number_string(value: f64) -> String {
    let rendered = format!("{value:.4}");
    rendered
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn unsupported_need_state(name: &str, state: &str) -> RunFailure {
    let mut failure = invalid_evidence();
    if safe_token(name) && safe_token(state) {
        failure.body = failure
            .body
            .with_diagnostic(&format!("needs.{name}.state: unsupported value {state}"));
    }
    failure
}

fn safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn invalid_evidence() -> RunFailure {
    RunFailure {
        command: CommandName::ModelDeviceEvidence,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidManifest, false),
        exit_code: std::process::ExitCode::from(EXIT_USAGE),
        message: "device evidence is not a valid fitr.retonr.evidence.v1 document",
    }
}

fn unsupported_evidence() -> RunFailure {
    RunFailure {
        command: CommandName::ModelDeviceEvidence,
        body: ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false),
        exit_code: std::process::ExitCode::from(EXIT_COMPATIBILITY),
        message: "device evidence is not accepted as a qualification",
    }
}

#[derive(Deserialize)]
struct RawEvidence {
    schema: String,
    kind: String,
    disclaimer: String,
    model: String,
    #[serde(default)]
    quant: Option<String>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    param_size: Option<String>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    repeats: Option<u32>,
    #[serde(default)]
    device: RawDevice,
    #[serde(default)]
    device_key: Option<String>,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    needs: std::collections::BTreeMap<String, RawNeed>,
    #[serde(default)]
    serves: Vec<String>,
    #[serde(default)]
    use_for: Option<String>,
    #[serde(default)]
    plumbing: Option<String>,
}

#[derive(Default, Deserialize)]
struct RawDevice {
    #[serde(default)]
    os: Option<String>,
    #[serde(default)]
    gpu: Option<String>,
    #[serde(default)]
    gpu_backend: Option<String>,
    #[serde(default, rename = "ollama")]
    runtime: Option<String>,
    #[serde(default)]
    ram_gb: Option<f64>,
    #[serde(default)]
    vram_gb: Option<f64>,
    #[serde(default)]
    inference_device: Option<String>,
}

#[derive(Deserialize)]
struct RawNeed {
    state: String,
    #[serde(default)]
    why: Option<String>,
}
