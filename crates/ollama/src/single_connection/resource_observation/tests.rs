use std::time::{Duration, Instant};

use rewrite_model::OllamaRetainedSessionResponseId;
use rewrite_types::Digest;

use super::{
    GenerateProviderResourceValues, OllamaGenerateResourceObservation,
    OllamaRetainedSessionSubject, checked_elapsed,
};

#[test]
fn response_head_elapsed_accepts_before_and_equal_but_rejects_reversed_time() {
    let head = Instant::now();
    let start = head
        .checked_sub(Duration::from_nanos(7))
        .expect("representable start");
    assert_eq!(
        checked_elapsed(head, start).expect("ordered time"),
        Duration::from_nanos(7)
    );
    assert_eq!(
        checked_elapsed(head, head).expect("equal time"),
        Duration::ZERO
    );
    assert!(checked_elapsed(head, head + Duration::from_millis(1)).is_err());
}

#[test]
fn exact_getters_cover_the_opaque_boundary() {
    let digest = Digest::sha256(b"opaque resource observation");
    let observation = OllamaGenerateResourceObservation {
        session_subject: OllamaRetainedSessionSubject::new().marker,
        request_binding_digest: digest.clone(),
        response_id: OllamaRetainedSessionResponseId::from_derived_digest(digest.clone()),
        response_ordinal: 7,
        response_head: Instant::now(),
        values: GenerateProviderResourceValues {
            total_duration_nanoseconds: 11,
            load_duration_nanoseconds: 12,
            prompt_token_count: 13,
            prompt_evaluation_duration_nanoseconds: 14,
            generated_token_count: 15,
            evaluation_duration_nanoseconds: 16,
        },
    };
    assert_eq!(observation.response_ordinal(), 7);
    assert_eq!(observation.request_binding_digest(), &digest);
    assert_eq!(observation.response_id().digest(), &digest);
    assert_eq!(observation.total_duration_nanoseconds(), 11);
    assert_eq!(observation.load_duration_nanoseconds(), 12);
    assert_eq!(observation.prompt_token_count(), 13);
    assert_eq!(observation.prompt_evaluation_duration_nanoseconds(), 14);
    assert_eq!(observation.generated_token_count(), 15);
    assert_eq!(observation.evaluation_duration_nanoseconds(), 16);
    assert_eq!(
        format!("{observation:?}"),
        "OllamaGenerateResourceObservation { content: \"redacted\" }"
    );
}
