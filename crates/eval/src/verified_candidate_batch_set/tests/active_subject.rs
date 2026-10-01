use rewrite_inference::GenerationCandidate;
use rewrite_model::{
    CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptV1,
    GenerationResourceAttemptResultRecordV1,
};
use rewrite_types::CancellationToken;

use super::support::{OfflineBatch, OfflineBatchError, scenario};
use super::{CoreError, Relationship, RetainedCandidateBatch, VerifiedCandidateBatchSetCore};
use crate::active_generation_qualification_subject::{
    ActiveGenerationQualificationBinding, ActiveGenerationQualificationSubject,
};

struct BoundBatch {
    inner: OfflineBatch,
    binding: ActiveGenerationQualificationBinding,
}

/// Private synthetic adapter, never issued through a shipping authority constructor.
pub(crate) fn bound_synthetic_set(
    fixture: super::support::Scenario,
    subject: &ActiveGenerationQualificationSubject,
) -> crate::VerifiedCandidateBatchSet {
    let batches = fixture
        .batches
        .into_iter()
        .map(|inner| BoundBatch {
            inner,
            binding: subject.binding(),
        })
        .collect();
    let core =
        VerifiedCandidateBatchSetCore::verify(fixture.input, batches, &CancellationToken::new())
            .expect("synthetic exact bound batch set");
    super::super::erase_core(core, super::map_offline_error)
}

impl RetainedCandidateBatch for BoundBatch {
    type Error = OfflineBatchError;

    fn active_binding(&self) -> Option<&ActiveGenerationQualificationBinding> {
        Some(&self.binding)
    }

    fn receipt(&self) -> &CandidateGenerationReceiptV1 {
        self.inner.receipt()
    }

    fn attempt_record(&self) -> &CandidateGenerationAttemptRecordV1 {
        self.inner.attempt_record()
    }

    fn resource_result(&self) -> Option<&GenerationResourceAttemptResultRecordV1> {
        self.inner.resource_result()
    }

    fn candidate_count(&self) -> usize {
        self.inner.candidate_count()
    }

    fn candidate(
        &self,
        ordinal: u8,
        cancellation: &CancellationToken,
    ) -> Result<Option<&GenerationCandidate>, Self::Error> {
        self.inner.candidate(ordinal, cancellation)
    }

    fn revalidate(&self, cancellation: &CancellationToken) -> Result<(), Self::Error> {
        self.inner.revalidate(cancellation)
    }
}

#[test]
fn batches_must_share_one_active_subject_or_remain_all_offline() {
    let exact = scenario("same-active");
    let subject = ActiveGenerationQualificationSubject::new();
    let batches = exact
        .batches
        .into_iter()
        .map(|inner| BoundBatch {
            inner,
            binding: subject.binding(),
        })
        .collect();
    let set =
        VerifiedCandidateBatchSetCore::verify(exact.input, batches, &CancellationToken::new())
            .expect("one Active subject is retained");
    assert!(
        set.active_binding
            .as_ref()
            .is_some_and(|binding| subject.accepts(binding))
    );

    let replay = scenario("cross-active");
    let first = ActiveGenerationQualificationSubject::new();
    let second = ActiveGenerationQualificationSubject::new();
    let mut batches = replay.batches.into_iter();
    let mixed = vec![
        BoundBatch {
            inner: batches.next().expect("first batch"),
            binding: first.binding(),
        },
        BoundBatch {
            inner: batches.next().expect("second batch"),
            binding: second.binding(),
        },
    ];
    assert!(matches!(
        VerifiedCandidateBatchSetCore::verify(replay.input, mixed, &CancellationToken::new(),),
        Err(CoreError::Relationship(Relationship::ActiveSubjectClosure))
    ));
}
