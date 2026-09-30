use rewrite_model::{
    GenerationQualificationSelectionV1, GenerationQualificationSelectionV1Relations,
};

use super::super::{
    GenerationQualificationSelectionV1Input, GenerationQualificationSelectionV1ReadInput,
    GenerationQualificationSelectionV1TransactionError,
};
use crate::store::generation_qualification_record::tests::support as qualification;
use crate::{ArtifactStateStore, StoreError, WriteDisposition};

pub(super) const TABLE: &str = "generation_qualification_selections";

pub(super) struct Session {
    pub(super) parents: qualification::Session,
    pub(super) selection: GenerationQualificationSelectionV1,
}

pub(super) fn session() -> Session {
    open(true)
}

pub(super) fn without_qualification() -> Session {
    open(false)
}

pub(super) fn read_input(session: &Session) -> GenerationQualificationSelectionV1ReadInput<'_> {
    GenerationQualificationSelectionV1ReadInput {
        selection: &session.selection,
    }
}

pub(super) fn commit(session: &mut Session) -> WriteDisposition {
    session
        .parents
        .store
        .transact_generation_qualification_selection_v1(
            GenerationQualificationSelectionV1Input {
                selection: &session.selection,
            },
            || Ok::<_, ()>(()),
        )
        .expect("selection")
}

pub(super) fn count(store: &ArtifactStateStore, table: &str) -> i64 {
    store
        .connection()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count")
}

pub(super) fn canonical_json(session: &Session) -> Vec<u8> {
    session
        .parents
        .store
        .connection()
        .query_row(&format!("SELECT canonical_json FROM {TABLE}"), [], |row| {
            row.get(0)
        })
        .expect("canonical json")
}

pub(super) fn qualification_status(session: &Session) -> String {
    session
        .parents
        .store
        .connection()
        .query_row(
            "SELECT status FROM generation_qualification_records",
            [],
            |row| row.get(0),
        )
        .expect("qualification status")
}

pub(super) fn qualification_json(session: &Session) -> Vec<u8> {
    session
        .parents
        .store
        .connection()
        .query_row(
            "SELECT canonical_json FROM generation_qualification_records",
            [],
            |row| row.get(0),
        )
        .expect("qualification json")
}

pub(super) fn expect_store(
    store: &mut ArtifactStateStore,
    selection: &GenerationQualificationSelectionV1,
    expected: &StoreError,
) {
    let error = store
        .transact_generation_qualification_selection_v1(
            GenerationQualificationSelectionV1Input { selection },
            || Ok::<_, ()>(()),
        )
        .expect_err("refused write");
    assert_eq!(
        format!("{error:?}"),
        "GenerationQualificationSelectionV1TransactionError::Store"
    );
    assert_eq!(
        error.to_string(),
        "generation qualification selection storage failed"
    );
    assert!(matches!(
        error,
        GenerationQualificationSelectionV1TransactionError::Store(value)
            if same_store(&value, expected)
    ));
}

pub(super) fn expect_read(
    store: &ArtifactStateStore,
    selection: &GenerationQualificationSelectionV1,
    expected: &StoreError,
) {
    let error = store
        .generation_qualification_selection_v1(GenerationQualificationSelectionV1ReadInput {
            selection,
        })
        .expect_err("refused read");
    assert!(same_store(&error, expected));
}

fn open(persist_qualification: bool) -> Session {
    let mut parents = qualification::session();
    if persist_qualification {
        qualification::commit(&mut parents);
    }
    let selection =
        GenerationQualificationSelectionV1::new(&GenerationQualificationSelectionV1Relations {
            qualification_record: &parents.record,
        })
        .expect("selection");
    Session { parents, selection }
}

fn same_store(actual: &StoreError, expected: &StoreError) -> bool {
    std::mem::discriminant(actual) == std::mem::discriminant(expected)
}
