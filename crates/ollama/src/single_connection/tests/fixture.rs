use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use rewrite_inference::OperationContext;
use rewrite_types::{CancellationToken, Digest};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

use crate::{
    OllamaEndpoint, OllamaLimits, OllamaPreflightTarget,
    single_connection::OllamaSingleConnectionPreflight,
};

pub(super) const MODEL: &str = "fixture:latest";
pub(super) const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(super) const SECOND_DIGEST: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Clone, Copy)]
pub(super) enum FirstResponseMode {
    Normal,
    ConnectionClose,
    Upgrade,
    SilentClose,
    Http10,
    SwitchingProtocols,
    UpgradeHeaderOnly,
    DeclaredTrailer,
    ActualTrailer,
    WrongContentType,
    NotFound,
    TooManyRequests,
    Rejected,
    InvalidJson,
    StallBody,
    StallHeaders,
    RuntimeDrift,
}

pub(super) struct FixtureServer {
    pub(super) endpoint: OllamaEndpoint,
    pub(super) requests: Arc<AtomicUsize>,
    pub(super) responses: Arc<AtomicUsize>,
    task: JoinHandle<ServerResult>,
}

pub(super) struct ServerResult {
    pub(super) accepts: usize,
    pub(super) requests: Vec<String>,
    pub(super) client_closed: bool,
}

impl FixtureServer {
    pub(super) async fn start(mode: FirstResponseMode) -> Self {
        Self::start_with_targets(mode, 1).await
    }

    pub(super) async fn start_with_targets(mode: FirstResponseMode, target_count: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture server");
        let endpoint = OllamaEndpoint::parse(&format!(
            "http://{}",
            listener.local_addr().expect("fixture address")
        ))
        .expect("fixture endpoint");
        let requests = Arc::new(AtomicUsize::new(0));
        let responses = Arc::new(AtomicUsize::new(0));
        let task_requests = Arc::clone(&requests);
        let task_responses = Arc::clone(&responses);
        let task = tokio::spawn(async move {
            serve(listener, mode, target_count, task_requests, task_responses)
                .await
                .expect("serve fixture")
        });
        Self {
            endpoint,
            requests,
            responses,
            task,
        }
    }

    pub(super) async fn finish(self) -> ServerResult {
        self.task.await.expect("fixture task")
    }
}

pub(super) fn target() -> OllamaPreflightTarget {
    OllamaPreflightTarget::new(
        MODEL,
        Digest::from_sha256_hex(DIGEST).expect("fixture digest"),
    )
    .expect("fixture target")
}

pub(super) fn second_target() -> OllamaPreflightTarget {
    OllamaPreflightTarget::new(
        "second:latest",
        Digest::from_sha256_hex(SECOND_DIGEST).expect("second fixture digest"),
    )
    .expect("second fixture target")
}

pub(super) fn context(token: &CancellationToken) -> OperationContext<'_> {
    OperationContext::new(token, Some(Instant::now() + Duration::from_secs(5)))
}

pub(super) fn preflight(
    endpoint: OllamaEndpoint,
    session_bytes: usize,
) -> OllamaSingleConnectionPreflight {
    configured_preflight(
        endpoint,
        vec![target()],
        OllamaLimits::default(),
        session_bytes,
    )
}

pub(super) fn configured_preflight(
    endpoint: OllamaEndpoint,
    targets: Vec<OllamaPreflightTarget>,
    limits: OllamaLimits,
    session_bytes: usize,
) -> OllamaSingleConnectionPreflight {
    OllamaSingleConnectionPreflight::new(endpoint, targets, limits, session_bytes)
        .expect("single connection preflight")
}

async fn serve(
    listener: TcpListener,
    mode: FirstResponseMode,
    target_count: usize,
    request_count: Arc<AtomicUsize>,
    response_count: Arc<AtomicUsize>,
) -> io::Result<ServerResult> {
    let (mut stream, _client) = listener.accept().await?;
    let mut accepts = 1;
    let mut requests = Vec::new();
    let mut buffer = Vec::new();
    let mut saw_disconnect = false;
    for ordinal in 1..=target_count + 6 {
        let request = match read_request(&mut stream, &mut buffer).await {
            Ok(Some(request)) => request,
            Ok(None) => {
                saw_disconnect = true;
                break;
            }
            Err(error) if is_disconnect(&error) => {
                saw_disconnect = true;
                break;
            }
            Err(error) => return Err(error),
        };
        request_count.store(ordinal, Ordering::SeqCst);
        requests.push(request.clone());
        let path = request
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| io::Error::other("missing request path"))?;
        if let Err(error) = write_response(&mut stream, path, mode, ordinal, target_count).await {
            if is_disconnect(&error) {
                saw_disconnect = true;
                break;
            }
            return Err(error);
        }
        response_count.store(ordinal, Ordering::SeqCst);
        if ordinal == 1
            && matches!(
                mode,
                FirstResponseMode::ConnectionClose | FirstResponseMode::SilentClose
            )
        {
            break;
        }
    }
    let mut probe = [0_u8; 1];
    let probe_result = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut probe)).await;
    let client_closed = saw_disconnect
        || match probe_result {
            Ok(Ok(0)) => true,
            Ok(Err(error)) if is_disconnect(&error) => true,
            _ => false,
        };
    drop(stream);
    if tokio::time::timeout(Duration::from_millis(100), listener.accept())
        .await
        .is_ok()
    {
        accepts += 1;
    }
    Ok(ServerResult {
        accepts,
        requests,
        client_closed,
    })
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

