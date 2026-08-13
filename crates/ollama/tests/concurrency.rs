use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use rewrite_inference::{
    BackendDiscovery, InferenceBackend as _, InferenceError, InferenceErrorKind, OperationContext,
};
use rewrite_ollama::{OllamaBackend, OllamaEndpoint, OllamaLimits};
use rewrite_types::CancellationToken;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, oneshot},
    task::{JoinHandle, JoinSet},
};

struct DiscoveryServer {
    endpoint: String,
    requests: Arc<Mutex<Vec<String>>>,
    first_seen: oneshot::Receiver<()>,
    release_first: Arc<Semaphore>,
    worker: JoinHandle<()>,
}

impl DiscoveryServer {
    async fn start(expected_requests: usize, gate_first: bool, fail_first: bool) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind discovery server");
        let address = listener.local_addr().expect("read discovery address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let worker_requests = Arc::clone(&requests);
        let release_first = Arc::new(Semaphore::new(usize::from(!gate_first)));
        let worker_release = Arc::clone(&release_first);
        let (first_seen_tx, first_seen) = oneshot::channel();
        let worker = tokio::spawn(async move {
            let mut first_seen_tx = Some(first_seen_tx);
            let mut handlers = JoinSet::new();
            for ordinal in 0..expected_requests {
                let (stream, _peer) = listener.accept().await.expect("accept discovery request");
                let handler_requests = Arc::clone(&worker_requests);
                let handler_release = Arc::clone(&worker_release);
                let handler_first_seen = if ordinal == 0 {
                    first_seen_tx.take()
                } else {
                    None
                };
                handlers.spawn(async move {
                    serve_discovery_request(
                        stream,
                        ordinal,
                        fail_first,
                        handler_requests,
                        handler_first_seen,
                        handler_release,
                    )
                    .await;
                });
            }
            while let Some(result) = handlers.join_next().await {
                result.expect("discovery request handler joins");
            }
        });
        Self {
            endpoint: format!("http://{address}"),
            requests,
            first_seen,
            release_first,
            worker,
        }
    }

    fn backend(&self, max_concurrency: usize) -> OllamaBackend {
        OllamaBackend::new(
            OllamaEndpoint::parse(&self.endpoint).expect("parse discovery endpoint"),
            Vec::new(),
            OllamaLimits {
                max_concurrency,
                read_timeout: Duration::from_secs(5),
                request_timeout: Duration::from_secs(5),
                ..OllamaLimits::default()
            },
        )
        .expect("build discovery backend")
    }

    async fn wait_for_first_request(&mut self) {
        (&mut self.first_seen)
            .await
            .expect("observe first discovery request");
    }

    fn release_first_request(&self) {
        self.release_first.add_permits(1);
    }

    fn request_count(&self) -> usize {
        self.requests
            .lock()
            .expect("request recorder is not poisoned")
            .len()
    }

    async fn finish(self) -> Vec<String> {
        self.worker.await.expect("discovery server joins");
        Arc::try_unwrap(self.requests)
            .expect("request recorder has one owner")
            .into_inner()
            .expect("request recorder is not poisoned")
    }
}

