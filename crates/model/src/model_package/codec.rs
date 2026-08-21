use serde::Deserialize;

use crate::runtime_identity::{append_digest, append_text, append_u32};
use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetRelativePath, PackageSource, PackageTransformation,
};

use super::{
    EmbeddedModelComponent, EmbeddedModelComponentPurpose, MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES,
    ModelPackageManifest, ModelPackageManifestError, ModelPackageMember, ModelPackageMemberRole,
    ModelWeightLayout,
};

impl ModelPackageManifest {
    /// Parses bounded JSON and revalidates all byte and semantic relationships.
    ///
    /// # Errors
    ///
    /// Returns [`ModelPackageManifestError`] for malformed or inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        artifact_set: &ArtifactSetManifest,
    ) -> Result<Self, ModelPackageManifestError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct MemberWire {
            artifact_id: ArtifactId,
            byte_size: u64,
            relative_path: String,
            roles: Vec<ModelPackageMemberRole>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct EmbeddedWire {
            container_path: String,
            purpose: EmbeddedModelComponentPurpose,
            extraction_contract_id: String,
            extraction_contract_schema_version: u32,
            selector: String,
            value_digest: rewrite_types::Digest,
        }
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum LayoutWire {
            Single { member: String },
            Sharded { shards: Vec<String>, index: String },
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            artifact_set_id: crate::ArtifactSetId,
            format_contract_id: String,
            format_contract_schema_version: u32,
            source: serde_json::Value,
            transformation: PackageTransformation,
            members: Vec<MemberWire>,
            weight_layout: LayoutWire,
            embedded_components: Vec<EmbeddedWire>,
        }

        if bytes.len() > MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES {
            return Err(ModelPackageManifestError::EncodedManifestTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| ModelPackageManifestError::InvalidEncoding)?;
        if wire.artifact_set_id != artifact_set.artifact_set_id() {
            return Err(ModelPackageManifestError::ArtifactSetMismatch);
        }
        let source = PackageSource::from_json_bytes(
            &serde_json::to_vec(&wire.source)
                .map_err(|_| ModelPackageManifestError::InvalidSource)?,
        )
        .map_err(|_| ModelPackageManifestError::InvalidSource)?;
        let members = wire
            .members
            .into_iter()
            .map(|member| {
                Ok(ModelPackageMember::new(
                    member.artifact_id,
                    member.byte_size,
                    parse_path(member.relative_path)?,
                    member.roles,
                ))
            })
            .collect::<Result<Vec<_>, ModelPackageManifestError>>()?;
        let weight_layout = match wire.weight_layout {
            LayoutWire::Single { member } => ModelWeightLayout::Single {
                member: parse_path(member)?,
            },
            LayoutWire::Sharded { shards, index } => ModelWeightLayout::Sharded {
                shards: shards
                    .into_iter()
                    .map(parse_path)
                    .collect::<Result<Vec<_>, _>>()?,
                index: parse_path(index)?,
            },
        };
        let embedded_components = wire
            .embedded_components
            .into_iter()
            .map(|component| {
                EmbeddedModelComponent::new(
                    parse_path(component.container_path)?,
                    component.purpose,
                    component.extraction_contract_id,
                    component.extraction_contract_schema_version,
                    component.selector,
                    component.value_digest,
                )
            })
            .collect::<Result<Vec<_>, ModelPackageManifestError>>()?;
        ModelPackageManifest::from_wire(
            wire.schema_version,
            artifact_set,
            wire.format_contract_id,
            wire.format_contract_schema_version,
            source,
            wire.transformation,
            members,
            weight_layout,
            embedded_components,
        )
    }

    pub(super) fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(b"retonr:model-package-manifest:v1\0");
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.artifact_set_id.digest());
        append_text(&mut output, &self.format_contract_id);
        append_u32(&mut output, self.format_contract_schema_version);
        self.source.append_identity(&mut output);
        self.transformation.append_canonical(&mut output);
        append_u32(
            &mut output,
            u32::try_from(self.members.len()).expect("validated member count fits u32"),
        );
        for member in &self.members {
            append_text(&mut output, member.relative_path.as_str());
            append_digest(&mut output, member.artifact_id.digest());
            output.extend_from_slice(&member.byte_size.to_be_bytes());
            append_u32(
                &mut output,
                u32::try_from(member.roles.len()).expect("validated role count fits u32"),
            );
            output.extend(member.roles.iter().copied().map(role_byte));
        }
        append_weight_layout(&mut output, &self.weight_layout);
        append_u32(
            &mut output,
            u32::try_from(self.embedded_components.len())
                .expect("validated embedded count fits u32"),
        );
        for component in &self.embedded_components {
            append_text(&mut output, component.container_path.as_str());
            output.push(embedded_purpose_byte(component.purpose));
            append_text(&mut output, &component.extraction_contract_id);
            append_u32(&mut output, component.extraction_contract_schema_version);
            append_text(&mut output, &component.selector);
            append_digest(&mut output, &component.value_digest);
        }
        output
    }
}

