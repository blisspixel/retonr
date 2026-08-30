use rewrite_ollama_package::RuntimeSourceBuildInputRole;
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InventoryWire {
    pub(super) materials: Vec<MaterialWire>,
    pub(super) schema_version: u32,
    pub(super) status: InventoryStatus,
    pub(super) subjects: Vec<SubjectWire>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum InventoryStatus {
    PendingReview,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum SubjectWire {
    Component {
        byte_size: u64,
        declared_spdx_expression: String,
        digest: Digest,
        material_ids: Vec<Digest>,
        name: String,
        relative_path: String,
        revision: String,
        roles: Vec<RuntimeSourceBuildInputRole>,
        source_locator: String,
    },
    CargoPackage {
        checksum: Option<String>,
        declared_spdx_expression: String,
        material_ids: Vec<Digest>,
        name: String,
        source: Option<String>,
        version: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MaterialWire {
    pub(super) byte_size: u64,
    pub(super) content: String,
    pub(super) digest: Digest,
    pub(super) kind: MaterialKind,
    pub(super) relative_path: String,
    pub(super) subject_key: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[cfg(test)]
mod tests {
    use super::MaterialKind;

    #[test]
    fn every_material_kind_has_one_fixed_identity_name() {
        assert_eq!(MaterialKind::LicenseText.domain_name(), "license_text");
        assert_eq!(MaterialKind::Notice.domain_name(), "notice");
        assert_eq!(MaterialKind::Attribution.domain_name(), "attribution");
        assert_eq!(
            MaterialKind::LicenseException.domain_name(),
            "license_exception"
        );
        assert_eq!(
            MaterialKind::OtherLegalText.domain_name(),
            "other_legal_text"
        );
    }
}
