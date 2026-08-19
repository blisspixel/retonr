use std::fmt::Write as _;

use rewrite_app::{ArtifactInventoryReport, RegisteredArtifactInspection};
use rewrite_model::{ArtifactManifest, LicenseRecord};
use serde::Serialize;

use super::{ModelOutput, RegisteredBytesSummary, role_name};
use crate::contract::ArtifactSelectionDto;

impl ModelOutput {
    pub(crate) fn list(report: ArtifactInventoryReport) -> Self {
        let artifacts: Vec<_> = report
            .registered
            .into_iter()
            .map(ListedArtifact::from_inspection)
            .collect();
        let mut text = format!("registered: {}\n", artifacts.len());
        for artifact in &artifacts {
            writeln!(
                text,
                "artifact {} generation={} bytes={} status={} roles={} qualified={}",
                artifact.selection.artifact_id,
                artifact.selection.installation_generation,
                artifact.byte_size,
                artifact.bytes.name(),
                join_roles(&artifact.active_roles),
                artifact.qualified
            )
            .expect("writing to a String cannot fail");
        }
        let result = ListResult { artifacts };
        Self {
            value: serde_json::to_value(result).expect("list DTO serialization is infallible"),
            text,
            findings: false,
        }
    }

    pub(crate) fn inspect(entry: RegisteredArtifactInspection) -> Self {
        let artifact = InspectedArtifact::from_inspection(entry);
        let mut text = format!(
            "artifact_id: {}\ninstallation_generation: {}\nbyte_size: {}\nbytes: {}\nactive_roles: {}\nqualified: {}\nqualification: {}\nformat: {}\nfamily: {}\n",
            artifact.selection.artifact_id,
            artifact.selection.installation_generation,
            artifact.byte_size,
            artifact.bytes.name(),
            join_roles(&artifact.active_roles),
            artifact.qualified,
            artifact.qualification,
            artifact.declared.format,
            artifact.declared.family,
        );
        if let Some(architecture) = &artifact.declared.architecture {
            writeln!(text, "architecture: {architecture}")
                .expect("writing to a String cannot fail");
        }
        if let Some(quantization) = &artifact.declared.quantization {
            writeln!(text, "quantization: {quantization}")
                .expect("writing to a String cannot fail");
        }
        for license in &artifact.declared.licenses {
            writeln!(
                text,
                "license {} identifier={}",
                license.component, license.identifier
            )
            .expect("writing to a String cannot fail");
        }
        writeln!(
            text,
            "declared_roles: {}\ndeclared_languages: {}",
            join_roles(&artifact.declared.roles),
            join_values(&artifact.declared.languages)
        )
        .expect("writing to a String cannot fail");
        if let Some(context_tokens) = &artifact.declared.context_tokens {
            writeln!(text, "declared_context_tokens: {context_tokens}")
                .expect("writing to a String cannot fail");
        }
        Self {
            value: serde_json::to_value(artifact).expect("inspect DTO serialization is infallible"),
            text,
            findings: false,
        }
    }
}

#[derive(Serialize)]
struct ListResult {
    artifacts: Vec<ListedArtifact>,
}

#[derive(Serialize)]
struct ListedArtifact {
    selection: ArtifactSelectionDto,
    byte_size: String,
    bytes: RegisteredBytesSummary,
    active_roles: Vec<&'static str>,
    qualified: bool,
}

impl ListedArtifact {
    fn from_inspection(entry: RegisteredArtifactInspection) -> Self {
        Self {
            selection: ArtifactSelectionDto::from(&entry.installation),
            byte_size: entry.manifest.byte_size.to_string(),
            bytes: RegisteredBytesSummary::from(entry.bytes),
            active_roles: entry
                .active_bindings
                .iter()
                .map(|binding| role_name(binding.role))
                .collect(),
            qualified: false,
        }
    }
}

#[derive(Serialize)]
struct InspectedArtifact {
    selection: ArtifactSelectionDto,
    byte_size: String,
    bytes: RegisteredBytesSummary,
    active_roles: Vec<&'static str>,
    qualified: bool,
    qualification: &'static str,
    declared: DeclaredInspection,
}

impl InspectedArtifact {
    fn from_inspection(entry: RegisteredArtifactInspection) -> Self {
        let active_roles = entry
            .active_bindings
            .iter()
            .map(|binding| role_name(binding.role))
            .collect();
        Self {
            selection: ArtifactSelectionDto::from(&entry.installation),
            byte_size: entry.manifest.byte_size.to_string(),
            declared: DeclaredInspection::from_manifest(&entry.manifest),
            bytes: RegisteredBytesSummary::from(entry.bytes),
            active_roles,
            qualified: false,
            qualification: "absent",
        }
    }
}

