use serde_json::json;

use super::*;

fn rubric() -> LocalJudgeRubric {
    LocalJudgeRubric {
        schema_version: LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION,
        clauses: vec![
            LocalJudgeRubricClause {
                id: "clarity".to_owned(),
                instruction: "Prefer precise, readable prose.".to_owned(),
            },
            LocalJudgeRubricClause {
                id: "meaning".to_owned(),
                instruction: "Prefer the candidate that preserves meaning.".to_owned(),
            },
        ],
    }
}

#[test]
fn parses_canonical_rubric_and_has_stable_sensitive_digest() {
    let value = rubric();
    let encoded = serde_json::to_string(&value).expect("encode rubric");
    assert_eq!(parse_local_judge_rubric(&encoded), Ok(value.clone()));
    let first = local_judge_rubric_digest(&value).expect("digest rubric");
    let mut changed = value;
    changed.clauses[0].instruction.push_str(" Exactly.");
    assert_ne!(
        first,
        local_judge_rubric_digest(&changed).expect("digest changed rubric")
    );
}

#[test]
fn rejects_size_json_version_and_unknown_fields() {
    assert_eq!(
        parse_local_judge_rubric(&" ".repeat(MAX_LOCAL_JUDGE_RUBRIC_BYTES + 1)),
        Err(LocalJudgeRubricError::TooLarge)
    );
    for input in ["", "{", "[]", "{}", "null"] {
        assert_eq!(
            parse_local_judge_rubric(input),
            Err(LocalJudgeRubricError::InvalidJson)
        );
    }
    assert_eq!(
        parse_local_judge_rubric(
            &json!({"schema_version": 2, "clauses": rubric().clauses}).to_string()
        ),
        Err(LocalJudgeRubricError::UnsupportedSchema(2))
    );
    assert_eq!(
        parse_local_judge_rubric(
            &json!({"schema_version": 1, "clauses": [], "extra": true}).to_string()
        ),
        Err(LocalJudgeRubricError::InvalidJson)
    );
}

#[test]
fn rejects_noncanonical_clause_sets_and_instructions() {
    let mut fixtures = Vec::new();
    let mut empty = rubric();
    empty.clauses.clear();
    fixtures.push(empty);
    let mut reversed = rubric();
    reversed.clauses.reverse();
    fixtures.push(reversed);
    let mut duplicate = rubric();
    duplicate.clauses[1].id = duplicate.clauses[0].id.clone();
    fixtures.push(duplicate);
    let mut invalid_id = rubric();
    invalid_id.clauses[0].id = "Clarity".to_owned();
    fixtures.push(invalid_id);
    for instruction in ["", " leading", "trailing ", "contains\nnewline"] {
        let mut invalid = rubric();
        invalid.clauses[0].instruction = instruction.to_owned();
        fixtures.push(invalid);
    }
    for invalid in fixtures {
        assert_eq!(
            local_judge_rubric_digest(&invalid),
            Err(LocalJudgeRubricError::InvalidClauses)
        );
    }
    let mut too_many = rubric();
    too_many.clauses = (0..=MAX_LOCAL_JUDGE_RUBRIC_CLAUSES)
        .map(|index| LocalJudgeRubricClause {
            id: format!("clause-{index:02}"),
            instruction: "Bounded instruction.".to_owned(),
        })
        .collect();
    assert_eq!(
        local_judge_rubric_digest(&too_many),
        Err(LocalJudgeRubricError::InvalidClauses)
    );
}
