use std::{io, time::Duration};

use rewrite_inference::{
    OperationContext, ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
    SamplingParameters, StructuredCompletionRequest, candidate_output_contract,
    local_judge_attempt_output_contract,
};
use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

use crate::{OllamaEndpoint, OllamaLimits, OllamaModelBinding, OllamaRetainedStreamSessionConfig};

pub(super) const MODEL: &str = "fixture:latest";
pub(super) const INVENTORY_DIGEST: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(super) const ARTIFACT_DIGEST: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Clone, Copy)]
pub(super) enum SessionMode {
    Normal { completions: usize },
    ResidentNormal { completions: usize },
    ResidentDrift,
    ResidentAmbiguous,
    ResidentDelayed,
    ResidentUnloaded,
    ResidentWrongDigest,
    ResidentWrongSize,
    ResidentWrongContext,
    ResidentWrongReference,
    ResidentPreloaded,
    CloseResidency,
    StallResidency,
    LargeResidency,
    RuntimeDrift,
    InventoryDrift,
    DetailsDrift,
    RemoteGeneration,
    NonterminalGeneration,
    InvalidGenerationOutput,
    TruncatedGeneration,
    CloseGeneration,
    StallGeneration,
    LargeGeneration,
    JudgeOutput,
    InvalidJudgeOutput,
}

pub(super) struct SessionServer {
    pub(super) endpoint: OllamaEndpoint,
    task: JoinHandle<SessionServerResult>,
}

pub(super) struct SessionServerResult {
    pub(super) accepts: usize,
    pub(super) requests: Vec<String>,
    pub(super) generate_requests: Vec<serde_json::Value>,
}

impl SessionServer {
    pub(super) async fn start(mode: SessionMode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind session fixture");
        let endpoint = OllamaEndpoint::parse(&format!(
            "http://{}",
            listener.local_addr().expect("session fixture address")
        ))
        .expect("session fixture endpoint");
        let task = tokio::spawn(async move { serve(listener, mode).await.expect("serve session") });
        Self { endpoint, task }
    }

    pub(super) async fn supplied_stream(&self) -> std::net::TcpStream {
        TcpStream::connect(self.endpoint.socket_addr())
            .await
            .expect("connect supplied session stream")
            .into_std()
            .expect("convert supplied session stream")
    }

    pub(super) async fn finish(self) -> SessionServerResult {
        self.task.await.expect("session fixture task")
    }
}

pub(super) fn binding() -> OllamaModelBinding {
    let artifact = Digest::from_sha256_hex(ARTIFACT_DIGEST).expect("fixture artifact digest");
    let inventory = Digest::from_sha256_hex(INVENTORY_DIGEST).expect("fixture inventory digest");
    OllamaModelBinding::new_with_inventory(
        MODEL,
        ArtifactId::from_digest(artifact.clone()),
        artifact,
        inventory,
    )
    .expect("fixture binding")
}

pub(super) fn config(
    endpoint: OllamaEndpoint,
    limits: OllamaLimits,
) -> OllamaRetainedStreamSessionConfig {
    OllamaRetainedStreamSessionConfig::new(endpoint, vec![binding()], limits, 4 * 1024 * 1024)
        .expect("retained stream config")
}

pub(super) fn request(input: &str) -> StructuredCompletionRequest {
    let binding = binding();
    StructuredCompletionRequest {
        schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
        artifact_id: binding.artifact_id().clone(),
        artifact_digest: binding.artifact_digest().clone(),
        input: input.to_owned(),
        output: candidate_output_contract(),
        source_byte_count: input.len() as u64,
        source_byte_limit: 1024,
        input_byte_limit: 2048,
        context_token_limit: 2048,
        output_token_limit: 256,
        output_byte_limit: 4096,
        sampling: SamplingParameters {
            temperature: 0.0,
            top_p: 1.0,
            seed: Some(7),
        },
        reasoning: ReasoningPolicy::Disabled,
    }
}

pub(super) fn judge_request(input: &str) -> StructuredCompletionRequest {
    StructuredCompletionRequest {
        output: local_judge_attempt_output_contract(),
        ..request(input)
    }
}

pub(super) fn request_with_relaxed_self_limit(input: String) -> StructuredCompletionRequest {
    let input_bytes = u64::try_from(input.len()).expect("fixture input length");
    let mut request = request("");
    request.input = input;
    request.source_byte_count = input_bytes;
    request.source_byte_limit = input_bytes;
    request.input_byte_limit = u64::MAX;
    request
}