async fn read_request(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> io::Result<Option<String>> {
    loop {
        if let Some(header_end) = find_header_end(buffer) {
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
                let request = headers
                    .lines()
                    .next()
                    .ok_or_else(|| io::Error::other("missing request line"))?;
                let request = request
                    .strip_suffix(" HTTP/1.1")
                    .ok_or_else(|| io::Error::other("unexpected HTTP version"))?
                    .to_owned();
                buffer.drain(..total);
                return Ok(Some(request));
            }
        }
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(None);
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn write_response(
    stream: &mut TcpStream,
    path: &str,
    mode: FirstResponseMode,
    ordinal: usize,
    target_count: usize,
) -> io::Result<()> {
    if ordinal == 1 && matches!(mode, FirstResponseMode::StallHeaders) {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let body = response_body(path, mode, ordinal, target_count);
    if ordinal == 1 && matches!(mode, FirstResponseMode::ActualTrailer) {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n0\r\nX-Fixture: present\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).await?;
        return stream.flush().await;
    }
    let extra_headers = response_extra_headers(mode, ordinal);
    let status_line = response_status_line(mode, ordinal);
    let content_type = if ordinal == 1 && matches!(mode, FirstResponseMode::WrongContentType) {
        "text/plain"
    } else {
        "application/json"
    };
    let head = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra_headers}\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    let split = body.len() / 2;
    stream.write_all(&body.as_bytes()[..split]).await?;
    if ordinal == 1 && matches!(mode, FirstResponseMode::StallBody) {
        tokio::time::sleep(Duration::from_millis(200)).await;
    } else {
        tokio::task::yield_now().await;
    }
    stream.write_all(&body.as_bytes()[split..]).await?;
    stream.flush().await
}

fn response_extra_headers(mode: FirstResponseMode, ordinal: usize) -> &'static str {
    if ordinal != 1 {
        return "";
    }
    match mode {
        FirstResponseMode::ConnectionClose => "Connection: close\r\n",
        FirstResponseMode::Upgrade => "Connection: upgrade\r\nUpgrade: fixture\r\n",
        FirstResponseMode::UpgradeHeaderOnly => "Upgrade: fixture\r\n",
        FirstResponseMode::DeclaredTrailer => "Trailer: X-Fixture\r\n",
        _ => "",
    }
}

fn response_status_line(mode: FirstResponseMode, ordinal: usize) -> &'static str {
    if ordinal != 1 {
        return "HTTP/1.1 200 OK";
    }
    match mode {
        FirstResponseMode::Http10 => "HTTP/1.0 200 OK",
        FirstResponseMode::SwitchingProtocols => "HTTP/1.1 101 Switching Protocols",
        FirstResponseMode::NotFound => "HTTP/1.1 404 Not Found",
        FirstResponseMode::TooManyRequests => "HTTP/1.1 429 Too Many Requests",
        FirstResponseMode::Rejected => "HTTP/1.1 400 Bad Request",
        _ => "HTTP/1.1 200 OK",
    }
}

pub(super) fn response_body(
    path: &str,
    mode: FirstResponseMode,
    ordinal: usize,
    target_count: usize,
) -> String {
    if ordinal == 1 && matches!(mode, FirstResponseMode::InvalidJson) {
        return "not-json".to_owned();
    }
    match path {
        "/api/version"
            if ordinal > 1 && matches!(mode, FirstResponseMode::RuntimeDrift) =>
        {
            r#"{"version":"0.32.15"}"#.to_owned()
        }
        "/api/version" => r#"{"version":"0.32.14"}"#.to_owned(),
        "/api/tags" if target_count == 2 => format!(
            r#"{{"models":[{{"name":"fixture:latest","model":"fixture:latest","size":1024,"digest":"{DIGEST}"}},{{"name":"second:latest","model":"second:latest","size":2048,"digest":"{SECOND_DIGEST}"}}]}}"#
        ),
        "/api/tags" => format!(
            r#"{{"models":[{{"name":"fixture:latest","model":"fixture:latest","size":1024,"digest":"{DIGEST}"}}]}}"#
        ),
        "/api/ps" => r#"{"models":[]}"#.to_owned(),
        "/api/show" => {
            r#"{"license":"fixture license","template":"fixture template","capabilities":["completion"],"details":{"format":"gguf","family":"fixture","quantization_level":"Q4_K_M"},"model_info":{"fixture.context_length":4096}}"#.to_owned()
        }
        _ => r#"{"error":"unexpected path"}"#.to_owned(),
    }
}
