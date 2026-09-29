use rusqlite::Connection;

use crate::StoreResult;

pub(super) fn create(connection: &Connection) -> StoreResult<()> {
    create_platform(connection)?;
    create_license(connection)
}

fn create_platform(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE generation_qualification_platform_evidence (
             generation_qualification_platform_evidence_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_qualification_platform_evidence_id) = 64)
                 CHECK(generation_qualification_platform_evidence_id =
                     lower(generation_qualification_platform_evidence_id))
                 CHECK(generation_qualification_platform_evidence_id NOT GLOB '*[^0-9a-f]*'),
             operation_policy_id TEXT NOT NULL
                 CHECK(length(operation_policy_id) = 64)
                 CHECK(operation_policy_id = lower(operation_policy_id))
                 CHECK(operation_policy_id NOT GLOB '*[^0-9a-f]*'),
             request_projection_id TEXT NOT NULL
                 CHECK(length(request_projection_id) = 64)
                 CHECK(request_projection_id = lower(request_projection_id))
                 CHECK(request_projection_id NOT GLOB '*[^0-9a-f]*'),
             target_generation_system_id TEXT NOT NULL
                 CHECK(length(target_generation_system_id) = 64)
                 CHECK(target_generation_system_id = lower(target_generation_system_id))
                 CHECK(target_generation_system_id NOT GLOB '*[^0-9a-f]*'),
             status TEXT NOT NULL CHECK(status IN ('supported', 'rejected')),
             reason TEXT NOT NULL CHECK(reason IN (
                 'reviewed_managed_linux_native_cpu',
                 'unsupported_operating_system',
                 'unsupported_architecture',
                 'unsupported_abi',
                 'unsupported_execution_class',
                 'unsupported_hardware_envelope',
                 'assessment_policy_denied'
             )),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             CHECK(
                 (status = 'supported' AND reason = 'reviewed_managed_linux_native_cpu')
                 OR (status = 'rejected' AND reason IN (
                     'unsupported_operating_system',
                     'unsupported_architecture',
                     'unsupported_abi',
                     'unsupported_execution_class',
                     'unsupported_hardware_envelope',
                     'assessment_policy_denied'
                 ))
             ),
             UNIQUE (operation_policy_id),
             UNIQUE (
                 generation_qualification_platform_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ),
             FOREIGN KEY (operation_policy_id)
                 REFERENCES generation_qualification_operation_policies(operation_policy_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (request_projection_id)
                 REFERENCES generation_qualification_request_projections(request_projection_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (target_generation_system_id)
                 REFERENCES generation_system_records(generation_system_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;",
    )?;
    Ok(())
}

fn create_license(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE generation_qualification_license_evidence (
             generation_qualification_license_evidence_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_qualification_license_evidence_id) = 64)
                 CHECK(generation_qualification_license_evidence_id =
                     lower(generation_qualification_license_evidence_id))
                 CHECK(generation_qualification_license_evidence_id NOT GLOB '*[^0-9a-f]*'),
             operation_policy_id TEXT NOT NULL
                 CHECK(length(operation_policy_id) = 64)
                 CHECK(operation_policy_id = lower(operation_policy_id))
                 CHECK(operation_policy_id NOT GLOB '*[^0-9a-f]*'),
             request_projection_id TEXT NOT NULL
                 CHECK(length(request_projection_id) = 64)
                 CHECK(request_projection_id = lower(request_projection_id))
                 CHECK(request_projection_id NOT GLOB '*[^0-9a-f]*'),
             target_generation_system_id TEXT NOT NULL
                 CHECK(length(target_generation_system_id) = 64)
                 CHECK(target_generation_system_id = lower(target_generation_system_id))
                 CHECK(target_generation_system_id NOT GLOB '*[^0-9a-f]*'),
             permission TEXT NOT NULL CHECK(permission = 'local_generation'),
             decision TEXT NOT NULL CHECK(decision IN ('local_use_only', 'rejected')),
             reason TEXT NOT NULL CHECK(reason IN (
                 'approved_local_generation',
                 'approval_policy_denied'
             )),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             CHECK(
                 (decision = 'local_use_only' AND reason = 'approved_local_generation')
                 OR (decision = 'rejected' AND reason = 'approval_policy_denied')
             ),
             UNIQUE (operation_policy_id),
             UNIQUE (
                 generation_qualification_license_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ),
             FOREIGN KEY (operation_policy_id)
                 REFERENCES generation_qualification_operation_policies(operation_policy_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (request_projection_id)
                 REFERENCES generation_qualification_request_projections(request_projection_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (target_generation_system_id)
                 REFERENCES generation_system_records(generation_system_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;",
    )?;
    Ok(())
}
