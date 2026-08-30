use super::*;

pub(super) fn sequence_with_errors(
    fixture: &Fixture,
    process_error: bool,
    native_error: bool,
) -> ManagedJudgeObservationSequence {
    ManagedJudgeObservationSequence::for_test(
        Box::new(FakeAuthority {
            process: fixture.process.clone(),
            native_load: fixture.native_load.clone(),
            worker: fixture.worker.clone(),
            worker_native_load: fixture.worker_native_load.clone(),
            model_mapping: fixture.model_mapping.clone(),
            process_error,
            native_error,
            worker_reobserve_error: false,
        }),
        schedule_id("schedule"),
        1,
        &CancellationToken::new(),
    )
    .expect("sequence")
}

pub(super) fn start_and_preflight(
    sequence: &mut ManagedJudgeObservationSequence,
    fixture: &Fixture,
) {
    begin(sequence);
    drive(sequence, 1, MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT as usize);
    let observation = sequence
        .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new())
        .expect("complete preflight observation");
    assert_eq!(observation.initial_process(), &fixture.process);
    assert_eq!(observation.post_preflight_process(), &fixture.process);
    assert_eq!(observation.final_process(), &fixture.process);
    assert_eq!(observation.native_load(), &fixture.native_load);
    assert_eq!(observation.connection_observations().len(), 8);
    assert!(format!("{observation:?}").contains("connection_observation_count: 8"));
    assert_eq!(
        observation.connection_witness(),
        observation
            .connection_observations()
            .last()
            .expect("witness")
    );
    sequence
        .seal_preflight(observation, &CancellationToken::new())
        .expect("seal preflight");
}

pub(super) fn begin(sequence: &mut ManagedJudgeObservationSequence) {
    observe(sequence, OllamaResponseObservationPhase::BeforeResponses).expect("begin");
}

pub(super) fn drive_attempt(
    sequence: &mut ManagedJudgeObservationSequence,
    cursor: u32,
    fixture: &Fixture,
) {
    let first = usize::try_from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT).expect("count")
        + usize::try_from(cursor).expect("cursor")
            * usize::try_from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT).expect("count")
        + 1;
    let worker = first + 3;
    let last = first + usize::try_from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT).expect("count") - 1;
    drive(sequence, first, worker);
    begin_worker(sequence, cursor, fixture);
    drive(sequence, worker + 1, last);
}

pub(super) fn begin_worker(
    sequence: &mut ManagedJudgeObservationSequence,
    cursor: u32,
    fixture: &Fixture,
) {
    sequence
        .begin_attempt_worker_observation(
            cursor,
            &fixture.worker_request(),
            &fixture.worker_native_request(),
            &CancellationToken::new(),
        )
        .expect("begin worker observation");
}

pub(super) fn begin_worker_result(
    sequence: &mut ManagedJudgeObservationSequence,
    cursor: u32,
    fixture: &Fixture,
) -> Result<(), ManagedJudgeObservationError> {
    sequence.begin_attempt_worker_observation(
        cursor,
        &fixture.worker_request(),
        &fixture.worker_native_request(),
        &CancellationToken::new(),
    )
}

pub(super) fn drive(sequence: &mut ManagedJudgeObservationSequence, first: usize, last: usize) {
    for ordinal in first..=last {
        observe(
            sequence,
            OllamaResponseObservationPhase::AfterResponse { ordinal },
        )
        .expect("observe response");
    }
}

pub(super) fn observe(
    sequence: &mut ManagedJudgeObservationSequence,
    phase: OllamaResponseObservationPhase,
) -> Result<(), ManagedJudgeObservationError> {
    sequence.observe_phase(phase, connection(), &CancellationToken::new())
}

pub(super) fn complete_observation(
    sequence: &mut ManagedJudgeObservationSequence,
    cursor: u32,
    fixture: &Fixture,
) -> ManagedJudgeAttemptObservation {
    complete_observation_result(sequence, cursor, fixture).expect("complete observation")
}

pub(super) fn complete_observation_result(
    sequence: &mut ManagedJudgeObservationSequence,
    cursor: u32,
    fixture: &Fixture,
) -> Result<ManagedJudgeAttemptObservation, ManagedJudgeObservationError> {
    sequence.complete_attempt_observation(
        cursor,
        &fixture.native_request(),
        &CancellationToken::new(),
    )
}

pub(super) fn expected_preflight_digest(process: &AttachedProcessEvidence) -> Digest {
    let evidence = connection_evidence(process).expect("connection evidence");
    let responses = vec![
        evidence.clone();
        usize::try_from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT).expect("count")
    ];
    connection_span_digest(
        &schedule_id("schedule"),
        0,
        0,
        1,
        u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT),
        &evidence,
        &responses,
    )
}

