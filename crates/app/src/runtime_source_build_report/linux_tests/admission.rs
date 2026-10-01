use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::CancellationToken;
use tempfile::tempdir_in;

use crate::{
    RUNTIME_SOURCE_BUILD_REPORT_PATH, RuntimeAdmissionEvidenceAssemblyCompiler,
    RuntimeAdmissionEvidenceAssemblyMember, RuntimeAdmissionEvidenceAssemblyPublisher,
    RuntimeAdmissionEvidenceBundleDestination, RuntimeAdmissionEvidenceBundleLimits,
    RuntimeAdmissionEvidenceBundleSource, RuntimeAdmissionEvidenceBundleVerifier,
    RuntimeAdmissionEvidenceFoundation, RuntimeAdmissionEvidenceFoundationInput,
    RuntimeAdmissionEvidenceMember, RuntimeSourceBuildEvidenceBundleLease,
};

// The native controlled-build fixture is synthetic. Its retained byte closure
// exercises public adapter composition without admitting a runtime or controls.
pub(super) fn assert_public_admission_assembly(
    source: &RuntimeSourceBuildEvidenceBundleLease,
    cancellation: &CancellationToken,
) {
    let foundation = foundation(source);
    let bytes = b"synthetic unreviewed native fixture evidence";
    let members: Vec<_> = RuntimeAdmissionEvidenceMember::ALL
        .into_iter()
        .skip(1)
        .map(|member| RuntimeAdmissionEvidenceAssemblyMember::new(member, bytes))
        .collect();
    let limits = RuntimeAdmissionEvidenceBundleLimits::default();
    let assembly = RuntimeAdmissionEvidenceAssemblyCompiler::compile(
        &foundation,
        source,
        &members,
        limits,
        cancellation,
    )
    .expect("compile public assembly with actual retained source-build lease");
    let parent = tempdir_in("/tmp").expect("independent admission evidence parent");
    let destination =
        RuntimeAdmissionEvidenceBundleDestination::new(parent.path().join("evidence"))
            .expect("absent admission evidence destination");
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(
        RuntimeAdmissionEvidenceAssemblyPublisher::publish(
            &assembly,
            source,
            &destination,
            limits,
            &cancelled,
        )
        .is_err()
    );
    assert!(!destination.path().exists());
    let published = RuntimeAdmissionEvidenceAssemblyPublisher::publish(
        &assembly,
        source,
        &destination,
        limits,
        cancellation,
    )
    .expect("publish public source-bound inert assembly");
    assert_eq!(published.manifest(), assembly.manifest());
    assert_eq!(published.foundation(), &foundation);
    assert!(
        RuntimeAdmissionEvidenceAssemblyPublisher::publish(
            &assembly,
            source,
            &destination,
            limits,
            cancellation,
        )
        .is_err()
    );
    let selection = RuntimeAdmissionEvidenceBundleSource::new(destination.path())
        .expect("select published admission evidence");
    let reacquired =
        RuntimeAdmissionEvidenceBundleVerifier::acquire(&selection, limits, cancellation)
            .expect("cold independent public admission evidence readback");
    assert_eq!(reacquired.manifest(), published.manifest());
    assert_eq!(reacquired.foundation(), &foundation);
    for member in RuntimeAdmissionEvidenceMember::ALL.into_iter().skip(1) {
        assert_eq!(
            reacquired
                .member_bytes(member, cancellation)
                .expect("fresh opaque member readback"),
            bytes,
        );
    }
    source
        .revalidate(cancellation)
        .expect("source remains unchanged");
    drop(reacquired);
    drop(published);
    let changed_member = RuntimeAdmissionEvidenceMember::ALL[1];
    std::fs::write(
        destination.path().join(changed_member.relative_path()),
        b"changed",
    )
    .expect("mutate synthetic admission evidence member");
    assert!(
        RuntimeAdmissionEvidenceBundleVerifier::acquire(&selection, limits, cancellation,).is_err()
    );
}

fn foundation(
    source: &RuntimeSourceBuildEvidenceBundleLease,
) -> RuntimeAdmissionEvidenceFoundation {
    let report_path = ArtifactSetRelativePath::new(RUNTIME_SOURCE_BUILD_REPORT_PATH.to_owned())
        .expect("fixed report path");
    let report_member = source
        .manifest()
        .members()
        .iter()
        .find(|member| member.relative_path() == &report_path)
        .expect("retained report member");
    RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
        source.manifest().artifact_set_id(),
        source
            .source_inputs()
            .manifest()
            .artifact_set()
            .artifact_set_id(),
        source.source_inputs().manifest().manifest_digest().clone(),
        source.plan().plan_digest().clone(),
        report_member.artifact_id().digest().clone(),
        source
            .report()
            .primary()
            .runtime_package()
            .runtime_package_manifest_id(),
    ))
    .expect("compile exact inert source-build foundation")
}
