use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReviewInput {
    pub(super) schema_version: u32,
    pub(super) status: ReviewStatus,
    pub(super) subjects: Vec<SubjectSelection>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReviewStatus {
    PendingReview,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SubjectSelection {
    pub(super) declared_spdx_expression: String,
    pub(super) materials: Vec<MaterialSelection>,
    pub(super) subject_identity: Digest,
    pub(super) subject_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MaterialSelection {
    pub(super) kind: MaterialKind,
    pub(super) relative_path: String,
    pub(super) source_path: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum MaterialKind {
    LicenseText,
    Notice,
    Attribution,
    LicenseException,
    OtherLegalText,
}

impl MaterialKind {
    pub(super) const fn domain_name(self) -> &'static str {
        match self {
            Self::LicenseText => "license_text",
            Self::Notice => "notice",
            Self::Attribution => "attribution",
            Self::LicenseException => "license_exception",
            Self::OtherLegalText => "other_legal_text",
        }
    }
}
