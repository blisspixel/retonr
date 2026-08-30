use serde::Deserialize;

use rewrite_types::Digest;

use super::{
    EffectivePackageEvidenceRoleV2, EffectivePackageEvidenceV2, EffectivePackageEvidenceV2Error,
    EffectivePackageEvidenceV2Input, EffectivePackageMemberEvidenceV2, EffectivePackageMemberUseV2,
    MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES,
};
use crate::runtime_identity::{append_digest, append_text, append_u32};
use crate::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetRelativePath,
    EffectivePackageEvidenceMode, EffectivePackageMemberPurpose, EffectiveRuntimeState,
    EffectiveRuntimeStateId, MAX_ARTIFACT_SET_MEMBERS, PackageTransformationDisposition,
    RuntimeBuildId, RuntimeBuildIdentity, RuntimeBuildMode,
};

impl EffectivePackageEvidenceV2 {
    /// Parses bounded JSON and revalidates it against exact references.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivePackageEvidenceV2Error`] before decoding when input
    /// exceeds its ceiling, or for any encoding, invariant, or reference error.
    pub fn from_json_bytes(
        bytes: &[u8],
        artifact_set: &ArtifactSetManifest,
        runtime_build: &RuntimeBuildIdentity,
        runtime_state: &EffectiveRuntimeState,
    ) -> Result<Self, EffectivePackageEvidenceV2Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct MemberWire {
            relative_path: String,
            artifact_id: ArtifactId,
            byte_size: u64,
            member_use: EffectivePackageMemberUseV2,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            evidence_mode: EffectivePackageEvidenceMode,
            artifact_set_id: ArtifactSetId,
            runtime_build_id: RuntimeBuildId,
            effective_runtime_state_id: EffectiveRuntimeStateId,
            evidence_contract_id: String,
            evidence_contract_schema_version: u32,
            member_evidence: Vec<MemberWire>,
            artifact_set_completeness_evidence_digest: Digest,
            acquisition_evidence_digest: Digest,
            license_review_evidence_digest: Digest,
            transformation: PackageTransformationDisposition,
            runtime_load_closure_evidence_digest: Digest,
            exclusion_isolation_evidence_digest: Digest,
        }

