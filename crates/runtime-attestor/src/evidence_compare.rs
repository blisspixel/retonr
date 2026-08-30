use crate::{AttachedProcessEvidence, AttachedProcessWitnessError};

pub(crate) fn compare_evidence(
    initial: &AttachedProcessEvidence,
    observed: &AttachedProcessEvidence,
) -> Result<(), AttachedProcessWitnessError> {
    if initial.owner_pid() != observed.owner_pid()
        || initial.ownership_snapshot_digest() != observed.ownership_snapshot_digest()
    {
        return Err(AttachedProcessWitnessError::ListenerRebound);
    }
    if initial.process_instance_digest() != observed.process_instance_digest() {
        return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
    }
    if initial.entrypoint_object_digest() != observed.entrypoint_object_digest()
        || initial.entrypoint_digest() != observed.entrypoint_digest()
        || initial.entrypoint_bytes() != observed.entrypoint_bytes()
    {
        return Err(AttachedProcessWitnessError::EntrypointChanged);
    }
    if initial.evidence_class() != observed.evidence_class()
        || initial.platform_evidence_digest() != observed.platform_evidence_digest()
    {
        return Err(AttachedProcessWitnessError::PlatformObservationFailed);
    }
    Ok(())
}