async fn serve_discovery_request(
    mut stream: TcpStream,
    ordinal: usize,
    fail_first: bool,
    requests: Arc<Mutex<Vec<String>>>,
    first_seen: Option<oneshot::Sender<()>>,
    release_first: Arc<Semaphore>,
) {
    let request = read_request(&mut stream).await;
    requests
        .lock()
        .expect("request recorder is not poisoned")
        .push(request.clone());
    if let Some(first_seen) = first_seen {
        let _observed = first_seen.send(());
        let permit = release_first
            .acquire()
            .await
            .expect("first request gate remains open");
        permit.forget();
    }
    let (status, body) = if ordinal == 0 && fail_first {
        ("500 Internal Server Error", r#"{"error":"fixture"}"#)
    } else if request.starts_with("GET /api/version ") {
        ("200 OK", r#"{"version":"0.13.0"}"#)
    } else if request.starts_with("GET /api/tags ") {
        ("200 OK", r#"{"models":[]}"#)
    } else {
        ("404 Not Found", r#"{"error":"unexpected path"}"#)
    };
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("write discovery response");
}

async fn read_request(stream: &mut TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    while request.len() < 8 * 1024 && !request.windows(4).any(|value| value == b"\r\n\r\n") {
        let read = stream
            .read(&mut buffer)
            .await
            .expect("read discovery request");
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8_lossy(&request)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn spawn_discovery(
    backend: OllamaBackend,
    cancellation: CancellationToken,
    deadline: Option<Instant>,
) -> (
    JoinHandle<Result<BackendDiscovery, InferenceError>>,
    oneshot::Receiver<()>,
) {
    let (started_tx, started) = oneshot::channel();
    let worker = tokio::spawn(async move {
        let _started = started_tx.send(());
        backend
            .discover(OperationContext::new(&cancellation, deadline))
            .await
    });
    (worker, started)
}

#[tokio::test]
async fn admits_operations_up_to_the_configured_limit() {
    let mut server = DiscoveryServer::start(4, true, false).await;
    let backend = server.backend(2);
    let (first, _first_started) = spawn_discovery(backend.clone(), CancellationToken::new(), None);
    server.wait_for_first_request().await;
    let (second, _second_started) = spawn_discovery(backend, CancellationToken::new(), None);
    tokio::time::timeout(Duration::from_secs(5), second)
        .await
        .expect("second discovery is admitted")
        .expect("second discovery joins")
        .expect("second discovery succeeds");
    assert_eq!(server.request_count(), 3);
    server.release_first_request();
    first
        .await
        .expect("first discovery joins")
        .expect("first discovery succeeds");
    assert_eq!(
        server.finish().await,
        [
            "GET /api/version HTTP/1.1",
            "GET /api/version HTTP/1.1",
            "GET /api/tags HTTP/1.1",
            "GET /api/tags HTTP/1.1",
        ]
    );
}

#[tokio::test]
async fn queued_cancellation_and_deadline_do_no_network_work() {
    let mut server = DiscoveryServer::start(4, true, false).await;
    let backend = server.backend(1);
    let (first, _first_started) = spawn_discovery(backend.clone(), CancellationToken::new(), None);
    server.wait_for_first_request().await;

    let cancellation = CancellationToken::new();
    let (cancelled, cancelled_started) =
        spawn_discovery(backend.clone(), cancellation.clone(), None);
    let queued_deadline = Instant::now() + Duration::from_secs(1);
    let (deadline, deadline_started) = spawn_discovery(
        backend.clone(),
        CancellationToken::new(),
        Some(queued_deadline),
    );
    cancelled_started
        .await
        .expect("cancelled discovery reaches the backend");
    deadline_started
        .await
        .expect("deadline discovery reaches the backend");
    assert!(Instant::now() < queued_deadline);
    assert_eq!(server.request_count(), 1);
    cancellation.cancel();
    let cancelled_error = cancelled
        .await
        .expect("cancelled discovery joins")
        .expect_err("queued cancellation fails");
    let deadline_error = deadline
        .await
        .expect("deadline discovery joins")
        .expect_err("queued deadline fails");
    assert_eq!(cancelled_error.kind, InferenceErrorKind::Cancelled);
    assert_eq!(cancelled_error.code, "cancelled");
    assert_eq!(deadline_error.kind, InferenceErrorKind::Deadline);
    assert_eq!(deadline_error.code, "deadline");
    assert_eq!(server.request_count(), 1);

    server.release_first_request();
    first
        .await
        .expect("first discovery joins")
        .expect("first discovery succeeds");
    let (recovery, _recovery_started) = spawn_discovery(backend, CancellationToken::new(), None);
    recovery
        .await
        .expect("recovery discovery joins")
        .expect("recovery discovery succeeds");
    assert_eq!(
        server.finish().await,
        [
            "GET /api/version HTTP/1.1",
            "GET /api/tags HTTP/1.1",
            "GET /api/version HTTP/1.1",
            "GET /api/tags HTTP/1.1",
        ]
    );
}

#[tokio::test]
async fn releases_the_permit_after_an_in_flight_failure() {
    let mut server = DiscoveryServer::start(3, true, true).await;
    let backend = server.backend(1);
    let (first, _first_started) = spawn_discovery(backend.clone(), CancellationToken::new(), None);
    server.wait_for_first_request().await;
    let (second, second_started) = spawn_discovery(backend, CancellationToken::new(), None);
    second_started
        .await
        .expect("queued discovery reaches the backend");
    assert_eq!(server.request_count(), 1);
    server.release_first_request();
    let first_error = first
        .await
        .expect("failed discovery joins")
        .expect_err("first discovery fails");
    assert_eq!(first_error.kind, InferenceErrorKind::Retryable);
    assert_eq!(first_error.code, "http_transient");
    tokio::time::timeout(Duration::from_secs(5), second)
        .await
        .expect("queued discovery receives recovered permit")
        .expect("queued discovery joins")
        .expect("queued discovery succeeds");
    assert_eq!(
        server.finish().await,
        [
            "GET /api/version HTTP/1.1",
            "GET /api/version HTTP/1.1",
            "GET /api/tags HTTP/1.1",
        ]
    );
}
