use super::*;

#[tokio::test]
async fn ordinary_generation_uses_the_exact_provider_neutral_candidate_count() {
    let server = MockServer::start().await;
    mount_common(&server, 2, 2).await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show_body()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": MODEL,
            "response": "{\"candidates\":[{\"text\":\"one\"},{\"text\":\"two\"}]}",
            "thinking": "",
            "done": true,
            "done_reason": "stop"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let binding = binding();
    let token = CancellationToken::new();
    let error = backend(&server)
        .generate(request(&binding), context(&token))
        .await
        .expect_err("extra candidate is rejected");
    assert_eq!(error.kind, InferenceErrorKind::MalformedResponse);
    assert_eq!(error.code, "candidate_count_mismatch");
}
