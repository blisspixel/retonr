use std::collections::BTreeSet;

use crate::ArtifactSetManifest;

use super::{
    EMBEDDED_MODEL_COMPONENT_LIMIT, EmbeddedModelComponentPurpose, MAX_FORMAT_CONTRACT_BYTES,
    MAX_MODEL_MEMBER_ROLES, MAX_MODEL_PACKAGE_CANONICAL_BYTES, MAX_MODEL_ROLE_ASSIGNMENTS,
    ModelPackageManifest, ModelPackageManifestError, ModelPackageMemberRole, ModelWeightLayout,
    valid_machine_id,
};

impl ModelPackageManifest {
    #[expect(
        clippy::too_many_arguments,
        reason = "wire reconstruction validates every explicit field"
    )]
    pub(super) fn from_wire(
        schema_version: u32,
        artifact_set: &ArtifactSetManifest,
        format_contract_id: String,
        format_contract_schema_version: u32,
        source: crate::PackageSource,
        transformation: crate::PackageTransformation,
        members: Vec<super::ModelPackageMember>,
        weight_layout: ModelWeightLayout,
        embedded_components: Vec<super::EmbeddedModelComponent>,
    ) -> Result<Self, ModelPackageManifestError> {
        if schema_version != super::MODEL_PACKAGE_MANIFEST_SCHEMA_VERSION {
            return Err(ModelPackageManifestError::UnsupportedSchema(schema_version));
        }
        if !valid_machine_id(&format_contract_id, MAX_FORMAT_CONTRACT_BYTES)
            || format_contract_schema_version == 0
        {
            return Err(ModelPackageManifestError::InvalidFormatContract);
        }
        let manifest = Self {
            schema_version,
            artifact_set_id: artifact_set.artifact_set_id(),
            format_contract_id,
            format_contract_schema_version,
            source,
            transformation,
            members,
            weight_layout,
            embedded_components,
        };
        manifest.validate_against(artifact_set)?;
        if manifest.canonical_bytes().len() > MAX_MODEL_PACKAGE_CANONICAL_BYTES {
            return Err(ModelPackageManifestError::CanonicalEncodingTooLarge);
        }
        Ok(manifest)
    }

    /// Revalidates complete semantic coverage against the byte manifest.
    ///
    /// # Errors
    ///
    /// Returns [`ModelPackageManifestError`] on any relationship or evidence drift.
    pub fn validate_against(
        &self,
        artifact_set: &ArtifactSetManifest,
    ) -> Result<(), ModelPackageManifestError> {
        if self.artifact_set_id != artifact_set.artifact_set_id() {
            return Err(ModelPackageManifestError::ArtifactSetMismatch);
        }
        if self.members.len() != artifact_set.members().len() {
            return Err(ModelPackageManifestError::MemberCoverageMismatch);
        }
        let mut assignments = 0usize;
        for (semantic, content) in self.members.iter().zip(artifact_set.members()) {
            validate_member(semantic)?;
            if semantic.relative_path() != content.relative_path()
                || semantic.artifact_id() != content.artifact_id()
                || semantic.byte_size() != content.byte_size()
            {
                return Err(ModelPackageManifestError::MemberCoverageMismatch);
            }
            assignments = assignments
                .checked_add(semantic.roles.len())
                .ok_or(ModelPackageManifestError::TooManyRoleAssignments)?;
        }
        if assignments > MAX_MODEL_ROLE_ASSIGNMENTS {
            return Err(ModelPackageManifestError::TooManyRoleAssignments);
        }
        self.validate_embedded_components()?;
        self.validate_weight_layout()?;
        self.validate_foundational_evidence()?;
        Ok(())
    }

    fn validate_embedded_components(&self) -> Result<(), ModelPackageManifestError> {
        if self.embedded_components.len() > EMBEDDED_MODEL_COMPONENT_LIMIT {
            return Err(ModelPackageManifestError::InvalidEmbeddedComponent);
        }
        let member_paths = self
            .members
            .iter()
            .map(|member| member.relative_path.as_str())
            .collect::<BTreeSet<_>>();
        let mut prior = None;
        let mut purposes = BTreeSet::new();
        for embedded in &self.embedded_components {
            embedded.validate()?;
            let key = (
                super::codec::embedded_purpose_byte(embedded.purpose),
                embedded.container_path.as_str(),
            );
            if prior.is_some_and(|value| value >= key)
                || !purposes.insert(key.0)
                || !member_paths.contains(embedded.container_path.as_str())
            {
                return Err(ModelPackageManifestError::InvalidEmbeddedComponent);
            }
            prior = Some(key);
        }
        Ok(())
    }

    fn validate_weight_layout(&self) -> Result<(), ModelPackageManifestError> {
        let complete = role_paths(&self.members, ModelPackageMemberRole::ModelWeights);
        let shards = role_paths(&self.members, ModelPackageMemberRole::ModelWeightShard);
        let indexes = role_paths(&self.members, ModelPackageMemberRole::ModelShardIndex);
        let valid = match &self.weight_layout {
            ModelWeightLayout::Single { member } => {
                complete.as_slice() == [member.as_str()] && shards.is_empty() && indexes.is_empty()
            }
            ModelWeightLayout::Sharded {
                shards: declared,
                index,
            } => {
                let declared_paths = declared
                    .iter()
                    .map(crate::ArtifactSetRelativePath::as_str)
                    .collect::<Vec<_>>();
                !declared.is_empty()
                    && declared
                        .windows(2)
                        .all(|pair| pair[0].as_str().as_bytes() < pair[1].as_str().as_bytes())
                    && complete.is_empty()
                    && declared_paths == shards
                    && indexes.as_slice() == [index.as_str()]
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ModelPackageManifestError::InvalidWeightLayout)
        }
    }

    fn validate_foundational_evidence(&self) -> Result<(), ModelPackageManifestError> {
        let role_count = |role| {
            self.members
                .iter()
                .filter(|member| member.roles.contains(&role))
                .count()
        };
        let embedded_count = |purpose| {
            self.embedded_components
                .iter()
                .filter(|component| component.purpose == purpose)
                .count()
        };
        let tokenizer_files = [
            ModelPackageMemberRole::TokenizerModel,
            ModelPackageMemberRole::TokenizerVocabulary,
            ModelPackageMemberRole::TokenizerMerges,
        ]
        .into_iter()
        .map(&role_count)
        .sum::<usize>();
        let template_files = role_count(ModelPackageMemberRole::PromptTemplate);
        let configuration_files = role_count(ModelPackageMemberRole::ModelConfiguration);
        let tokenizer_embedded = embedded_count(EmbeddedModelComponentPurpose::Tokenizer);
        let template_embedded = embedded_count(EmbeddedModelComponentPurpose::PromptTemplate);
        let configuration_embedded =
            embedded_count(EmbeddedModelComponentPurpose::ModelConfiguration);
        if (tokenizer_files > 0) == (tokenizer_embedded > 0)
            || (template_files == 0 && template_embedded == 0)
            || template_files > 1
            || template_embedded > 1
            || (configuration_files > 0) == (configuration_embedded > 0)
            || configuration_files > 1
        {
            return Err(ModelPackageManifestError::MissingFoundationalComponent);
        }
        if role_count(ModelPackageMemberRole::LicenseText) == 0
            || role_count(ModelPackageMemberRole::ProvenanceRecord) == 0
        {
            return Err(ModelPackageManifestError::MissingEvidence);
        }
        if self.transformation.requires_transformation_record()
            && role_count(ModelPackageMemberRole::TransformationEvidence) == 0
        {
            return Err(ModelPackageManifestError::MissingTransformationEvidence);
        }
        Ok(())
    }
}

fn validate_member(member: &super::ModelPackageMember) -> Result<(), ModelPackageManifestError> {
    if member.roles.is_empty()
        || member.roles.len() > MAX_MODEL_MEMBER_ROLES
        || member
            .roles
            .windows(2)
            .any(|pair| super::codec::role_byte(pair[0]) >= super::codec::role_byte(pair[1]))
        || (member.roles.iter().any(|role| role.is_evidence_only())
            && member.roles.iter().any(|role| !role.is_evidence_only()))
    {
        return Err(ModelPackageManifestError::InvalidMemberRoles);
    }
    Ok(())
}

fn role_paths(members: &[super::ModelPackageMember], role: ModelPackageMemberRole) -> Vec<&str> {
    members
        .iter()
        .filter(|member| member.roles.contains(&role))
        .map(|member| member.relative_path.as_str())
        .collect()
}
