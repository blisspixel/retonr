//! Optional fitr device-measurement evidence. It is not a qualification.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{ModelFailure, ModelOutput, ModelSuccess};
use crate::contract::{
    CommandName, EXIT_COMPATIBILITY, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode,
    STANDARD_STREAM_PATH, read_input_bounded,
};
use crate::failure::RunFailure;
use crate::render::escape_inline_for_display;

const SCHEMA: &str = "fitr.retonr.evidence.v1";
const KIND: &str = "device_measurement";
const MAXIMUM_EVIDENCE_BYTES: usize = 64 * 1024;
const MAXIMUM_SHORT_TEXT_BYTES: usize = 256;
const MAXIMUM_DETAIL_TEXT_BYTES: usize = 1_024;
const MAXIMUM_NEEDS: usize = 128;
const MAXIMUM_SERVES: usize = 128;
const DISCLAIMER: &str = "This is a fitr measurement of one model on one device. It is not a retonr qualification, activation, or license decision.";
const ALLOWED_NEED_STATES: [&str; 5] = ["PASS", "FAIL", "SKIP", "n/a", "BLKD"];

/// Path to one fitr.retonr.evidence.v1 document.
#[derive(Debug, clap::Args)]
pub(crate) struct DeviceEvidenceArgs {
    /// Evidence file, or - for standard input.
    #[arg(value_name = "EVIDENCE")]
    pub(crate) source: PathBuf,
}

pub(crate) fn run(args: &DeviceEvidenceArgs) -> Result<ModelSuccess, ModelFailure> {
    let report = inspect(&args.source).map_err(ModelFailure::from_run)?;
    Ok(ModelSuccess {
        output: ModelOutput::device_evidence(&report),
        exit_code: std::process::ExitCode::SUCCESS,
    })
}

fn inspect(source: &Path) -> Result<DeviceEvidenceReport, RunFailure> {
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
                return Err(invalid_evidence());
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

#[derive(Serialize)]
pub(crate) struct DeviceEvidenceReport {
    schema: &'static str,
    kind: &'static str,
    disclaimer: &'static str,
    qualified: bool,
    qualification: &'static str,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    quant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    param_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeats: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    device: DeviceSummary,
    needs: Vec<NeedObservation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    serves: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    use_for: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plumbing: Option<String>,
}

#[derive(Serialize)]
struct DeviceSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gpu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gpu_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ram_gb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vram_gb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inference_device: Option<String>,
}

#[derive(Serialize)]
struct NeedObservation {
    name: String,
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    why: Option<String>,
}

