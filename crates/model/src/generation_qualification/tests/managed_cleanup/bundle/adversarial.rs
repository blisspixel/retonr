use super::*;

#[test]
fn candidate_constructor_rejects_foreign_relationships_ordinals_and_bytes() {
    let fixture = managed_fixture(0);
    let foreign = managed_fixture(1);
    let build = |precursor, planned, case, ordinal, bytes: &[u8]| {
        CandidateArtifactEntryV1::new(
            precursor,
            planned,
            case,
            ordinal,
            path("candidate.txt"),
            bytes,
        )
    };
    for result in [
        build(
            &fixture.precursor,
            &foreign.base.attempts[1],
            &fixture.base.cases[0],
            0,
            b"candidate",
        ),
        build(
            &fixture.precursor,
            &fixture.base.attempts[0],
            &fixture.base.cases[1],
            0,
            b"candidate",
        ),
        build(
            &fixture.precursor,
            &fixture.base.attempts[0],
            &fixture.base.cases[0],
            2,
            b"candidate",
        ),
        build(
            &fixture.precursor,
            &fixture.base.attempts[0],
            &fixture.base.cases[0],
            0,
            &[0xff],
        ),
        build(
            &fixture.precursor,
            &fixture.base.attempts[0],
            &fixture.base.cases[0],
            0,
            &vec![b'a'; 1_025],
        ),
    ] {
        assert_eq!(
            result,
            Err(GenerationQualificationContractError::InvalidCandidateArtifact)
        );
    }
}

#[test]
fn bundle_rejects_order_duplicate_role_candidate_and_size_failures() {
    let fixture = bundle_fixture();
    let relations = fixture.relations();
    let mut reordered = valid_entries(&fixture.managed, &fixture.cleanup, &fixture.candidates);
    reordered.swap(0, 1);
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::new(
            relations,
            reordered,
            fixture.candidates.clone()
        ),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
    );
    let mut duplicate = valid_entries(&fixture.managed, &fixture.cleanup, &fixture.candidates);
    duplicate[1] = CandidateGenerationEvidenceBundleEntryV1::new(
        duplicate[0].relative_path().clone(),
        duplicate[1].role(),
        duplicate[1].artifact_id().clone(),
        duplicate[1].byte_size(),
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::new(
            relations,
            duplicate,
            fixture.candidates.clone()
        ),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
    );
    let mut missing = valid_entries(&fixture.managed, &fixture.cleanup, &fixture.candidates);
    missing.remove(2);
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::new(
            relations,
            missing,
            fixture.candidates.clone()
        ),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure)
    );
    let mut candidate_changed =
        valid_entries(&fixture.managed, &fixture.cleanup, &fixture.candidates);
    let index = candidate_changed
        .iter()
        .position(|entry| entry.role() == CandidateGenerationEvidenceBundleRoleV1::Candidate)
        .expect("candidate");
    candidate_changed[index] = CandidateGenerationEvidenceBundleEntryV1::new(
        candidate_changed[index].relative_path().clone(),
        CandidateGenerationEvidenceBundleRoleV1::Candidate,
        ArtifactId::from_digest(digest("changed")),
        candidate_changed[index].byte_size(),
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::new(
            relations,
            candidate_changed,
            fixture.candidates.clone()
        ),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure)
    );
    let mut oversized = valid_entries(&fixture.managed, &fixture.cleanup, &fixture.candidates);
    oversized[0] = CandidateGenerationEvidenceBundleEntryV1::new(
        oversized[0].relative_path().clone(),
        oversized[0].role(),
        oversized[0].artifact_id().clone(),
        MAX_GENERATION_EVIDENCE_BUNDLE_BYTES + 1,
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::new(
            relations,
            oversized,
            fixture.candidates.clone()
        ),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
    );
}

