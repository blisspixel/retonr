use rusqlite::Connection;

use crate::StoreResult;

const PLAN_SQL: &str = "CREATE TABLE candidate_judge_plans (
    candidate_judge_plan_id TEXT PRIMARY KEY NOT NULL
        CHECK(length(candidate_judge_plan_id) = 64)
        CHECK(candidate_judge_plan_id = lower(candidate_judge_plan_id))
        CHECK(candidate_judge_plan_id NOT GLOB '*[^0-9a-f]*'),
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    generation_qualification_plan_id TEXT NOT NULL
        CHECK(length(generation_qualification_plan_id) = 64)
        CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
        CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
    generation_suite_manifest_id TEXT NOT NULL
        CHECK(length(generation_suite_manifest_id) = 64)
        CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
        CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
    generation_repetition_id TEXT NOT NULL
        CHECK(length(generation_repetition_id) = 64)
        CHECK(generation_repetition_id = lower(generation_repetition_id))
        CHECK(generation_repetition_id NOT GLOB '*[^0-9a-f]*'),
    candidate_selection_policy_id TEXT NOT NULL
        CHECK(length(candidate_selection_policy_id) = 64)
        CHECK(candidate_selection_policy_id = lower(candidate_selection_policy_id))
        CHECK(candidate_selection_policy_id NOT GLOB '*[^0-9a-f]*'),
    candidate_a_generation_system_id TEXT NOT NULL
        CHECK(length(candidate_a_generation_system_id) = 64)
        CHECK(candidate_a_generation_system_id = lower(candidate_a_generation_system_id))
        CHECK(candidate_a_generation_system_id NOT GLOB '*[^0-9a-f]*'),
    candidate_b_generation_system_id TEXT NOT NULL
        CHECK(length(candidate_b_generation_system_id) = 64)
        CHECK(candidate_b_generation_system_id = lower(candidate_b_generation_system_id))
        CHECK(candidate_b_generation_system_id NOT GLOB '*[^0-9a-f]*'),
    judge_generation_system_id TEXT NOT NULL
        CHECK(length(judge_generation_system_id) = 64)
        CHECK(judge_generation_system_id = lower(judge_generation_system_id))
        CHECK(judge_generation_system_id NOT GLOB '*[^0-9a-f]*'),
    case_material_set_digest TEXT NOT NULL
        CHECK(length(case_material_set_digest) = 64)
        CHECK(case_material_set_digest = lower(case_material_set_digest))
        CHECK(case_material_set_digest NOT GLOB '*[^0-9a-f]*'),
    rubric_digest TEXT NOT NULL
        CHECK(length(rubric_digest) = 64)
        CHECK(rubric_digest = lower(rubric_digest))
        CHECK(rubric_digest NOT GLOB '*[^0-9a-f]*'),
    prompt_contract_digest TEXT NOT NULL
        CHECK(length(prompt_contract_digest) = 64)
        CHECK(prompt_contract_digest = lower(prompt_contract_digest))
        CHECK(prompt_contract_digest NOT GLOB '*[^0-9a-f]*'),
    output_schema_digest TEXT NOT NULL
        CHECK(length(output_schema_digest) = 64)
        CHECK(output_schema_digest = lower(output_schema_digest))
        CHECK(output_schema_digest NOT GLOB '*[^0-9a-f]*'),
    case_count INTEGER NOT NULL CHECK(case_count BETWEEN 1 AND 256),
    presentation_seed TEXT NOT NULL
        CHECK(length(presentation_seed) BETWEEN 1 AND 20)
        CHECK(presentation_seed NOT GLOB '*[^0-9]*')
        CHECK(presentation_seed = '0' OR substr(presentation_seed, 1, 1) <> '0'),
    order_policy TEXT NOT NULL CHECK(order_policy = 'both_orders'),
    attempts_per_order INTEGER NOT NULL CHECK(attempts_per_order = 1),
    maximum_judge_cases INTEGER NOT NULL CHECK(maximum_judge_cases BETWEEN 1 AND 256),
    maximum_source_bytes INTEGER NOT NULL CHECK(maximum_source_bytes BETWEEN 1 AND 1048576),
    maximum_candidate_bytes INTEGER NOT NULL
        CHECK(maximum_candidate_bytes BETWEEN 1 AND 1048576),
    maximum_complete_input_bytes INTEGER NOT NULL
        CHECK(maximum_complete_input_bytes BETWEEN 1 AND 4194304),
    maximum_context_tokens INTEGER NOT NULL
        CHECK(maximum_context_tokens BETWEEN 1 AND 131072),
    maximum_output_tokens INTEGER NOT NULL CHECK(maximum_output_tokens BETWEEN 1 AND 8192),
    maximum_response_bytes INTEGER NOT NULL
        CHECK(maximum_response_bytes BETWEEN 256 AND 65536),
    maximum_elapsed_milliseconds INTEGER NOT NULL
        CHECK(maximum_elapsed_milliseconds BETWEEN 1 AND 3600000),
    canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 4194304),
    CHECK(case_count <= maximum_judge_cases),
    CHECK(
        maximum_complete_input_bytes >= maximum_source_bytes
        AND maximum_complete_input_bytes >= maximum_candidate_bytes
    ),
    CHECK(
        candidate_a_generation_system_id != candidate_b_generation_system_id
        AND judge_generation_system_id != candidate_a_generation_system_id
        AND judge_generation_system_id != candidate_b_generation_system_id
    ),
    UNIQUE (candidate_judge_plan_id, judge_generation_system_id),
    UNIQUE (candidate_judge_plan_id, case_count, presentation_seed),
    UNIQUE (
        candidate_judge_plan_id,
        candidate_a_generation_system_id,
        candidate_b_generation_system_id,
        judge_generation_system_id,
        case_material_set_digest
    ),
    FOREIGN KEY (generation_qualification_plan_id)
        REFERENCES generation_qualification_plans(generation_qualification_plan_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (generation_suite_manifest_id)
        REFERENCES generation_suite_manifests(generation_suite_manifest_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (generation_repetition_id)
        REFERENCES generation_repetition_records(generation_repetition_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (generation_qualification_plan_id, generation_repetition_id)
        REFERENCES generation_qualification_plan_repetitions(
            generation_qualification_plan_id,
            generation_repetition_id
        ) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (candidate_selection_policy_id)
        REFERENCES candidate_selection_policies(candidate_selection_policy_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (candidate_a_generation_system_id)
        REFERENCES generation_system_records(generation_system_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (candidate_b_generation_system_id)
        REFERENCES generation_system_records(generation_system_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (judge_generation_system_id)
        REFERENCES generation_system_records(generation_system_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (generation_qualification_plan_id, candidate_a_generation_system_id)
        REFERENCES generation_qualification_plan_systems(
            generation_qualification_plan_id,
            generation_system_id
        ) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (generation_qualification_plan_id, candidate_b_generation_system_id)
        REFERENCES generation_qualification_plan_systems(
            generation_qualification_plan_id,
            generation_system_id
        ) ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;";

pub(super) fn create(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(PLAN_SQL)?;
    Ok(())
}
