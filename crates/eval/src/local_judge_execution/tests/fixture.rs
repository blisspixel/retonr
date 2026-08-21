use std::{convert::Infallible, io, time::Duration};

use rewrite_inference::{OperationContext, local_judge_attempt_output_contract};
use rewrite_model::ArtifactId;
use rewrite_ollama::{
    OllamaEndpoint, OllamaLimits, OllamaModelBinding, OllamaResponseObservation,
    OllamaRetainedStreamSession, OllamaRetainedStreamSessionConfig,
};
use rewrite_types::{CancellationToken, Digest};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub(super) const MODEL: &str = "judge-fixture:latest";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub(super) struct JudgeServer {
    pub(super) endpoint: OllamaEndpoint,
    task: JoinHandle<JudgeServerResult>,
}

pub(super) struct JudgeServerResult {
    pub(super) accepts: usize,
    pub(super) paths: Vec<String>,
    pub(super) generate_requests: Vec<Value>,
}

impl JudgeServer {
    pub(super) async fn start(outputs: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind judge server");
        let endpoint = OllamaEndpoint::parse(&format!(
            "http://{}",
            listener.local_addr().expect("judge server address")
        ))
        .expect("judge endpoint");
        let task =
            tokio::spawn(async move { serve(listener, outputs).await.expect("serve judge") });
        Self { endpoint, task }
    }

    pub(super) async fn supplied_stream(&self) -> std::net::TcpStream {
        TcpStream::connect(self.endpoint.socket_addr())
            .await
            .expect("connect supplied stream")
            .into_std()
            .expect("convert supplied stream")
    }

    pub(super) async fn finish(self) -> JudgeServerResult {
        self.task.await.expect("judge server task")
    }
}

pub(super) fn model_binding() -> OllamaModelBinding {
    let digest = Digest::from_sha256_hex(DIGEST).expect("fixture digest");
    OllamaModelBinding::new(MODEL, ArtifactId::from_digest(digest.clone()), digest)
        .expect("model binding")
}

pub(super) async fn preflighted_session(
    server: &JudgeServer,
    token: &CancellationToken,
) -> OllamaRetainedStreamSession<
    impl FnMut(OllamaResponseObservation) -> Result<(), Infallible> + use<>,
> {
    let config = OllamaRetainedStreamSessionConfig::new(
        server.endpoint.clone(),
        vec![model_binding()],
        OllamaLimits {
            discovery_body_bytes: 1024 * 1024,
            generation_body_bytes: 1024 * 1024,
            connect_timeout: Duration::from_secs(1),
            request_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(2),
            max_concurrency: 1,
        },
        8 * 1024 * 1024,
    )
    .expect("session config");
    let context = OperationContext::new(
        token,
        Some(std::time::Instant::now() + Duration::from_secs(5)),
    );
    let mut session = config
        .open(server.supplied_stream().await, context, |_observation| {
            Ok::<(), Infallible>(())
        })
        .await
        .expect("open retained session");
    session.preflight(context).await.expect("preflight session");
    session
}

pub(super) fn output(case_id: &str, choice: &str) -> String {
    json!({
        "schema_version": 1,
        "case_id": case_id,
        "choice": choice,
        "rubric_clauses": ["meaning"],
        "source_spans": [{"start": 0, "end": 5}],
        "first_candidate_spans": [{"start": 0, "end": 5}],
        "second_candidate_spans": [{"start": 0, "end": 5}]
    })
    .to_string()
}

pub(super) fn assert_judge_wire_policy(requests: &[Value]) {
    assert!(!requests.is_empty());
    let contract = local_judge_attempt_output_contract();
    for request in requests {
        assert_eq!(
            request["format"],
            serde_json::from_str::<Value>(&contract.schema_json).expect("judge output schema")
        );
        assert_eq!(request["model"], MODEL);
        assert_eq!(request["stream"], false);
        assert_eq!(request["think"], false);
        assert_eq!(request["raw"], false);
        assert_eq!(request["options"]["temperature"], 0.0);
        assert_eq!(request["options"]["top_p"], 1.0);
        assert!(request["options"]["seed"].is_u64());
        assert_eq!(request["options"]["num_ctx"], 8_192);
        assert_eq!(request["options"]["num_predict"], 512);
    }
}

