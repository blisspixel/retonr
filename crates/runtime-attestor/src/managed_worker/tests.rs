use std::{
    fs::File,
    io::{Seek as _, SeekFrom},
    time::{Duration, Instant},
};

use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::{
    ManagedGenerationWorkerError, ManagedGenerationWorkerLimits, RetainedModelWeight,
    RetainedModelWeightSink, RetainedModelWeightSource, worker_deadline_precedence,
};

#[test]
fn expired_retained_deadline_overrides_post_observation_cancellation() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        worker_deadline_precedence::<()>(
            Err(ManagedGenerationWorkerError::ObservationChanged),
            &cancellation,
            Some(Instant::now()),
        ),
        Err(ManagedGenerationWorkerError::DeadlineExceeded)
    );
}

#[test]
fn retained_cancellation_overrides_other_post_observation_error() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        worker_deadline_precedence::<()>(
            Err(ManagedGenerationWorkerError::ObservationChanged),
            &cancellation,
            Some(Instant::now() + Duration::from_secs(1)),
        ),
        Err(ManagedGenerationWorkerError::Cancelled)
    );
}

struct Source {
    artifact_id: ArtifactId,
    byte_size: u64,
    file: File,
}

impl<'lease> RetainedModelWeightSource<'lease> for Source {
    fn transfer(
        self,
        sink: &mut RetainedModelWeightSink<'_, 'lease>,
    ) -> Result<(), ManagedGenerationWorkerError> {
        sink.retain(self.artifact_id, self.byte_size, self.file)
    }
}

fn source(artifact_id: ArtifactId, byte_size: u64, file: File) -> Source {
    Source {
        artifact_id,
        byte_size,
        file,
    }
}

#[test]
fn limits_reject_zero_and_hard_maximum_violations() {
    let limits = ManagedGenerationWorkerLimits {
        maximum_processes: 0,
        ..ManagedGenerationWorkerLimits::default()
    };
    assert_eq!(
        limits.validate(),
        Err(ManagedGenerationWorkerError::InvalidLimits)
    );
    let limits = ManagedGenerationWorkerLimits {
        maximum_elapsed: Duration::from_secs(1_801),
        ..ManagedGenerationWorkerLimits::default()
    };
    assert_eq!(
        limits.validate(),
        Err(ManagedGenerationWorkerError::InvalidLimits)
    );
}

#[test]
fn model_hash_ceiling_is_distinct_from_native_aggregate_and_caller_reducible() {
    let candidate_bytes = 18_000_000_000;
    let limits = ManagedGenerationWorkerLimits {
        maximum_model_weight_bytes: candidate_bytes,
        ..ManagedGenerationWorkerLimits::default()
    };
    assert_eq!(limits.validate().expect("18 GB model ceiling"), limits);
    assert!(candidate_bytes > limits.native_load.maximum_aggregate_hash_bytes);

    let lower = ManagedGenerationWorkerLimits {
        maximum_model_weight_bytes: 1024,
        ..limits
    };
    assert_eq!(lower.validate().expect("caller-reduced ceiling"), lower);
}

#[test]
fn retained_weight_requires_exact_nonempty_regular_file() {
    let temporary = tempdir().expect("temporary directory");
    let path = temporary.path().join("weight.gguf");
    std::fs::write(&path, b"gguf").expect("write fixture");
    let artifact = ArtifactId::from_digest(Digest::sha256(b"gguf"));
    let mut file = File::open(&path).expect("open fixture");
    file.seek(SeekFrom::Start(2)).expect("position fixture");
    let retained_file = file.try_clone().expect("clone fixture handle");
    let cancellation = CancellationToken::new();
    let retained =
        RetainedModelWeight::from_source(source(artifact.clone(), 4, retained_file), &cancellation)
            .expect("exact retained file");
    assert_eq!(retained.artifact_id(), &artifact);
    assert_eq!(file.stream_position().expect("retained position"), 2);
    assert_eq!(
        RetainedModelWeight::from_source(
            source(artifact, 3, File::open(&path).expect("reopen fixture")),
            &cancellation,
        )
        .expect_err("wrong length fails"),
        ManagedGenerationWorkerError::InvalidRequest
    );
    assert_eq!(
        RetainedModelWeight::from_source(
            source(
                ArtifactId::from_digest(Digest::sha256(b"other")),
                4,
                File::open(&path).expect("open digest case"),
            ),
            &cancellation,
        )
        .expect_err("wrong digest fails"),
        ManagedGenerationWorkerError::InvalidRequest
    );
}

#[test]
fn cancellation_fails_before_platform_observation() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        super::ensure_worker_active(
            &cancellation,
            std::time::Instant::now(),
            ManagedGenerationWorkerLimits::default(),
        ),
        Err(ManagedGenerationWorkerError::Cancelled)
    );
}
