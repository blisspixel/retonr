use serde::Deserialize;

use rewrite_types::Digest;

use super::{
    MAX_QUALIFICATION_V2_JSON_BYTES, QualificationRecordV2, QualificationRecordV2Error,
    QualificationRecordV2Input,
};
use crate::runtime_identity::{append_digest, append_u32};
use crate::{
    ArtifactRole, ArtifactSetId, ArtifactSetManifest, EffectivePackageEvidence,
    EffectivePackageEvidenceId, EffectiveRuntimeState, EffectiveRuntimeStateId, LicenseDecision,
    QualificationStatus, RuntimeBuildId, RuntimeBuildIdentity,
};

impl QualificationRecordV2 {
    /// Parses a bounded JSON record and revalidates every exact reference.
    ///
    /// # Errors
    ///
    /// Returns [`QualificationRecordV2Error`] before decoding when the input exceeds
    /// its byte ceiling, or for any encoding, invariant, policy, or reference error.
    pub fn from_json_bytes(
        bytes: &[u8],
        artifact_set: &ArtifactSetManifest,
        runtime_build: &RuntimeBuildIdentity,
        runtime_state: &EffectiveRuntimeState,
        package_evidence: &EffectivePackageEvidence,
    ) -> Result<Self, QualificationRecordV2Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            role: ArtifactRole,
            artifact_set_id: ArtifactSetId,
            effective_package_evidence_id: EffectivePackageEvidenceId,
            runtime_build_id: RuntimeBuildId,
            effective_runtime_state_id: EffectiveRuntimeStateId,
            source_byte_limit: u64,
            context_token_limit: u32,
            prompt_template_digest: Digest,
            claim_output_contract_digest: Digest,
            claim_operation_contract_digest: Digest,
            request_policy_digest: Digest,
            threshold_policy_digest: Digest,
            language_policy_digest: Digest,
            hardware_envelope_digest: Digest,
            qualification_suite_digest: Digest,
            qualification_result_evidence_digest: Digest,
            license_decision: LicenseDecision,
            status: QualificationStatus,
        }

        if bytes.len() > MAX_QUALIFICATION_V2_JSON_BYTES {
            return Err(QualificationRecordV2Error::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| QualificationRecordV2Error::InvalidEncoding)?;
        let record = Self::from_wire(
            wire.schema_version,
            wire.role,
            wire.artifact_set_id,
            wire.effective_package_evidence_id,
            wire.runtime_build_id,
            wire.effective_runtime_state_id,
            QualificationRecordV2Input {
                source_byte_limit: wire.source_byte_limit,
                context_token_limit: wire.context_token_limit,
                prompt_template_digest: wire.prompt_template_digest,
                claim_output_contract_digest: wire.claim_output_contract_digest,
                claim_operation_contract_digest: wire.claim_operation_contract_digest,
                request_policy_digest: wire.request_policy_digest,
                threshold_policy_digest: wire.threshold_policy_digest,
                language_policy_digest: wire.language_policy_digest,
                hardware_envelope_digest: wire.hardware_envelope_digest,
                qualification_suite_digest: wire.qualification_suite_digest,
                qualification_result_evidence_digest: wire.qualification_result_evidence_digest,
                license_decision: wire.license_decision,
                status: wire.status,
            },
        )?;
        record.validate_against(artifact_set, runtime_build, runtime_state, package_evidence)?;
        Ok(record)
    }

    pub(super) fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(b"retonr:qualification-record:v2\0");
        append_u32(&mut output, self.schema_version);
        output.push(claim_extraction_role_byte(self.role));
        append_digest(&mut output, self.artifact_set_id.digest());
        append_digest(&mut output, self.effective_package_evidence_id.digest());
        append_digest(&mut output, self.runtime_build_id.digest());
        append_digest(&mut output, self.effective_runtime_state_id.digest());
        output.extend_from_slice(&self.source_byte_limit.to_be_bytes());
        append_u32(&mut output, self.context_token_limit);
        for digest in [
            &self.prompt_template_digest,
            &self.claim_output_contract_digest,
            &self.claim_operation_contract_digest,
            &self.request_policy_digest,
            &self.threshold_policy_digest,
            &self.language_policy_digest,
            &self.hardware_envelope_digest,
            &self.qualification_suite_digest,
            &self.qualification_result_evidence_digest,
        ] {
            append_digest(&mut output, digest);
        }
        output.push(license_byte(self.license_decision));
        output.push(status_byte(self.status));
        output
    }
}

pub(super) const fn claim_extraction_role_byte(role: ArtifactRole) -> u8 {
    match role {
        ArtifactRole::ClaimExtraction => 6,
        ArtifactRole::Generation
        | ArtifactRole::Embedding
        | ArtifactRole::SpeechRecognition
        | ArtifactRole::VoiceActivityDetection
        | ArtifactRole::SpeechSynthesis
        | ArtifactRole::Voice => u8::MAX,
    }
}

pub(super) const fn license_byte(value: LicenseDecision) -> u8 {
    match value {
        LicenseDecision::LocalUseOnly => 0,
        LicenseDecision::RedistributionApproved => 1,
        LicenseDecision::Rejected => 2,
    }
}

pub(super) const fn status_byte(value: QualificationStatus) -> u8 {
    match value {
        QualificationStatus::Qualified => 0,
        QualificationStatus::Rejected => 1,
    }
}
