//! Atomic rejected closeout over existing complete durable resource parents.

use super::{
    GenerationQualificationTerminalEvidenceV1ReadInput,
    GenerationQualificationTerminalEvidenceV1TransactionError, codec, context, read, write,
};
use crate::{
    ArtifactStateStore, GenerationResourcePhaseV1Input, StoreError, StoreResult, WriteDisposition,
};
use rewrite_model::{
    GenerationQualificationPhaseStatusV1, GenerationQualificationRecordV1,
    GenerationQualificationRecordV1Relations, GenerationQualificationStatusV1,
};
use rusqlite::{Connection, TransactionBehavior};

mod validation;

/// Independent typed closeout facts, never a source of live qualification authority.
#[derive(Clone, Copy)]
pub struct GenerationResourceRejectionV1Input<'a> {
    /// Complete existing resource phase and independently retained canonical parents.
    pub resource_phase: GenerationResourcePhaseV1Input<'a>,
    /// Exact terminal receipt, assessments and recursive model relationships.
    pub qualification_relations: &'a GenerationQualificationRecordV1Relations<'a>,
    /// Exact inert Rejected qualification record to store.
    pub record: &'a GenerationQualificationRecordV1,
}

impl ArtifactStateStore {
    /// Atomically closes a policy-directed resource rejection over existing parents.
    ///
    /// Only the assessments, skipped human phase, negative completed receipt and
    /// Rejected record are inserted. Existing complete phases are immutable parents.
    /// A stored record grants no qualification, activation or traffic authority.
    ///
    /// # Errors
    /// Refuses nonfailed resources, incomplete or changed parents, partial suffixes,
    /// canonical/indexed disagreement and the original caller gate.
    pub fn transact_generation_resource_rejection_v1<E>(
        &mut self,
        input: GenerationResourceRejectionV1Input<'_>,
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
        let disposition =
            write::insert_suffix(&transaction, &prepared, input.record).map_err(Error::Store)?;
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

    /// Reads the rejected closeout and full canonical parent tree in one snapshot.
    ///
    /// # Errors
    /// Refuses partial suffixes, changed indexed/canonical rows or parent corruption.
    pub fn generation_resource_rejection_v1(
        &self,
        input: GenerationResourceRejectionV1Input<'_>,
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
    input: &GenerationResourceRejectionV1Input<'a>,
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
            human_evidence_record_digests: &[],
            receipt_input: q.operation_receipt_input,
            phase_interruption_input: None,
        },
    })
}

fn load(
    connection: &Connection,
    input: &GenerationResourceRejectionV1Input<'_>,
) -> StoreResult<Option<GenerationQualificationRecordV1>> {
    let prepared = prepare(connection, input)?;
    if !read::resource_rejection::confirm(connection, &prepared)? {
        let exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM generation_qualification_records WHERE generation_qualification_id = ?1 OR generation_qualification_operation_receipt_id = ?2 OR operation_policy_id = ?3)", rusqlite::params![input.record.generation_qualification_id().digest().as_str(), input.record.operation_receipt_id().digest().as_str(), input.record.operation_policy_id().digest().as_str()], |row| row.get(0))?;
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
    if stored.status() != GenerationQualificationStatusV1::Rejected
        || input.resource_phase.manifest.status() != GenerationQualificationPhaseStatusV1::Failed
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(stored))
}