#[test]
fn bundle_rejects_every_required_role_artifact_substitution() {
    let fixture = bundle_fixture();
    for role in [
        CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
        CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
        CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
        CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
        CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
    ] {
        let mut entries = valid_entries(&fixture.managed, &fixture.cleanup, &fixture.candidates);
        let index = entries
            .iter()
            .position(|entry| entry.role() == role)
            .expect("required role");
        entries[index] = CandidateGenerationEvidenceBundleEntryV1::new(
            entries[index].relative_path().clone(),
            role,
            ArtifactId::from_digest(digest(&format!("substituted {role:?}"))),
            entries[index].byte_size(),
        );
        assert_eq!(
            CandidateGenerationEvidenceBundleManifestV1::new(
                fixture.relations(),
                entries,
                fixture.candidates.clone()
            ),
            Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure)
        );
    }
    assert_eq!(
        StructuredResponseArtifactV1Input::new(ArtifactId::from_digest(digest("empty")), 0),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
    );
}

#[test]
fn bundle_rejects_case_fold_prefix_and_reserved_manifest_collisions() {
    let fixture = bundle_fixture();
    let check = |extra: Vec<CandidateGenerationEvidenceBundleEntryV1>| {
        let mut entries = valid_entries(&fixture.managed, &fixture.cleanup, &fixture.candidates);
        entries.extend(extra);
        entries.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
        CandidateGenerationEvidenceBundleManifestV1::new(
            fixture.relations(),
            entries,
            fixture.candidates.clone(),
        )
    };
    assert_eq!(
        check(vec![
            entry(
                "Auxiliary/one.bin",
                CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
                b"one"
            ),
            entry(
                "auxiliary/ONE.bin",
                CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
                b"two"
            )
        ]),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
    );
    assert_eq!(
        check(vec![entry(
            "records",
            CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
            b"prefix"
        )]),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
    );
    for reserved in [
        EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH,
        "bundle-manifest.json/hidden",
        "BUNDLE-MANIFEST.JSON",
    ] {
        assert_eq!(
            check(vec![entry(
                reserved,
                CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
                b"reserved"
            )]),
            Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
        );
    }
}