        if bytes.len() > MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES {
            return Err(EffectivePackageEvidenceV2Error::EncodedEvidenceTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| EffectivePackageEvidenceV2Error::InvalidEncoding)?;
        if wire.member_evidence.len() > MAX_ARTIFACT_SET_MEMBERS {
            return Err(EffectivePackageEvidenceV2Error::TooManyMembers);
        }
        let member_evidence = wire
            .member_evidence
            .into_iter()
            .map(|member| {
                let relative_path = ArtifactSetRelativePath::new(member.relative_path)
                    .map_err(|_| EffectivePackageEvidenceV2Error::InvalidMemberPath)?;
                EffectivePackageMemberEvidenceV2::new(
                    relative_path,
                    member.artifact_id,
                    member.byte_size,
                    member.member_use,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let evidence = Self::from_wire(
            wire.schema_version,
            wire.artifact_set_id,
            wire.runtime_build_id,
            wire.effective_runtime_state_id,
            EffectivePackageEvidenceV2Input {
                evidence_mode: wire.evidence_mode,
                evidence_contract_id: wire.evidence_contract_id,
                evidence_contract_schema_version: wire.evidence_contract_schema_version,
                member_evidence,
                artifact_set_completeness_evidence_digest: wire
                    .artifact_set_completeness_evidence_digest,
                acquisition_evidence_digest: wire.acquisition_evidence_digest,
                license_review_evidence_digest: wire.license_review_evidence_digest,
                transformation: wire.transformation,
                runtime_load_closure_evidence_digest: wire.runtime_load_closure_evidence_digest,
                exclusion_isolation_evidence_digest: wire.exclusion_isolation_evidence_digest,
            },
        )?;
        evidence.validate_against(artifact_set, runtime_build, runtime_state)?;
        Ok(evidence)
    }

    pub(super) fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(b"retonr:effective-package-evidence:v2\0");
        append_u32(&mut output, self.schema_version);
        output.push(evidence_mode_byte(self.evidence_mode));
        append_digest(&mut output, self.artifact_set_id.digest());
        append_digest(&mut output, self.runtime_build_id.digest());
        append_digest(&mut output, self.effective_runtime_state_id.digest());
        append_text(&mut output, &self.evidence_contract_id);
        append_u32(&mut output, self.evidence_contract_schema_version);
        let member_count =
            u32::try_from(self.member_evidence.len()).expect("validated member count fits u32");
        append_u32(&mut output, member_count);
        for member in &self.member_evidence {
            append_text(&mut output, member.relative_path.as_str());
            append_digest(&mut output, member.artifact_id.digest());
            output.extend_from_slice(&member.byte_size.to_be_bytes());
            append_member_use(&mut output, &member.member_use);
        }
        append_digest(&mut output, &self.artifact_set_completeness_evidence_digest);
        append_digest(&mut output, &self.acquisition_evidence_digest);
        append_digest(&mut output, &self.license_review_evidence_digest);
        append_transformation(&mut output, &self.transformation);
        append_digest(&mut output, &self.runtime_load_closure_evidence_digest);
        append_digest(&mut output, &self.exclusion_isolation_evidence_digest);
        output
    }
}

pub(super) const fn mode_matches_build(
    evidence_mode: EffectivePackageEvidenceMode,
    build_mode: RuntimeBuildMode,
) -> bool {
    matches!(
        (evidence_mode, build_mode),
        (
            EffectivePackageEvidenceMode::ManagedImmutablePackage,
            RuntimeBuildMode::ManagedProcess
        ) | (
            EffectivePackageEvidenceMode::AttachedAttestedPackage,
            RuntimeBuildMode::AttachedAttestedProcess | RuntimeBuildMode::AttachedAttestedContainer
        )
    )
}

const fn evidence_mode_byte(value: EffectivePackageEvidenceMode) -> u8 {
    match value {
        EffectivePackageEvidenceMode::ManagedImmutablePackage => 0,
        EffectivePackageEvidenceMode::AttachedAttestedPackage => 1,
    }
}

pub(super) const fn purpose_byte(value: EffectivePackageMemberPurpose) -> u8 {
    match value {
        EffectivePackageMemberPurpose::ModelWeights => 0,
        EffectivePackageMemberPurpose::ModelShardIndex => 1,
        EffectivePackageMemberPurpose::ModelConfiguration => 2,
        EffectivePackageMemberPurpose::GenerationConfiguration => 3,
        EffectivePackageMemberPurpose::TokenizerModel => 4,
        EffectivePackageMemberPurpose::TokenizerVocabulary => 5,
        EffectivePackageMemberPurpose::TokenizerMerges => 6,
        EffectivePackageMemberPurpose::TokenizerConfiguration => 7,
        EffectivePackageMemberPurpose::PromptTemplate => 8,
        EffectivePackageMemberPurpose::SystemPrompt => 9,
        EffectivePackageMemberPurpose::GrammarOrSchema => 10,
        EffectivePackageMemberPurpose::Adapter => 11,
        EffectivePackageMemberPurpose::Projector => 12,
        EffectivePackageMemberPurpose::DraftModel => 13,
        EffectivePackageMemberPurpose::CustomModelCode => 14,
        EffectivePackageMemberPurpose::CustomGenerationCode => 15,
        EffectivePackageMemberPurpose::AuxiliaryData => 16,
    }
}

pub(super) const fn evidence_role_byte(value: EffectivePackageEvidenceRoleV2) -> u8 {
    match value {
        EffectivePackageEvidenceRoleV2::LicenseText => 0,
        EffectivePackageEvidenceRoleV2::ProvenanceRecord => 1,
        EffectivePackageEvidenceRoleV2::TransformationEvidence => 2,
    }
}

fn append_member_use(output: &mut Vec<u8>, value: &EffectivePackageMemberUseV2) {
    match value {
        EffectivePackageMemberUseV2::Effective { purposes } => {
            output.push(0);
            append_purposes(output, purposes);
        }
        EffectivePackageMemberUseV2::EvidenceOnly { roles } => {
            output.push(1);
            let count = u32::try_from(roles.len()).expect("validated role count fits u32");
            append_u32(output, count);
            output.extend(roles.iter().copied().map(evidence_role_byte));
        }
        EffectivePackageMemberUseV2::EffectiveAndEvidence { purposes, roles } => {
            output.push(2);
            append_purposes(output, purposes);
            let count = u32::try_from(roles.len()).expect("validated role count fits u32");
            append_u32(output, count);
            output.extend(roles.iter().copied().map(evidence_role_byte));
        }
        EffectivePackageMemberUseV2::Excluded { declared_purposes } => {
            output.push(3);
            append_purposes(output, declared_purposes);
        }
    }
}

fn append_purposes(output: &mut Vec<u8>, purposes: &[EffectivePackageMemberPurpose]) {
    let count = u32::try_from(purposes.len()).expect("validated purpose count fits u32");
    append_u32(output, count);
    output.extend(purposes.iter().copied().map(purpose_byte));
}

fn append_transformation(output: &mut Vec<u8>, value: &PackageTransformationDisposition) {
    match value {
        PackageTransformationDisposition::Untransformed { evidence_digest } => {
            output.push(0);
            append_digest(output, evidence_digest);
        }
        PackageTransformationDisposition::Transformed {
            source_artifact_set_id,
            process_evidence_digest,
            parameters_digest,
            log_digest,
        } => {
            output.push(1);
            append_digest(output, source_artifact_set_id.digest());
            append_digest(output, process_evidence_digest);
            append_digest(output, parameters_digest);
            append_digest(output, log_digest);
        }
    }
}
