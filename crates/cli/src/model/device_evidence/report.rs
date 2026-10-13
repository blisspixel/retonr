use serde::Serialize;

use super::super::ModelOutput;
use crate::render::escape_inline_for_display;

#[derive(Serialize)]
pub(crate) struct DeviceEvidenceReport {
    pub(super) schema: &'static str,
    pub(super) kind: &'static str,
    pub(super) disclaimer: &'static str,
    pub(super) qualified: bool,
    pub(super) qualification: &'static str,
    pub(super) model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) quant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) param_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) repeats: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) device_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) profile: Option<String>,
    pub(super) device: DeviceSummary,
    pub(super) needs: Vec<NeedObservation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) serves: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) use_for: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) plumbing: Option<String>,
}

#[derive(Serialize)]
pub(super) struct DeviceSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) gpu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) gpu_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) runtime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ram_gb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) vram_gb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) inference_device: Option<String>,
}

#[derive(Serialize)]
pub(super) struct NeedObservation {
    pub(super) name: String,
    pub(super) state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) why: Option<String>,
}

impl ModelOutput {
    pub(crate) fn device_evidence(report: &DeviceEvidenceReport) -> Self {
        use std::fmt::Write as _;
        let mut text = String::new();
        writeln!(text, "kind: {}", report.kind).expect("writing to a String cannot fail");
        writeln!(text, "qualified: {}", report.qualified).expect("writing to a String cannot fail");
        writeln!(text, "qualification: {}", report.qualification)
            .expect("writing to a String cannot fail");
        writeln!(text, "model: {}", escape_inline_for_display(&report.model))
            .expect("writing to a String cannot fail");
        if let Some(level) = &report.level {
            writeln!(text, "level: {}", escape_inline_for_display(level))
                .expect("writing to a String cannot fail");
        }
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
        if let Some(ram_gb) = &report.device.ram_gb {
            writeln!(text, "ram_gb: {}", escape_inline_for_display(ram_gb))
                .expect("writing to a String cannot fail");
        }
        if let Some(vram_gb) = &report.device.vram_gb {
            writeln!(text, "vram_gb: {}", escape_inline_for_display(vram_gb))
                .expect("writing to a String cannot fail");
        }
        if let Some(inference_device) = &report.device.inference_device {
            writeln!(
                text,
                "inference_device: {}",
                escape_inline_for_display(inference_device)
            )
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
        for served in &report.serves {
            writeln!(text, "serves: {}", escape_inline_for_display(served))
                .expect("writing to a String cannot fail");
        }
        if let Some(use_for) = &report.use_for {
            writeln!(text, "use_for: {}", escape_inline_for_display(use_for))
                .expect("writing to a String cannot fail");
        }
        Self {
            value: serde_json::to_value(report).expect("device evidence serializes"),
            text,
            findings: false,
        }
    }
}
