use super::*;

#[test]
fn owner_typed_ids_wrap_exact_digests_and_preserve_substitution_rejection() {
    let package = package_with_workers(1);
    let exact = admitted(&package, b"admission one");
    let other = admitted(&package, b"admission two");

    assert_eq!(
        exact.runtime_admission_join_id().digest(),
        exact.admitted_runtime_id().digest()
    );
    assert_ne!(
        exact.runtime_admission_join_id(),
        other.runtime_admission_join_id()
    );

    let review = review_bytes(&exact, &package);
    let path = verify_reviewed_path(&review, &exact, &package).expect("exact reviewed path");
    assert_eq!(
        path.managed_generation_path_id().digest(),
        path.generation_path_id()
    );
    assert_eq!(
        verify_reviewed_path(&review, &other, &package)
            .expect_err("another admission cannot reuse an exact path review"),
        RuntimeAdmissionAuthorityError::GenerationPathBinding
    );
}
