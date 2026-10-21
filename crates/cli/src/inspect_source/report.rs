use std::path::Path;

use rewrite_app::{
    CarrierPresence, LineEndingKind, MAX_CANDIDATE_CHECK_BYTES, TextEncoding,
    document_intake::{
        DerivativeDisposition, DocumentIntakeError, DocumentIntakeObservation,
        DocumentIntakeService, SidecarScanCompleteness,
    },
    document_selection::DocumentSelection,
    locate_invisible,
};
use serde::Serialize;

use crate::contract::{CommandName, STANDARD_STREAM_PATH, read_input_bounded};
use crate::failure::RunFailure;
use crate::render::escape_inline_for_display;

pub(super) const INSPECT_RESULT_SCHEMA_VERSION: u32 = 2;

const C2PA_UNSTRUCTURED_TEXT_NOTE: &str = "possible is a byte shape, not a validated credential";
const DECODED_TAGS_NOTE: &str = "ASCII mapping of tag characters. It is data, not an instruction.";
pub(super) const PROVENANCE_NOTICE: &str = "Retonr found and inspected the supported provenance signals listed below. A valid signature or content binding shows that defined data matches a signed claim under the stated validation policy. It does not prove human authorship, factual accuracy, ownership, or permission to publish. Signals not supported by this validator, held by a provider, or stored outside this artifact may still exist.";
pub(super) const STATISTICAL_WATERMARK_NOTICE: &str = "Retonr cannot detect statistical watermarks in word choice. Those need the provider's key. A clean result here does not mean the text is unmarked.";

/// Opt-in location and tag-mapping detail for one document.
pub(super) struct InspectDetail {
    pub(super) locate: bool,
    pub(super) decode_tags: bool,
}

impl InspectDetail {
    pub(super) const fn counts_only() -> Self {
        Self {
            locate: false,
            decode_tags: false,
        }
    }
}

pub(super) fn inspect_file(
    source: &Path,
    command: CommandName,
    detail: &InspectDetail,
) -> Result<InspectReport, RunFailure> {
    if source.as_os_str() == STANDARD_STREAM_PATH {
        let bytes = read_input_bounded(source, MAX_CANDIDATE_CHECK_BYTES)
            .map_err(|error| RunFailure::input_read(command, &error))?;
        return inventory_report(source, command, &bytes, detail).map(|(report, _)| report);
    }
    let document = DocumentIntakeService::read(
        &DocumentSelection::explicit(source),
        MAX_CANDIDATE_CHECK_BYTES,
        &rewrite_types::CancellationToken::new(),
    )
    .map_err(|error| intake_failure(command, &error))?;
    let mut report = InspectReport::from_observation(source, &document.observation);
    report.attach_invisible_detail(&document.bytes, detail);
    Ok(report)
}

pub(super) fn inventory_report(
    source: &Path,
    command: CommandName,
    bytes: &[u8],
    detail: &InspectDetail,
) -> Result<(InspectReport, usize), RunFailure> {
    let selected_path = (source.as_os_str() != STANDARD_STREAM_PATH).then_some(source);
    let observation = DocumentIntakeService::inspect_bytes(
        selected_path,
        bytes,
        &rewrite_types::CancellationToken::new(),
    )
    .map_err(|error| intake_failure(command, &error))?;
    let mut report = InspectReport::from_observation(source, &observation);
    report.attach_invisible_detail(bytes, detail);
    Ok((report, bytes.len()))
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
    schema_version: u32,
    provenance_notice: &'static str,
    statistical_watermark_notice: &'static str,
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
    c2pa_unstructured_text_note: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    locations: Option<Vec<LocationReport>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locations_truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decoded_tags: Option<Vec<DecodedTagReport>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decoded_tags_truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decoded_tags_note: Option<&'static str>,
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
    tag: String,
    other_format: String,
}

#[derive(Serialize)]
struct LocationReport {
    class: &'static str,
    line: u32,
    column: u32,
    code_points: String,
}