impl super::ModelOutput {
    pub(crate) fn device_evidence(report: &DeviceEvidenceReport) -> Self {
        use std::fmt::Write as _;
        let mut text = String::new();
        writeln!(text, "kind: {}", report.kind).expect("writing to a String cannot fail");
        writeln!(text, "qualified: {}", report.qualified).expect("writing to a String cannot fail");
        writeln!(text, "qualification: {}", report.qualification)
            .expect("writing to a String cannot fail");
        writeln!(text, "model: {}", escape_inline_for_display(&report.model))
            .expect("writing to a String cannot fail");
        writeln!(text, "disclaimer: {}", report.disclaimer)
            .expect("writing to a String cannot fail");
        if let Some(quant) = &report.quant {
            writeln!(text, "quant: {}", escape_inline_for_display(quant))
                .expect("writing to a String cannot fail");
        }
        if let Some(family) = &report.family {
            writeln!(text, "family: {}", escape_inline_for_display(family))
                .expect("writing to a String cannot fail");
        }
        if let Some(os) = &report.device.os {
            writeln!(text, "os: {}", escape_inline_for_display(os))
                .expect("writing to a String cannot fail");
        }
        if let Some(gpu) = &report.device.gpu {
            writeln!(text, "gpu: {}", escape_inline_for_display(gpu))
                .expect("writing to a String cannot fail");
        }
        if let Some(runtime) = &report.device.runtime {
            writeln!(text, "runtime: {}", escape_inline_for_display(runtime))
                .expect("writing to a String cannot fail");
        }
        for need in &report.needs {
            match &need.why {
                Some(why) => writeln!(
                    text,
                    "need {} state={} why={}",
                    escape_inline_for_display(&need.name),
                    escape_inline_for_display(&need.state),
                    escape_inline_for_display(why)
                )
                .expect("writing to a String cannot fail"),
                None => writeln!(
                    text,
                    "need {} state={}",
                    escape_inline_for_display(&need.name),
                    escape_inline_for_display(&need.state)
                )
                .expect("writing to a String cannot fail"),
            }
        }
        Self {
            value: serde_json::to_value(report).expect("device evidence serializes"),
            text,
            findings: false,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> serde_json::Value {
        json!({
            "schema": SCHEMA,
            "kind": KIND,
            "disclaimer": "This is a fitr measurement of one model on one device. It is not a retonr qualification, activation, or license decision.",
            "sister": "https://github.com/blisspixel/retonr",
            "fitr_version": "0.2.0",
            "model": "demo:8b",
            "quant": "Q4_K_M",
            "family": "qwen3",
            "param_size": "8B",
            "level": "full",
            "repeats": 3,
            "device": {
                "host": "secret-laptop",
                "os": "windows",
                "cpu": "secret-cpu",
                "ram_gb": 32.0,
                "gpu": "demo-gpu",
                "gpu_driver": "secret-driver",
                "ollama": "0.32.14",
                "inference_device": "GPU 100%",
                "gpu_backend": "cuda",
                "vram_gb": 8.0,
                "config": { "OLLAMA_MODELS": "C:\\\\secret\\\\models" }
            },
            "device_key": "opaque-key",
            "profile": "default",
            "needs": {
                "structured_output": { "state": "PASS", "why": "6/7" },
                "vision": { "state": "n/a", "why": "text-only" }
            },
            "serves": ["structured_output"],
            "use_for": "JSON pipelines",
            "plumbing": "healthy",
            "result": "C:\\\\Users\\\\secret\\\\.fitr\\\\results\\\\demo.json"
        })
    }

    #[test]
    fn accepted_evidence_is_measurement_not_qualification() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("demo.retonr.json");
        std::fs::write(&path, sample().to_string()).expect("write evidence");
        let report = inspect(&path).expect("inspect");
        assert_eq!(report.kind, KIND);
        assert!(!report.qualified);
        assert_eq!(report.qualification, "absent");
        assert_eq!(report.model, "demo:8b");
        assert_eq!(report.device.gpu.as_deref(), Some("demo-gpu"));
        assert_eq!(report.device.runtime.as_deref(), Some("0.32.14"));
        assert_eq!(report.needs[0].name, "structured_output");
        assert_eq!(report.needs[1].state, "n/a");
        let encoded = serde_json::to_string(&report).expect("serialize");
        assert!(!encoded.contains("secret-laptop"));
        assert!(!encoded.contains("secret-cpu"));
        assert!(!encoded.contains("OLLAMA_MODELS"));
        assert!(!encoded.contains("C:\\\\Users"));
        assert!(!encoded.contains("secret-driver"));
        assert!(encoded.contains("\"qualified\":false"));
    }

    #[test]
    fn dishonest_disclaimer_is_unsupported() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bad.json");
        let mut value = sample();
        value["disclaimer"] = json!("ready to activate");
        std::fs::write(&path, value.to_string()).expect("write");
        let Err(failure) = inspect(&path) else {
            panic!("dishonest disclaimer should refuse");
        };
        assert_eq!(
            failure.exit_code,
            std::process::ExitCode::from(EXIT_COMPATIBILITY)
        );
    }

    #[test]
    fn forwarded_fields_are_bounded_and_terminal_safe() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("unsafe.json");
        let mut value = sample();
        value["model"] = json!("demo\u{202e}:8b");
        std::fs::write(&path, value.to_string()).expect("write unsafe evidence");
        let Err(failure) = inspect(&path) else {
            panic!("directionality must be refused");
        };
        assert_eq!(
            failure.body,
            ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidManifest, false)
        );

        let mut value = sample();
        value["model"] = json!("x".repeat(MAXIMUM_SHORT_TEXT_BYTES + 1));
        std::fs::write(&path, value.to_string()).expect("write oversized evidence");
        let Err(failure) = inspect(&path) else {
            panic!("oversized model must be refused");
        };
        assert_eq!(
            failure.body,
            ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidManifest, false)
        );

        let mut value = sample();
        value["device"]["ram_gb"] = json!(-1.0);
        std::fs::write(&path, value.to_string()).expect("write invalid measurement");
        let Err(failure) = inspect(&path) else {
            panic!("negative memory must be refused");
        };
        assert_eq!(
            failure.body,
            ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidManifest, false)
        );
    }
}
