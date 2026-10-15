//! Atomic qualification closeout over existing complete durable resource parents.

use super::{
    GenerationQualificationTerminalEvidenceV1ReadInput,
    GenerationQualificationTerminalEvidenceV1TransactionError, codec, context, read, write,
};
use crate::{
    ArtifactStateStore, GenerationHumanAdjudicationPolicyDenialV1Input,
    GenerationResourcePhaseV1Input, StoreError, StoreResult, WriteDisposition,
};
use rewrite_model::{GenerationQualificationRecordV1, GenerationQualificationRecordV1Relations};
use rusqlite::{Connection, TransactionBehavior};

mod validation;

/// Independent typed closeout facts over durable resource parents.
#[derive(Clone, Copy)]
pub struct GenerationQualificationTerminalCloseoutV1Input<'a> {
    /// Complete existing resource phase and independently retained canonical parents.
    pub resource_phase: GenerationResourcePhaseV1Input<'a>,
    /// Exact terminal receipt, assessments and recursive model relationships.
    pub qualification_relations: &'a GenerationQualificationRecordV1Relations<'a>,
    /// Exact inert qualification record to store.
    pub record: &'a GenerationQualificationRecordV1,
    /// Optional human-adjudication policy denial when the phase is policy-denied.
    pub human_policy_denial: Option<GenerationHumanAdjudicationPolicyDenialV1Input<'a>>,
}

impl ArtifactStateStore {
    /// Atomically closes a generation qualification over complete existing parents.
    ///
    /// Inserts platform evidence, license evidence, human adjudication manifest,
    /// optional human policy denial record, operation receipt, and qualification record.
    /// Existing complete phases are immutable parents.
    ///
    /// # Errors
    /// Refuses incomplete or changed parents, partial suffixes, canonical/indexed
    /// disagreement, status mismatches, or gate rejections.
    pub fn transact_generation_qualification_terminal_closeout_v1<E>(
        &mut self,
        input: GenerationQualificationTerminalCloseoutV1Input<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationQualificationTerminalEvidenceV1TransactionError<E>>
    {
        use GenerationQualificationTerminalEvidenceV1TransactionError as Error;
        gate().map_err(Error::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        gate().map_err(Error::Gate)?;
        let prepared = prepare(&transaction, &input).map_err(Error::Store)?;
        let disposition = write::insert_closeout_suffix(
            &transaction,
            &prepared,
            input.record,
            input.human_policy_denial.as_ref(),
        )
        .map_err(Error::Store)?;
        if load(&transaction, &input).map_err(Error::Store)?.as_ref() != Some(input.record) {
            return Err(Error::Store(StoreError::CorruptRecord));
        }
        gate().map_err(Error::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        Ok(disposition)
    }

    /// Reads the qualification closeout and full canonical parent tree in one snapshot.
    ///
    /// # Errors
    /// Refuses partial suffixes, changed indexed/canonical rows or parent corruption.
    pub fn generation_qualification_terminal_closeout_v1(
        &self,
        input: GenerationQualificationTerminalCloseoutV1Input<'_>,
    ) -> StoreResult<Option<GenerationQualificationRecordV1>> {
        let before = self.connection.total_changes();
        let transaction = self.connection.unchecked_transaction()?;
        let result = load(&transaction, &input)?;
        transaction.commit()?;
        if self.connection.total_changes() != before {
            return Err(StoreError::CorruptRecord);
        }
        Ok(result)
    }
}

fn prepare<'a>(
    connection: &Connection,
    input: &GenerationQualificationTerminalCloseoutV1Input<'a>,
) -> StoreResult<codec::PreparedCohort<'a>> {
    validation::validate(connection, input)?;
    let q = input.qualification_relations;
    let r = q.operation_receipt_relations;
    let ledger = input.resource_phase.repeatability.ledger;
    let anchor = context::load_anchor(
        connection,
        ledger.preregistration,
        ledger.managed_evidence_inputs,
    )?;
    if q.attempt_ledger_relations.attempt_records != anchor.ledger.attempt_records {
        return Err(StoreError::CorruptRecord);
    }
    let records = codec::CohortRecords {
        platform: r.platform_evidence.clone(),
        license: r.license_evidence.clone(),
        ledger: r.attempt_ledger_manifest.clone(),
        results: input.resource_phase.repeatability.ordered_results.to_vec(),
        repeatability: r.repeatability_manifest.clone(),
        resource: r.resource_manifest.clone(),
        human: r.human_adjudication_manifest.clone(),
        receipt: q.operation_receipt.clone(),
        interruption: None,
    };
    let encoded = codec::encode_records(&records)?;
    Ok(codec::PreparedCohort {
        records,
        encoded,
        preregistration: anchor.preregistration,
        read: GenerationQualificationTerminalEvidenceV1ReadInput {
            preregistration: ledger.preregistration,
            platform_input: q.platform_evidence_input,
            license_input: q.license_evidence_input,
            model_package_foundation_id: q.license_evidence_relations.model_package_foundation_id,
            model_license_control_id: q.license_evidence_relations.model_license_control_id,
            managed_evidence_inputs: ledger.managed_evidence_inputs,
            resource_evidence_record_digests: q.resource_manifest_relations.evidence_record_digests,
            human_evidence_record_digests: q
                .human_adjudication_manifest_relations
                .evidence_record_digests,
            receipt_input: q.operation_receipt_input,
            phase_interruption_input: None,
        },
    })
}

fn load(
    connection: &Connection,
    input: &GenerationQualificationTerminalCloseoutV1Input<'_>,
) -> StoreResult<Option<GenerationQualificationRecordV1>> {
    let prepared = prepare(connection, input)?;
    if !read::closeout::confirm(connection, &prepared, input.human_policy_denial.as_ref())? {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM generation_qualification_records WHERE generation_qualification_id = ?1 OR generation_qualification_operation_receipt_id = ?2 OR operation_policy_id = ?3)",
            rusqlite::params![
                input.record.generation_qualification_id().digest().as_str(),
                input.record.operation_receipt_id().digest().as_str(),
                input.record.operation_policy_id().digest().as_str(),
            ],
            |row| row.get(0),
        )?;
        return if exists {
            Err(StoreError::CorruptRecord)
        } else {
            Ok(None)
        };
    }
    let stored = crate::store::generation_qualification_record::load_on_connection(
        connection,
        input.record,
    )?
    .ok_or(StoreError::MissingRecord)?;
    if stored.status() != input.record.status() {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(stored))
}