pub(super) fn judge_request_with_relaxed_self_limit(input: String) -> StructuredCompletionRequest {
    StructuredCompletionRequest {
        output: local_judge_attempt_output_contract(),
        ..request_with_relaxed_self_limit(input)
    }
}

pub(super) fn context(token: &CancellationToken) -> OperationContext<'_> {
    OperationContext::new(
        token,
        Some(std::time::Instant::now() + Duration::from_secs(5)),
    )
}

async fn serve(listener: TcpListener, mode: SessionMode) -> io::Result<SessionServerResult> {
    let (mut stream, _client) = listener.accept().await?;
    let mut buffer = Vec::new();
    let mut requests = Vec::new();
    let mut generate_requests = Vec::new();
    let mut truncated = false;
    for ordinal in 1..=maximum_requests(mode) {
        let Some(request) = read_request(&mut stream, &mut buffer).await? else {
            break;
        };
        requests.push(request.line.clone());
        let path = request
            .line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| io::Error::other("missing request path"))?;
        if path == "/api/generate" {
            generate_requests.push(
                serde_json::from_slice(&request.body)
                    .map_err(|_error| io::Error::other("invalid generate request"))?,
            );
        }
        if (matches!(mode, SessionMode::StallGeneration) && path == "/api/generate")
            || (matches!(mode, SessionMode::StallResidency) && path == "/api/ps" && ordinal > 7)
        {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        if matches!(mode, SessionMode::TruncatedGeneration) && path == "/api/generate" {
            write_truncated(&mut stream, &response_body(path, mode, ordinal)).await?;
            truncated = true;
            break;
        }
        let close = (matches!(mode, SessionMode::CloseGeneration) && path == "/api/generate")
            || (matches!(mode, SessionMode::CloseResidency) && path == "/api/ps" && ordinal > 7);
        write_response(&mut stream, &response_body(path, mode, ordinal), close).await?;
        if close {
            break;
        }
    }
    if !truncated {
        let mut probe = [0_u8; 1];
        let _ = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut probe)).await;
    }
    drop(stream);
    let accepts = if tokio::time::timeout(Duration::from_millis(100), listener.accept())
        .await
        .is_ok()
    {
        2
    } else {
        1
    };
    Ok(SessionServerResult {
        accepts,
        requests,
        generate_requests,
    })
}

const fn maximum_requests(mode: SessionMode) -> usize {
    match mode {
        SessionMode::Normal { completions } => 7 + completions * 7,
        SessionMode::ResidentNormal { completions } => 7 + completions * 9,
        _ => 16,
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
            Err(error) if is_disconnect(&error) => return Ok(None),
            Err(error) => return Err(error),
        }
    }
}

fn response_body(path: &str, mode: SessionMode, ordinal: usize) -> String {
    match path {
        "/api/version" if resident_mode(mode) => r#"{"version":"0.32.15"}"#.to_owned(),
        "/api/version" if matches!(mode, SessionMode::RuntimeDrift) && ordinal > 11 => {
            r#"{"version":"0.32.15"}"#.to_owned()
        }
        "/api/version" => r#"{"version":"0.32.14"}"#.to_owned(),
        "/api/tags" if matches!(mode, SessionMode::InventoryDrift) && ordinal > 11 => format!(
            r#"{{"models":[{{"name":"fixture:latest","model":"fixture:latest","size":2048,"digest":"{INVENTORY_DIGEST}"}}]}}"#
        ),
        "/api/tags" => format!(
            r#"{{"models":[{{"name":"fixture:latest","model":"fixture:latest","size":1024,"digest":"{INVENTORY_DIGEST}"}}]}}"#
        ),
        "/api/ps" => residency_response(mode, ordinal),
        "/api/show" if matches!(mode, SessionMode::DetailsDrift) && ordinal > 13 => show("Q8_0"),
        "/api/show" => show("Q4_K_M"),
        "/api/generate" => generate(mode),
        _ => r#"{"error":"unexpected path"}"#.to_owned(),
    }
}

const fn resident_mode(mode: SessionMode) -> bool {
    matches!(
        mode,
        SessionMode::ResidentNormal { .. }
            | SessionMode::ResidentDrift
            | SessionMode::ResidentAmbiguous
            | SessionMode::ResidentDelayed
            | SessionMode::ResidentUnloaded
            | SessionMode::ResidentWrongDigest
            | SessionMode::ResidentWrongSize
            | SessionMode::ResidentWrongContext
            | SessionMode::ResidentWrongReference
            | SessionMode::ResidentPreloaded
            | SessionMode::CloseResidency
            | SessionMode::StallResidency
            | SessionMode::LargeResidency
    )
}