#[derive(Serialize)]
struct DeclaredInspection {
    format: String,
    family: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quantization: Option<String>,
    licenses: Vec<LicenseInspection>,
    roles: Vec<&'static str>,
    languages: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_tokens: Option<String>,
}

impl DeclaredInspection {
    fn from_manifest(manifest: &ArtifactManifest) -> Self {
        Self {
            format: manifest.format.clone(),
            family: manifest.family.clone(),
            architecture: manifest.architecture.clone(),
            quantization: manifest.quantization.clone(),
            licenses: manifest
                .licenses
                .iter()
                .map(LicenseInspection::from_record)
                .collect(),
            roles: manifest
                .declared_capabilities
                .roles
                .iter()
                .copied()
                .map(role_name)
                .collect(),
            languages: manifest.declared_capabilities.languages.clone(),
            context_tokens: manifest
                .declared_capabilities
                .context_tokens
                .map(|value| value.to_string()),
        }
    }
}

#[derive(Serialize)]
struct LicenseInspection {
    component: String,
    identifier: String,
}

impl LicenseInspection {
    fn from_record(record: &LicenseRecord) -> Self {
        Self {
            component: record.component.clone(),
            identifier: record.identifier.clone(),
        }
    }
}

fn join_roles(roles: &[&str]) -> String {
    if roles.is_empty() {
        "none".to_owned()
    } else {
        roles.join(",")
    }
}

fn join_values(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(",")
    }
}

#[cfg(test)]
mod tests {
    use rewrite_app::{
        ArtifactInstallationKey, RegisteredArtifactBytes, RegisteredArtifactInspection,
    };
    use rewrite_model::{
        ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole,
        ArtifactSource, DeclaredCapabilities, LicenseRecord,
    };
    use rewrite_types::Digest;
    use serde_json::Value;

    use super::InspectedArtifact;

    fn fixture_entry() -> RegisteredArtifactInspection {
        let digest = Digest::sha256(b"private model bytes for catalog tests");
        let manifest = ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            artifact_id: ArtifactId::from_digest(digest.clone()),
            source: ArtifactSource {
                origin: "private/provider/model".to_owned(),
                revision: "secret-revision".to_owned(),
            },
            artifact_digest: digest.clone(),
            byte_size: 32,
            format: "gguf".to_owned(),
            family: "private-family".to_owned(),
            architecture: Some("transformer".to_owned()),
            quantization: Some("q4".to_owned()),
            tokenizer: None,
            licenses: vec![LicenseRecord {
                component: "weights".to_owned(),
                identifier: "Apache-2.0".to_owned(),
                text_digest: Digest::sha256(b"license"),
            }],
            declared_capabilities: DeclaredCapabilities {
                roles: vec![ArtifactRole::Generation],
                languages: vec!["en".to_owned()],
                context_tokens: Some(8_192),
            },
        };
        RegisteredArtifactInspection {
            manifest,
            installation: ArtifactInstallationKey::new(ArtifactId::from_digest(digest), 1)
                .expect("positive generation"),
            active_bindings: Vec::new(),
            bytes: RegisteredArtifactBytes::Verified,
        }
    }

    #[test]
    fn inspect_reports_declared_facts_without_source_locators_or_qualification() {
        let encoded = serde_json::to_value(InspectedArtifact::from_inspection(fixture_entry()))
            .expect("serialize inspect DTO");
        assert_eq!(encoded["qualified"], false);
        assert_eq!(encoded["qualification"], "absent");
        assert_eq!(encoded["declared"]["family"], "private-family");
        assert_eq!(encoded["declared"]["format"], "gguf");
        assert_eq!(
            encoded["declared"]["roles"],
            serde_json::json!(["generation"])
        );
        assert_eq!(encoded["bytes"]["status"], "verified");
        let dump = encoded.to_string();
        assert!(!dump.contains("private/provider"));
        assert!(!dump.contains("secret-revision"));
        assert!(!dump.contains("origin"));
        assert!(!dump.contains("revision"));
        assert_eq!(encoded.get("source"), None);
        assert!(
            matches!(encoded["declared"].get("licenses"), Some(Value::Array(items)) if !items.is_empty())
        );
    }
}
