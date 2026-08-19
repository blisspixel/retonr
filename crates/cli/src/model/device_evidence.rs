//! Optional fitr device-measurement evidence. It is not a qualification.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{ModelFailure, ModelOutput, ModelSuccess};
use crate::contract::{
    CommandName, EXIT_COMPATIBILITY, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode,
    STANDARD_STREAM_PATH, read_input_bounded,
};
use crate::failure::RunFailure;

const SCHEMA: &str = "fitr.retonr.evidence.v1";
const KIND: &str = "device_measurement";
const MAXIMUM_EVIDENCE_BYTES: usize = 64 * 1024;
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
    let mut needs: Vec<NeedObservation> = raw
        .needs
        .into_iter()
        .map(|(name, observation)| {
            if !ALLOWED_NEED_STATES.contains(&observation.state.as_str()) {
                return Err(invalid_evidence());
            }
            Ok(NeedObservation {
                name,
                state: observation.state,
                why: empty_to_none(observation.why),
            })
        })
        .collect::<Result<_, _>>()?;
    needs.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(DeviceEvidenceReport {
        schema: SCHEMA,
        kind: KIND,
        disclaimer: DISCLAIMER,
        qualified: false,
        qualification: "absent",
        model: raw.model,
        quant: empty_to_none(raw.quant),
        family: empty_to_none(raw.family),
        param_size: empty_to_none(raw.param_size),
        level: empty_to_none(raw.level),
        repeats: raw.repeats.map(|value| value.to_string()),
        device_key: empty_to_none(raw.device_key),
        profile: empty_to_none(raw.profile),
        device: DeviceSummary {
            os: empty_to_none(raw.device.os),
            gpu: empty_to_none(raw.device.gpu),
            gpu_backend: empty_to_none(raw.device.gpu_backend),
            runtime: empty_to_none(raw.device.runtime),
            ram_gb: raw.device.ram_gb.map(number_string),
            vram_gb: raw.device.vram_gb.map(number_string),
            inference_device: empty_to_none(raw.device.inference_device),
        },
        needs,
        serves: raw.serves,
        use_for: empty_to_none(raw.use_for),
        plumbing: empty_to_none(raw.plumbing),
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

fn empty_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
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
        writeln!(text, "model: {}", report.model).expect("writing to a String cannot fail");
        writeln!(text, "disclaimer: {}", report.disclaimer)
            .expect("writing to a String cannot fail");
        if let Some(quant) = &report.quant {
            writeln!(text, "quant: {quant}").expect("writing to a String cannot fail");
        }
        if let Some(family) = &report.family {
            writeln!(text, "family: {family}").expect("writing to a String cannot fail");
        }
        if let Some(os) = &report.device.os {
            writeln!(text, "os: {os}").expect("writing to a String cannot fail");
        }
        if let Some(gpu) = &report.device.gpu {
            writeln!(text, "gpu: {gpu}").expect("writing to a String cannot fail");
        }
        if let Some(runtime) = &report.device.runtime {
            writeln!(text, "runtime: {runtime}").expect("writing to a String cannot fail");
        }
        for need in &report.needs {
            match &need.why {
                Some(why) => writeln!(text, "need {} state={} why={why}", need.name, need.state)
                    .expect("writing to a String cannot fail"),
                None => writeln!(text, "need {} state={}", need.name, need.state)
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
}