#[test]
fn bundle_decoder_rejects_untrusted_and_substituted_json() {
    let fixture = bundle_fixture();
    let encoded = serde_json::to_vec(&fixture.bundle).expect("bundle JSON");
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES + 1],
            fixture.relations(),
            &fixture.candidates
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            b"{",
            fixture.relations(),
            &fixture.candidates
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let future = replace_once(&encoded, "\"schema_version\":1", "\"schema_version\":2");
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &future,
            fixture.relations(),
            &[]
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    let mut spaced = encoded.clone();
    spaced.push(b' ');
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &spaced,
            fixture.relations(),
            &fixture.candidates
        ),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
    let duplicate = replace_once(
        &encoded,
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &duplicate,
            fixture.relations(),
            &fixture.candidates
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let unknown = replace_once(
        &encoded,
        "{\"schema_version\":1",
        "{\"unknown\":0,\"schema_version\":1",
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &unknown,
            fixture.relations(),
            &fixture.candidates,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
}

#[test]
fn manifest_preflight_rejects_invalid_encodings_before_relationship_work() {
    let fixture = bundle_fixture();
    let encoded = serde_json::to_vec(&fixture.bundle).expect("bundle JSON");
    let mut trailing = encoded.clone();
    trailing.push(b' ');
    for (bytes, expected) in [
        (
            vec![b' '; MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES + 1],
            GenerationQualificationContractError::EncodedRecordTooLarge,
        ),
        (
            b"{".to_vec(),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            replace_once(&encoded, "\"schema_version\":1", "\"schema_version\":2"),
            GenerationQualificationContractError::UnsupportedSchema(2),
        ),
        (
            replace_once(
                &encoded,
                "\"schema_version\":1",
                "\"schema_version\":1,\"unknown\":true",
            ),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            replace_once(
                &encoded,
                "\"schema_version\":1",
                "\"schema_version\":1,\"schema_version\":1",
            ),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            trailing,
            GenerationQualificationContractError::NonCanonicalEncoding,
        ),
    ] {
        assert_eq!(
            CandidateGenerationEvidenceBundleManifestV1::preflight_json_bytes(&bytes)
                .expect_err("invalid manifest preflight"),
            expected,
        );
    }
}

#[test]
fn manifest_preflight_requires_one_nonempty_structured_response_entry() {
    let fixture = bundle_fixture();
    let encoded = serde_json::to_vec(&fixture.bundle).expect("bundle JSON");
    let response = fixture
        .bundle
        .entries()
        .iter()
        .find(|entry| entry.role() == CandidateGenerationEvidenceBundleRoleV1::StructuredResponse)
        .expect("response entry");
    let candidate = fixture
        .bundle
        .entries()
        .iter()
        .find(|entry| entry.role() == CandidateGenerationEvidenceBundleRoleV1::Candidate)
        .expect("candidate entry");
    let encode_entry = |entry: &CandidateGenerationEvidenceBundleEntryV1| {
        String::from_utf8(serde_json::to_vec(entry).expect("entry JSON")).expect("UTF-8 entry")
    };
    let replace_entry =
        |original: &CandidateGenerationEvidenceBundleEntryV1,
         changed: CandidateGenerationEvidenceBundleEntryV1| {
            replace_once(&encoded, &encode_entry(original), &encode_entry(&changed))
        };

    let missing = replace_entry(
        response,
        CandidateGenerationEvidenceBundleEntryV1::new(
            response.relative_path().clone(),
            CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
            response.artifact_id().clone(),
            response.byte_size(),
        ),
    );
    let missing = CandidateGenerationEvidenceBundleManifestV1::preflight_json_bytes(&missing)
        .expect("canonical missing-role metadata");
    assert_eq!(
        missing.structured_response_artifact(),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure),
    );

    let duplicate = replace_entry(
        candidate,
        CandidateGenerationEvidenceBundleEntryV1::new(
            candidate.relative_path().clone(),
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            candidate.artifact_id().clone(),
            candidate.byte_size(),
        ),
    );
    let duplicate = CandidateGenerationEvidenceBundleManifestV1::preflight_json_bytes(&duplicate)
        .expect("canonical duplicate-role metadata");
    assert_eq!(
        duplicate.structured_response_artifact(),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure),
    );

    let empty = replace_entry(
        response,
        CandidateGenerationEvidenceBundleEntryV1::new(
            response.relative_path().clone(),
            response.role(),
            response.artifact_id().clone(),
            0,
        ),
    );
    let empty = CandidateGenerationEvidenceBundleManifestV1::preflight_json_bytes(&empty)
        .expect("canonical empty response metadata");
    assert_eq!(
        empty.structured_response_artifact(),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry),
    );
}

#[test]
fn bundle_decoder_rejects_relationship_candidate_and_count_substitution() {
    let fixture = bundle_fixture();
    let encoded = serde_json::to_vec(&fixture.bundle).expect("bundle JSON");
    let wrong_response = replace_once(
        &encoded,
        fixture.bundle.response_id().digest().as_str(),
        digest("wrong response").as_str(),
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &wrong_response,
            fixture.relations(),
            &fixture.candidates
        ),
        Err(GenerationQualificationContractError::EvidenceBundleRelationshipMismatch)
    );
    let wrong_candidate = replace_once(
        &encoded,
        fixture.candidates[0]
            .candidate_evidence_id()
            .digest()
            .as_str(),
        digest("wrong candidate").as_str(),
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &wrong_candidate,
            fixture.relations(),
            &fixture.candidates
        ),
        Err(GenerationQualificationContractError::InvalidCandidateArtifact)
    );
    let wrong_count = replace_once(&encoded, "\"entry_count\":7", "\"entry_count\":8");
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &wrong_count,
            fixture.relations(),
            &fixture.candidates
        ),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
    );
    let invalid_path = replace_once(&encoded, "records/planned.json", "../planned.json");
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &invalid_path,
            fixture.relations(),
            &fixture.candidates,
        ),
        Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &encoded,
            fixture.relations(),
            &[],
        ),
        Err(GenerationQualificationContractError::InvalidCandidateArtifact)
    );
}
