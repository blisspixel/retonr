use std::path::Path;

use rewrite_app::{
    CarrierPresence, LineEndingKind, MAX_CANDIDATE_CHECK_BYTES, TextEncoding,
    document_intake::{
        DerivativeDisposition, DocumentIntakeError, DocumentIntakeObservation,
        DocumentIntakeService, SidecarScanCompleteness,
    },
    document_selection::DocumentSelection,
};
use serde::Serialize;

use crate::contract::{CommandName, STANDARD_STREAM_PATH, read_input_bounded};
use crate::failure::RunFailure;
use crate::render::escape_inline_for_display;

pub(super) fn inspect_file(
    source: &Path,
    command: CommandName,
) -> Result<InspectReport, RunFailure> {
    if source.as_os_str() == STANDARD_STREAM_PATH {
        let bytes = read_input_bounded(source, MAX_CANDIDATE_CHECK_BYTES)
            .map_err(|error| RunFailure::input_read(command, &error))?;
        return inventory_report(source, command, &bytes).map(|(report, _)| report);
    }
    let document = DocumentIntakeService::read(
        &DocumentSelection::explicit(source),
        MAX_CANDIDATE_CHECK_BYTES,
        &rewrite_types::CancellationToken::new(),
    )
    .map_err(|error| intake_failure(command, &error))?;
    Ok(InspectReport::from_observation(
        source,
        &document.observation,
    ))
}

pub(super) fn inventory_report(
    source: &Path,
    command: CommandName,
    bytes: &[u8],
) -> Result<(InspectReport, usize), RunFailure> {
    let selected_path = (source.as_os_str() != STANDARD_STREAM_PATH).then_some(source);
    let observation = DocumentIntakeService::inspect_bytes(
        selected_path,
        bytes,
        &rewrite_types::CancellationToken::new(),
    )
    .map_err(|error| intake_failure(command, &error))?;
    Ok((
        InspectReport::from_observation(source, &observation),
        bytes.len(),
    ))
}

fn intake_failure(command: CommandName, error: &DocumentIntakeError) -> RunFailure {
    match error {
        DocumentIntakeError::Input(error) => RunFailure::input_read(command, error),
        DocumentIntakeError::Inspection(error) => RunFailure::app(command, error),
        DocumentIntakeError::Changed => RunFailure::concurrent_modification(command),
        DocumentIntakeError::Cancelled => RunFailure::cancelled(command),
    }
}

#[derive(Serialize)]
pub(super) struct SidecarScan {
    pub(super) status: &'static str,
    pub(super) present: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct InspectReport {
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
    pub(super) fn from_observation(source: &Path, observation: &DocumentIntakeObservation) -> Self {
        let inventory = &observation.inventory;
        let derivative = match observation.derivative {
            DerivativeDisposition::NotRequired => "not_required",
            DerivativeDisposition::ExplicitDecisionRequired => "explicit_decision_required",
            DerivativeDisposition::NotChecked => "not_checked",
        };
        let sidecars = SidecarScan {
            status: match observation.sidecars.completeness {
                SidecarScanCompleteness::Complete => "complete",
                SidecarScanCompleteness::NotApplicable => "not_applicable",
                SidecarScanCompleteness::Incomplete => "incomplete",
            },
            present: observation
                .sidecars
                .present
                .iter()
                .filter_map(|kind| {
                    let mut name = source.file_name()?.to_os_string();
                    name.push(kind.suffix());
                    Some(name.to_string_lossy().into_owned())
                })
                .collect(),
        };
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

    pub(crate) fn derivative(&self) -> &'static str {
        self.derivative
    }

    pub(crate) fn encoding(&self) -> TextEncoding {
        self.encoding
    }

    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }

    pub(super) fn text(&self) -> String {
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
            self.sidecars
                .present
                .iter()
                .map(|value| escape_inline_for_display(value))
                .collect::<Vec<_>>()
                .join(",")
        };
        lines.push(format!("sidecars: {sidecars}"));
        lines.push(format!("external_references: {}", self.external_references));
        lines.push(format!("derivative: {}", self.derivative));
        lines.push(String::new());
        lines.join("\n")
    }
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

#[cfg(test)]
mod tests;