fn parse_path(value: String) -> Result<ArtifactSetRelativePath, ModelPackageManifestError> {
    ArtifactSetRelativePath::new(value).map_err(|_| ModelPackageManifestError::InvalidMemberPath)
}

fn append_weight_layout(output: &mut Vec<u8>, layout: &ModelWeightLayout) {
    output.push(weight_layout_byte(layout));
    match layout {
        ModelWeightLayout::Single { member } => {
            append_text(output, member.as_str());
        }
        ModelWeightLayout::Sharded { shards, index } => {
            append_u32(
                output,
                u32::try_from(shards.len()).expect("validated shard count fits u32"),
            );
            for shard in shards {
                append_text(output, shard.as_str());
            }
            append_text(output, index.as_str());
        }
    }
}

const fn weight_layout_byte(value: &ModelWeightLayout) -> u8 {
    match value {
        ModelWeightLayout::Single { .. } => 0,
        ModelWeightLayout::Sharded { .. } => 1,
    }
}

pub(super) const fn embedded_purpose_byte(value: EmbeddedModelComponentPurpose) -> u8 {
    match value {
        EmbeddedModelComponentPurpose::ModelConfiguration => 0,
        EmbeddedModelComponentPurpose::GenerationConfiguration => 1,
        EmbeddedModelComponentPurpose::Tokenizer => 2,
        EmbeddedModelComponentPurpose::PromptTemplate => 3,
    }
}

pub(super) const fn role_byte(value: ModelPackageMemberRole) -> u8 {
    match value {
        ModelPackageMemberRole::ModelWeights => 0,
        ModelPackageMemberRole::ModelWeightShard => 1,
        ModelPackageMemberRole::ModelShardIndex => 2,
        ModelPackageMemberRole::ModelConfiguration => 3,
        ModelPackageMemberRole::GenerationConfiguration => 4,
        ModelPackageMemberRole::TokenizerModel => 5,
        ModelPackageMemberRole::TokenizerVocabulary => 6,
        ModelPackageMemberRole::TokenizerMerges => 7,
        ModelPackageMemberRole::TokenizerConfiguration => 8,
        ModelPackageMemberRole::PromptTemplate => 9,
        ModelPackageMemberRole::SystemPrompt => 10,
        ModelPackageMemberRole::Adapter => 11,
        ModelPackageMemberRole::Projector => 12,
        ModelPackageMemberRole::DraftModel => 13,
        ModelPackageMemberRole::GrammarOrSchema => 14,
        ModelPackageMemberRole::CustomModelCode => 15,
        ModelPackageMemberRole::CustomGenerationCode => 16,
        ModelPackageMemberRole::LicenseText => 17,
        ModelPackageMemberRole::ProvenanceRecord => 18,
        ModelPackageMemberRole::TransformationEvidence => 19,
        ModelPackageMemberRole::AuxiliaryData => 20,
    }
}
