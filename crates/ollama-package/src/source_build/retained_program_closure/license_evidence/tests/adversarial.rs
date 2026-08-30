use super::super::verify_inventory;
use super::{
    Fixture, RetainedProgramLicenseEvidenceError, SubjectWire, set_expression,
    subject_materials_mut,
};

#[test]
fn every_external_closure_binding_changes_the_evidence_identity() {
    for mutate in [0_u8, 1, 2] {
        let fixture = Fixture::valid();
        let mut bindings = fixture.bindings.clone();
        match mutate {
            0 => bindings.source_set = rewrite_types::Digest::sha256(b"other source"),
            1 => bindings.cargo = rewrite_types::Digest::sha256(b"other cargo"),
            _ => bindings.upstream = rewrite_types::Digest::sha256(b"other upstream"),
        }
        let verified = verify_inventory(
            &fixture.manifest,
            &fixture.bytes(),
            &fixture.lock,
            bindings,
            super::RetainedProgramLicenseEvidenceLimits::default(),
        )
        .expect("external binding is incorporated");
        assert_ne!(
            verified.closure_id().digest(),
            fixture.verify().expect("base").closure_id().digest()
        );
    }
}

#[test]
fn incomplete_duplicate_reordered_and_drifted_subjects_are_rejected() {
    let mut missing = Fixture::valid();
    missing.inventory.subjects.pop();
    assert_eq!(
        missing.verify(),
        Err(RetainedProgramLicenseEvidenceError::IncompleteSubjectSet)
    );

    let mut reordered = Fixture::valid();
    reordered.inventory.subjects.swap(0, 1);
    assert_eq!(
        reordered.verify(),
        Err(RetainedProgramLicenseEvidenceError::IncompleteSubjectSet)
    );

    let mut drifted = Fixture::valid();
    if let SubjectWire::Component { digest, .. } = &mut drifted.inventory.subjects[0] {
        *digest = rewrite_types::Digest::sha256(b"changed component");
    }
    assert_eq!(
        drifted.verify(),
        Err(RetainedProgramLicenseEvidenceError::IncompleteSubjectSet)
    );

    let mut duplicate = Fixture::valid();
    duplicate.inventory.subjects[1] = duplicate.inventory.subjects[0].clone();
    assert_eq!(
        duplicate.verify(),
        Err(RetainedProgramLicenseEvidenceError::IncompleteSubjectSet)
    );
}

#[test]
fn unknown_missing_duplicate_reordered_and_unreferenced_materials_are_rejected() {
    let mut unknown = Fixture::valid();
    unknown.inventory.materials[0].subject_key = "component:unknown".to_owned();
    assert_eq!(
        unknown.verify(),
        Err(RetainedProgramLicenseEvidenceError::UnknownSubject)
    );

    let mut missing = Fixture::valid();
    missing.inventory.materials.pop();
    assert_eq!(
        missing.verify(),
        Err(RetainedProgramLicenseEvidenceError::InvalidMaterialSet)
    );

    let mut duplicate = Fixture::valid();
    duplicate.inventory.materials[1] = duplicate.inventory.materials[0].clone();
    assert_eq!(
        duplicate.verify(),
        Err(RetainedProgramLicenseEvidenceError::InvalidMaterialSet)
    );

    let mut reordered = Fixture::valid();
    reordered.inventory.materials.swap(0, 1);
    assert_eq!(
        reordered.verify(),
        Err(RetainedProgramLicenseEvidenceError::InvalidMaterialSet)
    );

    let mut unreferenced = Fixture::valid();
    let id = rewrite_types::Digest::sha256(b"unreferenced");
    subject_materials_mut(&mut unreferenced.inventory.subjects[0]).push(id);
    subject_materials_mut(&mut unreferenced.inventory.subjects[0])
        .sort_by(|left, right| left.as_str().cmp(right.as_str()));
    assert_eq!(
        unreferenced.verify(),
        Err(RetainedProgramLicenseEvidenceError::InvalidMaterialSet)
    );
}

#[test]
fn material_content_size_digest_path_and_expression_drift_are_rejected() {
    let mut content = Fixture::valid();
    content.inventory.materials[0].content.push('x');
    assert_eq!(
        content.verify(),
        Err(RetainedProgramLicenseEvidenceError::MaterialMeasurementMismatch)
    );

    let mut size = Fixture::valid();
    size.inventory.materials[0].byte_size += 1;
    assert_eq!(
        size.verify(),
        Err(RetainedProgramLicenseEvidenceError::MaterialMeasurementMismatch)
    );

    let mut digest = Fixture::valid();
    digest.inventory.materials[0].digest = rewrite_types::Digest::sha256(b"wrong");
    assert_eq!(
        digest.verify(),
        Err(RetainedProgramLicenseEvidenceError::MaterialMeasurementMismatch)
    );

    let mut path = Fixture::valid();
    path.inventory.materials[0].relative_path = "../escape".to_owned();
    assert_eq!(
        path.verify(),
        Err(RetainedProgramLicenseEvidenceError::InvalidReviewerFact)
    );

    let mut expression = Fixture::valid();
    set_expression(&mut expression.inventory.subjects[0], " MIT");
    assert_eq!(
        expression.verify(),
        Err(RetainedProgramLicenseEvidenceError::InvalidReviewerFact)
    );
}
