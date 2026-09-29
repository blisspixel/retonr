use rewrite_model::{
    GenerationHumanAdjudicationPolicyDenialRecordV1,
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations, GenerationQualificationPhaseScopeV1,
    GenerationResourcePolicyDenialRecordV1, GenerationResourcePolicyDenialRecordV1Relations,
};
use rewrite_types::Digest;

use super::super::{
    GenerationHumanAdjudicationPolicyDenialV1Input,
    GenerationHumanAdjudicationPolicyDenialV1ReadInput, GenerationResourcePolicyDenialV1Input,
    GenerationResourcePolicyDenialV1ReadInput,
};
use crate::store::generation_qualification_preregistration::tests::support::{self, Fixture};
use crate::{ArtifactStateStore, WriteDisposition};

pub(super) const RESOURCE_TABLE: &str = "generation_resource_policy_denial_records";
pub(super) const HUMAN_TABLE: &str = "generation_human_adjudication_policy_denial_records";

pub(super) struct Prepared {
    pub(super) fixture: Fixture,
    pub(super) policy: Digest,
    pub(super) resource: GenerationResourcePolicyDenialRecordV1,
    pub(super) human: GenerationHumanAdjudicationPolicyDenialRecordV1,
}

pub(super) struct Session {
    pub(super) _directory: tempfile::TempDir,
    pub(super) store: ArtifactStateStore,
    pub(super) prepared: Prepared,
}

pub(super) fn prepared() -> Prepared {
    let fixture = support::fixture();
    assert!(
        fixture
            .plan
            .generation_system_ids()
            .contains(fixture.systems[0].generation_system_id()),
        "target system belongs to the plan"
    );
    assert_eq!(
        fixture.plan.suite_manifest_id(),
        fixture.suite.suite_manifest_id()
    );
    let policy = Digest::sha256(b"denied phase policy");
    let resource =
        GenerationResourcePolicyDenialRecordV1::new(resource_relations(&fixture, &policy))
            .expect("resource denial");
    let human =
        GenerationHumanAdjudicationPolicyDenialRecordV1::new(human_relations(&fixture, &policy))
            .expect("human denial");
    Prepared {
        fixture,
        policy,
        resource,
        human,
    }
}

pub(super) fn session() -> Session {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let prepared = prepared();
    support::persist_plan_foundation(&mut store, &prepared.fixture);
    Session {
        _directory: directory,
        store,
        prepared,
    }
}

pub(super) fn resource_relations<'a>(
    fixture: &'a Fixture,
    policy: &'a Digest,
) -> GenerationResourcePolicyDenialRecordV1Relations<'a> {
    GenerationResourcePolicyDenialRecordV1Relations {
        scope: scope(fixture),
        phase_policy_digest: policy,
    }
}

pub(super) fn human_relations<'a>(
    fixture: &'a Fixture,
    policy: &'a Digest,
) -> GenerationHumanAdjudicationPolicyDenialRecordV1Relations<'a> {
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations {
        scope: scope(fixture),
        phase_policy_digest: policy,
    }
}

pub(super) fn resource_input(prepared: &Prepared) -> GenerationResourcePolicyDenialV1Input<'_> {
    GenerationResourcePolicyDenialV1Input {
        record: &prepared.resource,
        relations: resource_relations(&prepared.fixture, &prepared.policy),
    }
}

pub(super) fn human_input(
    prepared: &Prepared,
) -> GenerationHumanAdjudicationPolicyDenialV1Input<'_> {
    GenerationHumanAdjudicationPolicyDenialV1Input {
        record: &prepared.human,
        relations: human_relations(&prepared.fixture, &prepared.policy),
    }
}

pub(super) fn resource_read(prepared: &Prepared) -> GenerationResourcePolicyDenialV1ReadInput<'_> {
    GenerationResourcePolicyDenialV1ReadInput {
        relations: resource_relations(&prepared.fixture, &prepared.policy),
    }
}

pub(super) fn human_read(
    prepared: &Prepared,
) -> GenerationHumanAdjudicationPolicyDenialV1ReadInput<'_> {
    GenerationHumanAdjudicationPolicyDenialV1ReadInput {
        relations: human_relations(&prepared.fixture, &prepared.policy),
    }
}

pub(super) fn commit_resource(session: &mut Session) -> WriteDisposition {
    session
        .store
        .transact_generation_resource_policy_denial_v1(resource_input(&session.prepared), || {
            Ok::<_, ()>(())
        })
        .expect("resource denial")
}

pub(super) fn commit_human(session: &mut Session) -> WriteDisposition {
    session
        .store
        .transact_generation_human_adjudication_policy_denial_v1(
            human_input(&session.prepared),
            || Ok::<_, ()>(()),
        )
        .expect("human denial")
}

pub(super) fn count(store: &ArtifactStateStore, table: &str) -> i64 {
    store
        .connection()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count")
}

fn scope(fixture: &Fixture) -> GenerationQualificationPhaseScopeV1<'_> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.systems[0],
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
    }
}