fn residency_response(mode: SessionMode, ordinal: usize) -> String {
    if !resident_mode(mode) || (ordinal <= 7 && !matches!(mode, SessionMode::ResidentPreloaded)) {
        return r#"{"models":[]}"#.to_owned();
    }
    if matches!(mode, SessionMode::ResidentDelayed) && ordinal == 12
        || matches!(mode, SessionMode::ResidentUnloaded) && ordinal == 16
    {
        return r#"{"models":[]}"#.to_owned();
    }
    if matches!(mode, SessionMode::LargeResidency) && ordinal > 7 {
        return format!(r#"{{"models":[],"padding":"{}"}}"#, "x".repeat(4096));
    }
    let digest = if matches!(mode, SessionMode::ResidentWrongDigest) {
        ARTIFACT_DIGEST
    } else {
        INVENTORY_DIGEST
    };
    let size = if matches!(mode, SessionMode::ResidentWrongSize) && ordinal == 16 {
        2048
    } else {
        1024
    };
    let context = if matches!(mode, SessionMode::ResidentWrongContext) {
        4096
    } else {
        2048
    };
    let accelerator = if matches!(mode, SessionMode::ResidentDrift) && ordinal == 16 {
        512
    } else {
        256
    };
    let model_field = if matches!(mode, SessionMode::ResidentWrongReference) {
        "alias:latest"
    } else {
        MODEL
    };
    let primary = format!(
        r#"{{"name":"{MODEL}","model":"{model_field}","size":{size},"digest":"{digest}","size_vram":{accelerator},"context_length":{context}}}"#
    );
    if matches!(mode, SessionMode::ResidentAmbiguous) && ordinal > 7 {
        let other = r#"{"name":"other:latest","model":"other:latest","size":512,"digest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","size_vram":0,"context_length":2048}"#;
        format!(r#"{{"models":[{primary},{other}]}}"#)
    } else {
        format!(r#"{{"models":[{primary}]}}"#)
    }
}

fn show(quantization: &str) -> String {
    format!(
        r#"{{"license":"fixture license","template":"fixture template","capabilities":["completion"],"details":{{"format":"gguf","family":"fixture","quantization_level":"{quantization}"}},"model_info":{{"fixture.context_length":4096}}}}"#
    )
}

fn generate(mode: SessionMode) -> String {
    let remote = if matches!(mode, SessionMode::RemoteGeneration) {
        r#", "remote_model":"remote/model""#
    } else {
        ""
    };
    let done = !matches!(mode, SessionMode::NonterminalGeneration);
    let output = if matches!(mode, SessionMode::InvalidGenerationOutput) {
        "{"
    } else if matches!(mode, SessionMode::JudgeOutput) {
        r#"{\"schema_version\":1,\"case_id\":\"case_01\",\"choice\":\"first\",\"rubric_clauses\":[\"clarity\"],\"source_spans\":[{\"start\":0,\"end\":4}],\"first_candidate_spans\":[],\"second_candidate_spans\":[]}"#
    } else if matches!(mode, SessionMode::InvalidJudgeOutput) {
        r#"{\"schema_version\":1,\"case_id\":\"case_01\",\"choice\":\"first\",\"rubric_clauses\":[\"meaning\",\"clarity\"],\"source_spans\":[],\"first_candidate_spans\":[],\"second_candidate_spans\":[]}"#
    } else if matches!(mode, SessionMode::LargeGeneration) {
        return format!(
            r#"{{"model":"{MODEL}","response":"{}","done":true,"done_reason":"stop"}}"#,
            "x".repeat(1024)
        );
    } else {
        r#"{\"candidates\":[{\"text\":\"ok\"}]}"#
    };
    format!(
        r#"{{"model":"{MODEL}"{remote},"response":"{output}","done":{done},"done_reason":"stop","prompt_eval_count":4,"eval_count":2,"eval_duration":3000}}"#
    )
}

async fn write_response(stream: &mut TcpStream, body: &str, close: bool) -> io::Result<()> {
    let connection = if close { "Connection: close\r\n" } else { "" };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{connection}\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

async fn write_truncated(stream: &mut TcpStream, body: &str) -> io::Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len() + 5
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

fn is_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::UnexpectedEof
    )
}