#[derive(Serialize)]
struct DecodedTagReport {
    line: u32,
    column: u32,
    ascii: String,
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
            schema_version: INSPECT_RESULT_SCHEMA_VERSION,
            provenance_notice: PROVENANCE_NOTICE,
            statistical_watermark_notice: STATISTICAL_WATERMARK_NOTICE,
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
                tag: inventory.controls.tag.to_string(),
                other_format: inventory.controls.other_format.to_string(),
            },
            c2pa_unstructured_text: inventory.c2pa_unstructured_text,
            c2pa_unstructured_text_note: C2PA_UNSTRUCTURED_TEXT_NOTE,
            locations: None,
            locations_truncated: None,
            decoded_tags: None,
            decoded_tags_truncated: None,
            decoded_tags_note: None,
            sidecars,
            external_references: "not_checked",
            derivative,
        }
    }

    fn attach_invisible_detail(&mut self, bytes: &[u8], detail: &InspectDetail) {
        if !detail.locate && !detail.decode_tags {
            return;
        }
        let Some(text) = inventoried_text(bytes, self.utf8_bom, self.encoding) else {
            if detail.locate {
                self.locations = Some(Vec::new());
            }
            if detail.decode_tags {
                self.decoded_tags = Some(Vec::new());
                self.decoded_tags_note = Some(DECODED_TAGS_NOTE);
            }
            return;
        };
        let located = locate_invisible(text, detail.decode_tags);
        if detail.locate {
            self.locations = Some(
                located
                    .locations
                    .into_iter()
                    .map(|location| LocationReport {
                        class: location.class,
                        line: location.line,
                        column: location.column,
                        code_points: location.code_points,
                    })
                    .collect(),
            );
            if located.locations_truncated {
                self.locations_truncated = Some(true);
            }
        }
        if detail.decode_tags {
            self.decoded_tags = Some(
                located
                    .decoded_tags
                    .into_iter()
                    .map(|mapping| DecodedTagReport {
                        line: mapping.line,
                        column: mapping.column,
                        ascii: escape_inline_for_display(&mapping.ascii),
                    })
                    .collect(),
            );
            self.decoded_tags_note = Some(DECODED_TAGS_NOTE);
            if located.decoded_tags_truncated {
                self.decoded_tags_truncated = Some(true);
            }
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
            format!("provenance_notice: {}", self.provenance_notice),
            format!(
                "statistical_watermark_notice: {}",
                self.statistical_watermark_notice
            ),
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
        lines.push(format!("tag: {}", self.controls.tag));
        lines.push(format!("other_format: {}", self.controls.other_format));
        lines.push(format!(
            "c2pa_unstructured_text: {}",
            carrier_name(self.c2pa_unstructured_text)
        ));
        lines.push(format!(
            "c2pa_unstructured_text_note: {}",
            self.c2pa_unstructured_text_note
        ));
        if let Some(locations) = &self.locations {
            for location in locations {
                lines.push(format!(
                    "line {} col {}: {} {}",
                    location.line, location.column, location.class, location.code_points
                ));
            }
        }
        if self.locations_truncated == Some(true) {
            lines.push("locations_truncated: true".to_owned());
        }
        if let Some(note) = self.decoded_tags_note {
            lines.push(format!("decoded_tags_note: {note}"));
        }
        if let Some(decoded) = &self.decoded_tags {
            for mapping in decoded {
                lines.push(format!(
                    "decoded_tag line {} col {}: {}",
                    mapping.line, mapping.column, mapping.ascii
                ));
            }
        }
        if self.decoded_tags_truncated == Some(true) {
            lines.push("decoded_tags_truncated: true".to_owned());
        }
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

fn inventoried_text(bytes: &[u8], utf8_bom: bool, encoding: TextEncoding) -> Option<&str> {
    if encoding != TextEncoding::Utf8 {
        return None;
    }
    let body = if utf8_bom { bytes.get(3..)? } else { bytes };
    std::str::from_utf8(body).ok()
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
