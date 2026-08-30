use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::{
    CandidateDeterministicEvaluationStatusV1, CandidateDeterministicReportRelationshipV1,
    CandidateDeterministicTransformationCoverageV1,
    MAX_CANDIDATE_DETERMINISTIC_EVALUATION_JSON_BYTES, candidate_deterministic_policy_digest,
};
use crate::generation_qualification::codec::{append_digest, append_u32, validate_canonical_json};
use crate::generation_qualification::{
    CANDIDATE_DETERMINISTIC_EVALUATION_ID_DOMAIN, CANDIDATE_RECEIPT_PAIR_SET_ID_DOMAIN,
    CandidateDeterministicEvaluationId, CandidateGenerationReceiptSetId,
    CandidateGenerationReceiptSetV1, CandidateReceiptPairSetId,
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationQualificationContractError,
    GenerationRepetitionId, GenerationSuiteManifestId, GenerationSystemId,
};

/// Exact non-report inputs required to derive or decode one evaluation record.
#[derive(Clone, Copy)]
pub struct CandidateDeterministicEvaluationRecordV1Input<'a> {
    /// Digest of the exact semantic-order case material set.
    pub case_material_set_digest: &'a Digest,
    /// Digest of the exact permutation-aware ordered suite pair.
    pub suite_pair_digest: &'a Digest,
    /// Checked report bytes, digests, and aggregate summaries.
    pub report_relationship: &'a CandidateDeterministicReportRelationshipV1,
}

/// Portable deterministic result for one ordered candidate receipt-set pair.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDeterministicEvaluationRecordV1 {
    schema_version: u32,
    candidate_receipt_pair_set_id: CandidateReceiptPairSetId,
    candidate_a_receipt_set_id: CandidateGenerationReceiptSetId,
    candidate_b_receipt_set_id: CandidateGenerationReceiptSetId,
    suite_manifest_id: GenerationSuiteManifestId,
    repetition_id: GenerationRepetitionId,
    candidate_a_generation_system_id: GenerationSystemId,
    candidate_b_generation_system_id: GenerationSystemId,
    case_material_set_digest: Digest,
    suite_pair_digest: Digest,
    deterministic_policy_digest: Digest,
    candidate_a_report_digest: Digest,
    candidate_b_report_digest: Digest,
    report_pair_digest: Digest,
    total: u32,
    passed: u32,
    transformation_coverage: CandidateDeterministicTransformationCoverageV1,
    status: CandidateDeterministicEvaluationStatusV1,
    #[serde(skip)]
    id: CandidateDeterministicEvaluationId,
}

impl CandidateReceiptPairSetId {
    /// Derives an ordered pair identity from two complete compatible receipt sets.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] unless A and B share the
    /// exact plan, suite, repetition, selection policy, and semantic entry closure,
    /// while naming distinct generation systems.
    pub fn from_receipt_sets(
        candidate_a: &CandidateGenerationReceiptSetV1,
        candidate_b: &CandidateGenerationReceiptSetV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_receipt_pair(candidate_a, candidate_b)?;
        let mut canonical = CANDIDATE_RECEIPT_PAIR_SET_ID_DOMAIN.to_vec();
        for digest in [
            candidate_a.suite_manifest_id().digest(),
            candidate_a.repetition_id().digest(),
            candidate_a.receipt_set_id().digest(),
            candidate_b.receipt_set_id().digest(),
            candidate_a.generation_system_id().digest(),
            candidate_b.generation_system_id().digest(),
        ] {
            append_digest(&mut canonical, digest);
        }
        Ok(Self(Digest::sha256(&canonical)))
    }
}

