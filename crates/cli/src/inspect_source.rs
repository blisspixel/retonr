//! Pre-model inventory of one source document without mutation.

use std::{fs, path::Path, process::ExitCode};

use rewrite_app::{
    CarrierPresence, LineEndingKind, MAX_CANDIDATE_CHECK_BYTES, PlainTextInventory, TextEncoding,
    inspect_plain_text,
};
use serde::Serialize;

use crate::contract::{CommandName, STANDARD_STREAM_PATH, read_input_bounded};
use crate::failure::RunFailure;
use crate::model::ModelOutput;

const SIDECAR_SUFFIXES: [&str; 2] = [".c2pa", ".xmp"];

/// Inventories one source file or standard input.
pub(crate) fn run(source: &Path) -> Result<(CommandName, ModelOutput, ExitCode), RunFailure> {
    let bytes = read_input_bounded(source, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::input_read(CommandName::Inspect, &error))?;
    let inventory = inspect_plain_text(&bytes)
        .map_err(|error| RunFailure::app(CommandName::Inspect, &error))?;
    let sidecars = sidecar_scan(source);
    let report = InspectReport::from_inventory(&inventory, sidecars);
    Ok((
        CommandName::Inspect,
        ModelOutput {
            value: serde_json::to_value(&report).expect("inspect report serializes"),
            text: report.text(),
            findings: false,
        },
        ExitCode::SUCCESS,
    ))
}

fn sidecar_scan(source: &Path) -> SidecarScan {
    if source.as_os_str() == STANDARD_STREAM_PATH {
        return SidecarScan {
            status: "not_applicable",
            present: Vec::new(),
        };
    }
    let Some(parent) = source.parent() else {
        return SidecarScan {
            status: "complete",
            present: Vec::new(),
        };
    };
    let Some(name) = source.file_name().and_then(|value| value.to_str()) else {
        return SidecarScan {
            status: "complete",
            present: Vec::new(),
        };
    };
    let mut present = Vec::new();
    for suffix in SIDECAR_SUFFIXES {
        let candidate = format!("{name}{suffix}");
        let path = parent.join(&candidate);
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && metadata.is_file()
            && !metadata.file_type().is_symlink()
        {
            present.push(candidate);
        }
    }
    present.sort();
    SidecarScan {
        status: "complete",
        present,
    }
}

#[derive(Serialize)]
struct SidecarScan {
    status: &'static str,
    present: Vec<String>,
}

#[derive(Serialize)]
struct InspectReport {
    encoding: TextEncoding,
    #[serde(skip_serializing_if = "Option::is_none")]
    valid_up_to: Option<String>,
    utf8_bom: bool,
    byte_size: String,
    digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_endings: Option<LineEndingKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_newline: Option<bool>,
    controls: ControlReport,
    c2pa_unstructured_text: CarrierPresence,
    sidecars: SidecarScan,
    external_references: &'static str,
    derivative: &'static str,
}

#[derive(Serialize)]
struct ControlReport {
    c0: String,
    c1: String,
    bidi: String,
    variation_selectors: String,
    zero_width: String,
    other_format: String,
}

impl InspectReport {
    fn from_inventory(inventory: &PlainTextInventory, sidecars: SidecarScan) -> Self {
        let derivative = derivative_decision(
            inventory.encoding,
            inventory.c2pa_unstructured_text,
            !sidecars.present.is_empty(),
        );
        Self {
            encoding: inventory.encoding,
            valid_up_to: inventory.valid_up_to.map(|value| value.to_string()),
            utf8_bom: inventory.utf8_bom,
            byte_size: inventory.byte_size.to_string(),
            digest: inventory.digest.as_str().to_owned(),
            line_endings: inventory.line_endings,
            final_newline: inventory.final_newline,
            controls: ControlReport {
                c0: inventory.controls.c0.to_string(),
                c1: inventory.controls.c1.to_string(),
                bidi: inventory.controls.bidi.to_string(),
                variation_selectors: inventory.controls.variation_selectors.to_string(),
                zero_width: inventory.controls.zero_width.to_string(),
                other_format: inventory.controls.other_format.to_string(),
            },
            c2pa_unstructured_text: inventory.c2pa_unstructured_text,
            sidecars,
            external_references: "not_checked",
            derivative,
        }
    }

    fn text(&self) -> String {
        let mut lines = vec![
            format!("encoding: {}", encoding_name(self.encoding)),
            format!("utf8_bom: {}", self.utf8_bom),
            format!("byte_size: {}", self.byte_size),
            format!("digest: {}", self.digest),
        ];
        if let Some(line_endings) = self.line_endings {
            lines.push(format!("line_endings: {}", line_ending_name(line_endings)));
        }
        if let Some(final_newline) = self.final_newline {
            lines.push(format!("final_newline: {final_newline}"));
        }
        lines.push(format!("c0: {}", self.controls.c0));
        lines.push(format!("c1: {}", self.controls.c1));
        lines.push(format!("bidi: {}", self.controls.bidi));
        lines.push(format!(
            "variation_selectors: {}",
            self.controls.variation_selectors
        ));
        lines.push(format!("zero_width: {}", self.controls.zero_width));
        lines.push(format!("other_format: {}", self.controls.other_format));
        lines.push(format!(
            "c2pa_unstructured_text: {}",
            carrier_name(self.c2pa_unstructured_text)
        ));
        let sidecars = if self.sidecars.present.is_empty() {
            self.sidecars.status.to_owned()
        } else {
            self.sidecars.present.join(",")
        };
        lines.push(format!("sidecars: {sidecars}"));
        lines.push(format!("external_references: {}", self.external_references));
        lines.push(format!("derivative: {}", self.derivative));
        lines.push(String::new());
        lines.join("\n")
    }
}

fn derivative_decision(
    encoding: TextEncoding,
    carrier: CarrierPresence,
    sidecar_present: bool,
) -> &'static str {
    if sidecar_present || carrier == CarrierPresence::Possible {
        return "explicit_decision_required";
    }
    if encoding != TextEncoding::Utf8 {
        return "not_checked";
    }
    "not_required"
}

const fn encoding_name(encoding: TextEncoding) -> &'static str {
    match encoding {
        TextEncoding::Utf8 => "utf8",
        TextEncoding::Utf16Le => "utf16_le",
        TextEncoding::Utf16Be => "utf16_be",
        TextEncoding::InvalidUtf8 => "invalid_utf8",
    }
}

const fn line_ending_name(kind: LineEndingKind) -> &'static str {
    match kind {
        LineEndingKind::None => "none",
        LineEndingKind::Lf => "lf",
        LineEndingKind::CrLf => "crlf",
        LineEndingKind::Cr => "cr",
        LineEndingKind::Mixed => "mixed",
    }
}

const fn carrier_name(presence: CarrierPresence) -> &'static str {
    match presence {
        CarrierPresence::Absent => "absent",
        CarrierPresence::Possible => "possible",
        CarrierPresence::NotDecoded => "not_decoded",
    }
}
