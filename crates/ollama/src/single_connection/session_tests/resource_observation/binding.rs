use super::*;

const DUPLICATE_TOTAL_DURATION: &str = concat!(
    "\"total_duration\":9000,\"total_duration\":9001,",
    "\"load_duration\":1000,\"prompt_eval_count\":4,",
    "\"prompt_eval_duration\":2000,\"eval_count\":2,\"eval_duration\":3000"
);
const DUPLICATE_LOAD_DURATION: &str = concat!(
    "\"total_duration\":9000,\"load_duration\":1000,\"load_duration\":1001,",
    "\"prompt_eval_count\":4,\"prompt_eval_duration\":2000,",
    "\"eval_count\":2,\"eval_duration\":3000"
);
const DUPLICATE_PROMPT_EVAL_DURATION: &str = concat!(
    "\"total_duration\":9000,\"load_duration\":1000,\"prompt_eval_count\":4,",
    "\"prompt_eval_duration\":2000,\"prompt_eval_duration\":2001,",
    "\"eval_count\":2,\"eval_duration\":3000"
);

#[tokio::test]
async fn duplicate_new_telemetry_keys_preserve_legacy_compatibility_but_fail_strict_evidence() {
    for metrics in [
        DUPLICATE_TOTAL_DURATION,
        DUPLICATE_LOAD_DURATION,
        DUPLICATE_PROMPT_EVAL_DURATION,
    ] {
        let compatibility_server = SessionServer::start(SessionMode::ResidentResource {
            metrics,
            delayed_body: false,
        })
        .await;
        let stream = compatibility_server.supplied_stream().await;
        let token = CancellationToken::new();
        let mut session = config(
            compatibility_server.endpoint.clone(),
            OllamaLimits::default(),
        )
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open compatibility session");
        session.preflight(context(&token)).await.expect("preflight");
        session
            .complete_structured_with_residency_borrowed(
                &request("duplicate compatibility"),
                context(&token),
            )
            .await
            .expect("new duplicate telemetry remains an ignored legacy field");
        drop(session);
        assert_eq!(compatibility_server.finish().await.requests.len(), 16);

        assert_resource_failure(metrics, "resource_observation_unavailable").await;
    }
}

#[tokio::test]
async fn identical_portable_completions_retain_distinct_unseparable_session_subjects() {
    let first = resource_completion_from_new_session("identical session input").await;
    let second = resource_completion_from_new_session("identical session input").await;

    assert_eq!(
        derive_ollama_retained_session_response_id(first.response()),
        derive_ollama_retained_session_response_id(second.response())
    );
    assert_eq!(
        first.resident_execution_receipt(),
        second.resident_execution_receipt()
    );
    let first_token = first.retained_session_subject_token();
    let first_token_clone = first_token.clone();
    let second_token = second.retained_session_subject_token();
    assert!(first.binds_retained_session_subject(&first_token));
    assert!(first.binds_retained_session_subject(&first_token_clone));
    assert!(second.binds_retained_session_subject(&second_token));
    assert!(!first.binds_retained_session_subject(&second_token));
    assert!(!second.binds_retained_session_subject(&first_token));
    assert!(!first.shares_retained_session_subject(&second));
    assert_eq!(
        format!("{first_token:?}"),
        "OllamaRetainedSessionSubjectToken { content: \"redacted\" }"
    );
    first
        .verify_completion_binding()
        .expect("first owned binding");
    second
        .verify_completion_binding()
        .expect("second owned binding");
}

async fn resource_completion_from_new_session(
    input: &str,
) -> OllamaResidentResourceObservedCompletion {
    let server = SessionServer::start(SessionMode::ResidentResource {
        metrics: COMPLETE_METRICS,
        delayed_body: false,
    })
    .await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open resource session");
    session.preflight(context(&token)).await.expect("preflight");
    let completion = session
        .complete_structured_with_residency_and_resource_observation_borrowed(
            &request(input),
            context(&token),
        )
        .await
        .expect("resource completion");
    drop(session);
    assert_eq!(server.finish().await.requests.len(), 16);
    completion
}