impl CandidateDeterministicEvaluationRecordV1 {
    /// Derives one portable deterministic record from complete receipt sets and
    /// exact checked report relationships.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for substituted receipt
    /// scope, report counts that do not close over both sets, inconsistent aggregate
    /// arithmetic, or an identity that exceeds its fixed ceiling.
    pub fn new(
        candidate_a: &CandidateGenerationReceiptSetV1,
        candidate_b: &CandidateGenerationReceiptSetV1,
        input: CandidateDeterministicEvaluationRecordV1Input<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        let pair_id = CandidateReceiptPairSetId::from_receipt_sets(candidate_a, candidate_b)?;
        let summary_a = input.report_relationship.candidate_a_summary();
        let summary_b = input.report_relationship.candidate_b_summary();
        if summary_a.total() != candidate_a.entry_count()
            || summary_b.total() != candidate_b.entry_count()
        {
            return Err(
                GenerationQualificationContractError::DeterministicEvaluationRelationshipMismatch,
            );
        }
        // Each summary is capped at one suite and closed over its receipt set, so
        // these sums are bounded by twice MAX_GENERATION_SUITE_CASES.
        let total = summary_a.total() + summary_b.total();
        let passed = summary_a.passed() + summary_b.passed();
        let transformation_coverage = CandidateDeterministicTransformationCoverageV1 {
            acceptable: summary_a.transformation_coverage().acceptable()
                + summary_b.transformation_coverage().acceptable(),
            rewritten: summary_a.transformation_coverage().rewritten()
                + summary_b.transformation_coverage().rewritten(),
        };
        let status = if passed == total {
            CandidateDeterministicEvaluationStatusV1::Passed
        } else {
            CandidateDeterministicEvaluationStatusV1::Failed
        };
        let mut record = Self {
            schema_version: GENERATION_QUALIFICATION_SCHEMA_VERSION,
            candidate_receipt_pair_set_id: pair_id,
            candidate_a_receipt_set_id: candidate_a.receipt_set_id().clone(),
            candidate_b_receipt_set_id: candidate_b.receipt_set_id().clone(),
            suite_manifest_id: candidate_a.suite_manifest_id().clone(),
            repetition_id: candidate_a.repetition_id().clone(),
            candidate_a_generation_system_id: candidate_a.generation_system_id().clone(),
            candidate_b_generation_system_id: candidate_b.generation_system_id().clone(),
            case_material_set_digest: input.case_material_set_digest.clone(),
            suite_pair_digest: input.suite_pair_digest.clone(),
            deterministic_policy_digest: candidate_deterministic_policy_digest(),
            candidate_a_report_digest: input
                .report_relationship
                .candidate_a_report_digest()
                .clone(),
            candidate_b_report_digest: input
                .report_relationship
                .candidate_b_report_digest()
                .clone(),
            report_pair_digest: input.report_relationship.report_pair_digest().clone(),
            total,
            passed,
            transformation_coverage,
            status,
            id: CandidateDeterministicEvaluationId(Digest::sha256(b"uninitialized")),
        };
        record.id = CandidateDeterministicEvaluationId(Digest::sha256(&record.canonical_bytes()));
        Ok(record)
    }