pub(super) fn connection() -> RetainedTcpConnection {
    RetainedTcpConnection::new(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 41_001),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 11_434),
    )
    .expect("connection")
}

pub(super) fn connection_evidence(
    process: &AttachedProcessEvidence,
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
    RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
        attribution_kind: TcpConnectionAttributionKind::WindowsContextBindingPid,
        sharing_limitation: TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable,
        process_evidence_digest: process.evidence_digest().clone(),
        platform_connection_digest: digest("connection"),
    })
}

pub(super) fn process(tag: &str) -> AttachedProcessEvidence {
    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::WindowsOwnerPidProcessHandle,
        owner_pid: 42,
        process_instance_digest: digest(&format!("{tag}-instance")),
        ownership_snapshot_digest: digest(&format!("{tag}-ownership")),
        entrypoint_object_digest: digest(&format!("{tag}-object")),
        entrypoint_digest: digest(&format!("{tag}-entrypoint")),
        entrypoint_bytes: 16,
        platform_evidence_digest: digest(&format!("{tag}-platform")),
    })
    .expect("process")
}

pub(super) fn package() -> RuntimePackageManifest {
    let entrypoint_path = ArtifactSetRelativePath::new("bin/runtime").expect("entrypoint path");
    let worker_path = ArtifactSetRelativePath::new("bin/worker").expect("worker path");
    let evidence_path = ArtifactSetRelativePath::new("legal/evidence").expect("evidence path");
    let entrypoint = ArtifactId::from_digest(digest("entrypoint"));
    let worker = ArtifactId::from_digest(Digest::sha256(b"worker"));
    let evidence = ArtifactId::from_digest(digest("evidence"));
    let artifact_set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(entrypoint.clone(), 10, entrypoint_path.clone()),
        ArtifactSetMember::new(worker.clone(), 6, worker_path.clone()),
        ArtifactSetMember::new(evidence.clone(), 8, evidence_path.clone()),
    ])
    .expect("artifact set");
    RuntimePackageManifest::new(
        &artifact_set,
        "managed-judge-test",
        "1.0.0",
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("target"),
        PackageSource::new(
            PackageSourceKind::LocalArchive,
            "local:managed-judge-test",
            "1.0.0",
            digest("source"),
        )
        .expect("source"),
        PackageTransformation::Untransformed {
            evidence_digest: digest("transformation"),
        },
        vec![
            RuntimePackageMember::new(
                entrypoint,
                10,
                entrypoint_path,
                vec![RuntimePackageMemberRole::Entrypoint],
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            RuntimePackageMember::new(
                worker,
                6,
                worker_path,
                vec![RuntimePackageMemberRole::WorkerExecutable],
                RuntimePackageLoadPolicy::BackendConditional,
            ),
            RuntimePackageMember::new(
                evidence,
                8,
                evidence_path,
                vec![
                    RuntimePackageMemberRole::LicenseText,
                    RuntimePackageMemberRole::ProvenanceRecord,
                ],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
        ],
    )
    .expect("package")
}

pub(super) fn schedule_id(tag: &str) -> CandidateJudgeScheduleId {
    serde_json::from_value(serde_json::Value::String(digest(tag).as_str().to_owned()))
        .expect("schedule id")
}

pub(super) fn digest(value: &str) -> Digest {
    Digest::sha256(value.as_bytes())
}

pub(super) fn runtime_identity() -> RuntimeIdentity {
    RuntimeIdentity {
        backend: "ollama_native".to_owned(),
        version: "0.32.15".to_owned(),
        digest: Some(digest("runtime")),
    }
}

pub(super) fn running_model() -> OllamaRunningModel {
    OllamaRunningModel {
        reference: "fixture:latest".to_owned(),
        inventory_digest: digest("inventory"),
        byte_size: 4_096,
        accelerator_bytes: 0,
        context_tokens: 4_096,
    }
}

pub(super) fn preflight(running: &OllamaRunningModel) -> OllamaPreflight {
    OllamaPreflight {
        runtime: runtime_identity(),
        inventory: vec![OllamaInventoryEntry {
            reference: running.reference.clone(),
            inventory_digest: running.inventory_digest.clone(),
            byte_size: running.byte_size,
        }],
        bindings: vec![OllamaPreflightBinding {
            reference: running.reference.clone(),
            inventory_digest: running.inventory_digest.clone(),
            details: OllamaModelDetails {
                format: "gguf".to_owned(),
                family: "llama".to_owned(),
                quantization: "Q4_K_M".to_owned(),
                capabilities: vec!["completion".to_owned()],
                license_digest: digest("license"),
                template_digest: digest("template"),
                metadata_digest: digest("metadata"),
            },
        }],
        running: vec![running.clone()],
    }
}