async fn serve(listener: TcpListener, outputs: Vec<String>) -> io::Result<JudgeServerResult> {
    let (mut stream, _client) = listener.accept().await?;
    let mut buffer = Vec::new();
    let mut paths = Vec::new();
    let mut generate_requests = Vec::new();
    let mut output_index = 0_usize;
    while let Some(request) = read_request(&mut stream, &mut buffer).await? {
        let path = request
            .line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| io::Error::other("missing request path"))?
            .to_owned();
        paths.push(path.clone());
        let body = if path == "/api/generate" {
            let request_json = serde_json::from_slice::<Value>(&request.body)
                .map_err(|_error| io::Error::other("invalid generate request"))?;
            generate_requests.push(request_json);
            let output = outputs
                .get(output_index)
                .ok_or_else(|| io::Error::other("unexpected judge attempt"))?;
            output_index += 1;
            json!({
                "model": MODEL,
                "response": output,
                "done": true,
                "done_reason": "stop",
                "prompt_eval_count": 32,
                "eval_count": 8,
                "eval_duration": 3_000
            })
            .to_string()
        } else {
            discovery_body(&path)
        };
        if write_response(&mut stream, &body).await.is_err() {
            break;
        }
    }
    let accepts = if tokio::time::timeout(Duration::from_millis(100), listener.accept())
        .await
        .is_ok()
    {
        2
    } else {
        1
    };
    Ok(JudgeServerResult {
        accepts,
        paths,
        generate_requests,
    })
}

fn discovery_body(path: &str) -> String {
    match path {
        "/api/version" => json!({"version": "0.32.14"}).to_string(),
        "/api/tags" => json!({
            "models": [{
                "name": MODEL,
                "model": MODEL,
                "size": 1024,
                "digest": DIGEST
            }]
        })
        .to_string(),
        "/api/ps" => json!({"models": []}).to_string(),
        "/api/show" => json!({
            "license": "fixture license",
            "template": "fixture template",
            "capabilities": ["completion"],
            "details": {
                "format": "gguf",
                "family": "fixture",
                "quantization_level": "Q4_K_M"
            },
            "model_info": {"fixture.context_length": 8192}
        })
        .to_string(),
        _ => json!({"error": "unexpected path"}).to_string(),
    }
}

struct HttpRequest {
    line: String,
    body: Vec<u8>,
}

async fn read_request(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
) -> io::Result<Option<HttpRequest>> {
    loop {
        if let Some(header_end) = buffer.windows(4).position(|value| value == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&buffer[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length: ")
                        .or_else(|| line.strip_prefix("Content-Length: "))
                })
                .map(str::parse::<usize>)
                .transpose()
                .map_err(|_error| io::Error::other("invalid content length"))?
                .unwrap_or(0);
            let total = header_end + 4 + content_length;
            if buffer.len() >= total {
                let line = headers
                    .lines()
                    .next()
                    .and_then(|line| line.strip_suffix(" HTTP/1.1"))
                    .ok_or_else(|| io::Error::other("invalid request line"))?
                    .to_owned();
                let body = buffer[header_end + 4..total].to_vec();
                buffer.drain(..total);
                return Ok(Some(HttpRequest { line, body }));
            }
        }
        let mut chunk = [0_u8; 4096];
        match stream.read(&mut chunk).await {
            Ok(0) => return Ok(None),
            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::BrokenPipe
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::UnexpectedEof
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error),
        }
    }
}

async fn write_response(stream: &mut TcpStream, body: &str) -> io::Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}