    /// Parses bounded canonical JSON and rederives every field from exact inputs.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// unknown, duplicate, trailing, future-schema, noncanonical, or stale input.
    pub fn from_json_bytes(
        bytes: &[u8],
        candidate_a: &CandidateGenerationReceiptSetV1,
        candidate_b: &CandidateGenerationReceiptSetV1,
        input: CandidateDeterministicEvaluationRecordV1Input<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        let wire = decode_wire(bytes)?;
        if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version,
            ));
        }
        let record = Self::new(candidate_a, candidate_b, input)?;
        if !wire.matches(&record) {
            return Err(
                GenerationQualificationContractError::DeterministicEvaluationRelationshipMismatch,
            );
        }
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact ordered receipt-pair identity.
    #[must_use]
    pub const fn candidate_receipt_pair_set_id(&self) -> &CandidateReceiptPairSetId {
        &self.candidate_receipt_pair_set_id
    }

    /// Returns candidate A's exact receipt-set identity.
    #[must_use]
    pub const fn candidate_a_receipt_set_id(&self) -> &CandidateGenerationReceiptSetId {
        &self.candidate_a_receipt_set_id
    }

    /// Returns candidate B's exact receipt-set identity.
    #[must_use]
    pub const fn candidate_b_receipt_set_id(&self) -> &CandidateGenerationReceiptSetId {
        &self.candidate_b_receipt_set_id
    }

    /// Returns the common exact suite identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }

    /// Returns the common exact repetition identity.
    #[must_use]
    pub const fn repetition_id(&self) -> &GenerationRepetitionId {
        &self.repetition_id
    }

    /// Returns candidate A's generation-system identity.
    #[must_use]
    pub const fn candidate_a_generation_system_id(&self) -> &GenerationSystemId {
        &self.candidate_a_generation_system_id
    }

    /// Returns candidate B's generation-system identity.
    #[must_use]
    pub const fn candidate_b_generation_system_id(&self) -> &GenerationSystemId {
        &self.candidate_b_generation_system_id
    }

    /// Returns the exact case-material set digest.
    #[must_use]
    pub const fn case_material_set_digest(&self) -> &Digest {
        &self.case_material_set_digest
    }

    /// Returns the permutation-aware ordered suite-pair digest.
    #[must_use]
    pub const fn suite_pair_digest(&self) -> &Digest {
        &self.suite_pair_digest
    }

    /// Returns the frozen deterministic-policy digest.
    #[must_use]
    pub const fn deterministic_policy_digest(&self) -> &Digest {
        &self.deterministic_policy_digest
    }

    /// Returns candidate A's report digest.
    #[must_use]
    pub const fn candidate_a_report_digest(&self) -> &Digest {
        &self.candidate_a_report_digest
    }

    /// Returns candidate B's report digest.
    #[must_use]
    pub const fn candidate_b_report_digest(&self) -> &Digest {
        &self.candidate_b_report_digest
    }

    /// Returns the ordered compatibility report-pair digest.
    #[must_use]
    pub const fn report_pair_digest(&self) -> &Digest {
        &self.report_pair_digest
    }

    /// Returns the aggregate case count across A and B.
    #[must_use]
    pub const fn total(&self) -> u32 {
        self.total
    }

    /// Returns the aggregate passing-case count across A and B.
    #[must_use]
    pub const fn passed(&self) -> u32 {
        self.passed
    }

    /// Returns aggregate transformation coverage across A and B.
    #[must_use]
    pub const fn transformation_coverage(&self) -> CandidateDeterministicTransformationCoverageV1 {
        self.transformation_coverage
    }

    /// Returns the outcome derived from the exact two report summaries.
    #[must_use]
    pub const fn status(&self) -> CandidateDeterministicEvaluationStatusV1 {
        self.status
    }

    /// Returns the canonical content identity of this record.
    #[must_use]
    pub const fn deterministic_evaluation_id(&self) -> &CandidateDeterministicEvaluationId {
        &self.id
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = CANDIDATE_DETERMINISTIC_EVALUATION_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.candidate_receipt_pair_set_id.digest(),
            self.candidate_a_receipt_set_id.digest(),
            self.candidate_b_receipt_set_id.digest(),
            self.suite_manifest_id.digest(),
            self.repetition_id.digest(),
            self.candidate_a_generation_system_id.digest(),
            self.candidate_b_generation_system_id.digest(),
            &self.case_material_set_digest,
            &self.suite_pair_digest,
            &self.deterministic_policy_digest,
            &self.candidate_a_report_digest,
            &self.candidate_b_report_digest,
            &self.report_pair_digest,
        ] {
            append_digest(&mut output, digest);
        }
        append_u32(&mut output, self.total);
        append_u32(&mut output, self.passed);
        append_u32(&mut output, self.transformation_coverage.acceptable());
        append_u32(&mut output, self.transformation_coverage.rewritten());
        output.push(match self.status {
            CandidateDeterministicEvaluationStatusV1::Passed => 0,
            CandidateDeterministicEvaluationStatusV1::Failed => 1,
        });
        output
    }
}

