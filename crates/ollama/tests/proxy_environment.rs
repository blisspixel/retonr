use std::{
    io::{ErrorKind, Read as _, Write as _},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use rewrite_inference::{InferenceBackend as _, OperationContext};
use rewrite_ollama::{OllamaBackend, OllamaEndpoint, OllamaLimits};
use rewrite_types::CancellationToken;

const CHILD_ENDPOINT_ENV: &str = "REWRITE_OLLAMA_PROXY_TEST_ENDPOINT";
const PROXY_ENVIRONMENT: [&str; 8] = [
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
];
const TESTED_PROXY_ENVIRONMENT: [&str; 4] = ["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"];

#[derive(Clone, Copy)]
enum ServerRole {
    Target,
    Proxy,
}

struct TestServer {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    worker: thread::JoinHandle<()>,
}

impl TestServer {
    fn spawn(role: ServerRole) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
        listener
            .set_nonblocking(true)
            .expect("make test server nonblocking");
        let address = listener.local_addr().expect("read test server address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let worker_requests = Arc::clone(&requests);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            while !worker_stop.load(Ordering::SeqCst) && Instant::now() < deadline {
                match listener.accept() {
                    Ok((stream, _peer)) => {
                        serve_connection(stream, role, &worker_requests);
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept test connection: {error}"),
                }
            }
        });
        Self {
            address,
            requests,
            stop,
            worker,
        }
    }

    fn uri(&self) -> String {
        format!("http://{}", self.address)
    }

    fn finish(self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.join().expect("test server joins");
        Arc::try_unwrap(self.requests)
            .expect("test request recorder has one owner")
            .into_inner()
            .expect("test request recorder is not poisoned")
    }
}

fn serve_connection(mut stream: TcpStream, role: ServerRole, requests: &Arc<Mutex<Vec<String>>>) {
    stream
        .set_nonblocking(false)
        .expect("make accepted test stream blocking");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set test read timeout");
    let request = read_request(&mut stream);
    requests
        .lock()
        .expect("test request recorder is not poisoned")
        .push(request.clone());
    let (status, body) = match role {
        ServerRole::Proxy if request.contains("/proxy-control ") => {
            ("200 OK", r#"{"control":true}"#)
        }
        ServerRole::Proxy => ("502 Bad Gateway", r#"{"error":"adapter used proxy"}"#),
        ServerRole::Target if request.starts_with("GET /api/version ") => {
            ("200 OK", r#"{"version":"0.13.0"}"#)
        }
        ServerRole::Target if request.starts_with("GET /api/tags ") => {
            ("200 OK", r#"{"models":[]}"#)
        }
        ServerRole::Target => ("404 Not Found", r#"{"error":"unexpected path"}"#),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .expect("write test response");
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    while request.len() < 8 * 1024 && !request.windows(4).any(|value| value == b"\r\n\r\n") {
        let read = stream.read(&mut buffer).expect("read test request");
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

#[test]
fn ignores_ambient_proxy_environment() {
    let target = TestServer::spawn(ServerRole::Target);
    let proxy = TestServer::spawn(ServerRole::Proxy);
    let proxy_uri = proxy.uri();
    let target_uri = target.uri();
    for proxy_variable in TESTED_PROXY_ENVIRONMENT {
        let output = run_proxy_child(&target_uri, &proxy_uri, proxy_variable);
        assert!(
            output.status.success(),
            "proxy test child for {proxy_variable} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let target_requests = target.finish();
    let proxy_requests = proxy.finish();
    let mut expected_target_requests = Vec::new();
    for _proxy_variable in TESTED_PROXY_ENVIRONMENT {
        expected_target_requests.push("GET /api/version HTTP/1.1".to_owned());
        expected_target_requests.push("GET /api/tags HTTP/1.1".to_owned());
    }
    assert_eq!(
        target_requests, expected_target_requests,
        "the adapter did not use only its configured loopback target"
    );
    assert_eq!(
        proxy_requests,
        vec![format!("GET {target_uri}/proxy-control HTTP/1.1"); TESTED_PROXY_ENVIRONMENT.len()],
        "the proxy did not receive exactly the positive-control requests"
    );
}

fn run_proxy_child(target_uri: &str, proxy_uri: &str, proxy_variable: &str) -> Output {
    let mut command =
        Command::new(std::env::current_exe().expect("locate integration test binary"));
    command.args([
        "--exact",
        "proxy_environment_child",
        "--ignored",
        "--nocapture",
    ]);
    for variable in PROXY_ENVIRONMENT {
        command.env_remove(variable);
    }
    command
        .env_remove("REQUEST_METHOD")
        .env(CHILD_ENDPOINT_ENV, target_uri)
        .env(proxy_variable, proxy_uri)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("start isolated proxy test child");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().expect("poll proxy test child").is_some() {
            return child.wait_with_output().expect("collect proxy test child");
        }
        if Instant::now() >= deadline {
            child.kill().expect("terminate timed out proxy test child");
            let output = child
                .wait_with_output()
                .expect("collect timed out proxy test child");
            panic!(
                "proxy test child for {proxy_variable} timed out\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test]
#[ignore = "invoked in an isolated child process by ignores_ambient_proxy_environment"]
async fn proxy_environment_child() {
    let Ok(endpoint) = std::env::var(CHILD_ENDPOINT_ENV) else {
        return;
    };
    let control_response = reqwest::Client::new()
        .get(format!("{endpoint}/proxy-control"))
        .send()
        .await
        .expect("control client reaches configured proxy");
    assert!(control_response.status().is_success());
    let backend = OllamaBackend::new(
        OllamaEndpoint::parse(&endpoint).expect("parse child test endpoint"),
        Vec::new(),
        OllamaLimits {
            connect_timeout: Duration::from_millis(500),
            read_timeout: Duration::from_secs(1),
            request_timeout: Duration::from_secs(2),
            ..OllamaLimits::default()
        },
    )
    .expect("build child test backend");
    let token = CancellationToken::new();
    let discovery = backend
        .discover(OperationContext::new(&token, None))
        .await
        .expect("discover directly despite poisoned proxy environment");
    assert_eq!(discovery.runtime.version, "0.13.0");
    assert!(discovery.inventory.is_empty());
}