impl fmt::Debug for CandidateDeterministicEvaluationRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateDeterministicEvaluationRecordV1")
            .field("schema_version", &self.schema_version)
            .field("deterministic_evaluation_id", &self.id)
            .field("total", &self.total)
            .field("passed", &self.passed)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    candidate_receipt_pair_set_id: CandidateReceiptPairSetId,
    candidate_a_receipt_set_id: CandidateGenerationReceiptSetId,
    candidate_b_receipt_set_id: CandidateGenerationReceiptSetId,
    suite_manifest_id: GenerationSuiteManifestId,
    repetition_id: GenerationRepetitionId,
    candidate_a_generation_system_id: GenerationSystemId,
    candidate_b_generation_system_id: GenerationSystemId,
    case_material_set_digest: Digest,
    suite_pair_digest: Digest,
    deterministic_policy_digest: Digest,
    candidate_a_report_digest: Digest,
    candidate_b_report_digest: Digest,
    report_pair_digest: Digest,
    total: u32,
    passed: u32,
    transformation_coverage: CandidateDeterministicTransformationCoverageV1,
    status: CandidateDeterministicEvaluationStatusV1,
}

impl Wire {
    fn matches(&self, record: &CandidateDeterministicEvaluationRecordV1) -> bool {
        self.schema_version == record.schema_version
            && self.candidate_receipt_pair_set_id == record.candidate_receipt_pair_set_id
            && self.candidate_a_receipt_set_id == record.candidate_a_receipt_set_id
            && self.candidate_b_receipt_set_id == record.candidate_b_receipt_set_id
            && self.suite_manifest_id == record.suite_manifest_id
            && self.repetition_id == record.repetition_id
            && self.candidate_a_generation_system_id == record.candidate_a_generation_system_id
            && self.candidate_b_generation_system_id == record.candidate_b_generation_system_id
            && self.case_material_set_digest == record.case_material_set_digest
            && self.suite_pair_digest == record.suite_pair_digest
            && self.deterministic_policy_digest == record.deterministic_policy_digest
            && self.candidate_a_report_digest == record.candidate_a_report_digest
            && self.candidate_b_report_digest == record.candidate_b_report_digest
            && self.report_pair_digest == record.report_pair_digest
            && self.total == record.total
            && self.passed == record.passed
            && self.transformation_coverage == record.transformation_coverage
            && self.status == record.status
    }
}

fn decode_wire(bytes: &[u8]) -> Result<Wire, GenerationQualificationContractError> {
    if bytes.len() > MAX_CANDIDATE_DETERMINISTIC_EVALUATION_JSON_BYTES {
        return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
    }
    serde_json::from_slice(bytes).map_err(|_| GenerationQualificationContractError::InvalidEncoding)
}

fn validate_receipt_pair(
    candidate_a: &CandidateGenerationReceiptSetV1,
    candidate_b: &CandidateGenerationReceiptSetV1,
) -> Result<(), GenerationQualificationContractError> {
    let semantic_entries_match = candidate_a.entries().len() == candidate_b.entries().len()
        && candidate_a
            .entries()
            .iter()
            .zip(candidate_b.entries())
            .all(|(entry_a, entry_b)| {
                entry_a.case_id() == entry_b.case_id()
                    && entry_a.selected_ordinal() == entry_b.selected_ordinal()
            });
    if candidate_a.qualification_plan_id() != candidate_b.qualification_plan_id()
        || candidate_a.suite_manifest_id() != candidate_b.suite_manifest_id()
        || candidate_a.repetition_id() != candidate_b.repetition_id()
        || candidate_a.selection_policy_id() != candidate_b.selection_policy_id()
        || candidate_a.generation_system_id() == candidate_b.generation_system_id()
        || candidate_a.entry_count() != candidate_b.entry_count()
        || !semantic_entries_match
    {
        return Err(GenerationQualificationContractError::ReceiptPairRelationshipMismatch);
    }
    Ok(())
}
