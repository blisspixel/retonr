use std::{future::Future, time::Duration};

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::{
    Method, Request, Response, StatusCode, Version,
    body::{Body as _, Incoming},
    client::conn::http1::{SendRequest, handshake},
    header::{self, HeaderMap, HeaderValue},
};
use hyper_util::rt::TokioIo;
use rewrite_inference::{InferenceError, InferenceErrorKind, OperationContext};
use rewrite_model::RuntimeIdentity;
use serde::{Serialize, de::DeserializeOwned};
use tokio::{net::TcpStream, task::JoinHandle};

use super::{
    OllamaConnectionAddresses, OllamaObservedPreflightError, OllamaResponseObservation,
    OllamaResponseObservationPhase,
};
use crate::{
    OllamaEndpoint,
    contract::{
        BACKEND_ID, MAX_VERSION_BYTES, OllamaLimits, OllamaModelDetails, OllamaRunningModel,
    },
    response::{
        await_context, check_context, malformed_error, map_status, parse_running_models,
        parse_show_details, policy_error, valid_text,
    },
    wire::{PsResponse, ShowRequest, ShowResponse, TagsResponse, VersionResponse},
};

pub(super) struct SingleConnectionTransport {
    sender: SendRequest<Full<Bytes>>,
    driver: ConnectionDriver,
    addresses: OllamaConnectionAddresses,
    host: HeaderValue,
    limits: OllamaLimits,
    remaining_session_bytes: usize,
    completed_responses: usize,
    response_attempt_in_progress: bool,
}

impl SingleConnectionTransport {
    pub(super) async fn connect(
        endpoint: &OllamaEndpoint,
        limits: OllamaLimits,
        session_body_bytes: usize,
        context: OperationContext<'_>,
    ) -> Result<Self, InferenceError> {
        let stream = await_timeout(
            context,
            limits.connect_timeout,
            TcpStream::connect(endpoint.socket_addr()),
        )
        .await?
        .map_err(|_error| retryable_error("connection_failed"))?;
        let client = stream
            .local_addr()
            .map_err(|_error| retryable_error("connection_address_failed"))?;
        let server = stream
            .peer_addr()
            .map_err(|_error| retryable_error("connection_address_failed"))?;
        if !client.ip().is_loopback() || server != endpoint.socket_addr() {
            return Err(policy_error("connection_endpoint_mismatch"));
        }
        let addresses = OllamaConnectionAddresses { client, server };
        let host = HeaderValue::from_str(&server.to_string())
            .map_err(|_error| policy_error("invalid_endpoint_authority"))?;
        let (sender, connection) = await_timeout(
            context,
            limits.connect_timeout,
            handshake(TokioIo::new(stream)),
        )
        .await?
        .map_err(|_error| retryable_error("http_handshake_failed"))?;
        let driver = ConnectionDriver::spawn(connection);
        Ok(Self {
            sender,
            driver,
            addresses,
            host,
            limits,
            remaining_session_bytes: session_body_bytes,
            completed_responses: 0,
            response_attempt_in_progress: false,
        })
    }

    pub(super) const fn addresses(&self) -> OllamaConnectionAddresses {
        self.addresses
    }

    pub(super) const fn completed_responses(&self) -> usize {
        self.completed_responses
    }

    pub(super) const fn response_attempt_in_progress(&self) -> bool {
        self.response_attempt_in_progress
    }

    pub(super) async fn runtime_identity<F, E>(
        &mut self,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<RuntimeIdentity, OllamaObservedPreflightError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let response: VersionResponse = self.get_json("api/version", context, observer).await?;
        if !valid_text(&response.version, MAX_VERSION_BYTES) {
            return Err(OllamaObservedPreflightError::Preflight(malformed_error(
                "invalid_runtime_version",
            )));
        }
        Ok(RuntimeIdentity {
            backend: BACKEND_ID.to_owned(),
            version: response.version,
            digest: None,
        })
    }

    pub(super) async fn tags<F, E>(
        &mut self,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<TagsResponse, OllamaObservedPreflightError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        self.get_json("api/tags", context, observer).await
    }

    pub(super) async fn running_models<F, E>(
        &mut self,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<Vec<OllamaRunningModel>, OllamaObservedPreflightError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let response: PsResponse = self.get_json("api/ps", context, observer).await?;
        parse_running_models(&response).map_err(OllamaObservedPreflightError::Preflight)
    }

    pub(super) async fn show_details<F, E>(
        &mut self,
        reference: &str,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<OllamaModelDetails, OllamaObservedPreflightError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let response: ShowResponse = self
            .send_json(
                "api/show",
                &ShowRequest {
                    model: reference,
                    verbose: true,
                },
                context,
                observer,
            )
            .await?;
        parse_show_details(response).map_err(OllamaObservedPreflightError::Preflight)
    }

    async fn get_json<T, F, E>(
        &mut self,
        path: &str,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<T, OllamaObservedPreflightError<E>>
    where
        T: DeserializeOwned,
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        self.request_json(Method::GET, path, Bytes::new(), context, observer)
            .await
    }

    async fn send_json<B, T, F, E>(
        &mut self,
        path: &str,
        body: &B,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<T, OllamaObservedPreflightError<E>>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let body = serde_json::to_vec(body)
            .map(Bytes::from)
            .map_err(|_error| {
                OllamaObservedPreflightError::Preflight(policy_error("invalid_json_request"))
            })?;
        self.request_json(Method::POST, path, body, context, observer)
            .await
    }

    async fn request_json<T, F, E>(
        &mut self,
        method: Method,
        path: &str,
        body: Bytes,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<T, OllamaObservedPreflightError<E>>
    where
        T: DeserializeOwned,
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let request_deadline = tokio::time::Instant::now()
            .checked_add(self.limits.request_timeout)
            .ok_or_else(|| {
                OllamaObservedPreflightError::Preflight(policy_error("invalid_limits"))
            })?;
        let mut builder = Request::builder()
            .method(method.clone())
            .uri(format!("/{path}"))
            .version(Version::HTTP_11)
            .header(header::HOST, self.host.clone())
            .header(header::ACCEPT, "application/json");
        if method == Method::POST {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        let request = builder.body(Full::new(body)).map_err(|_error| {
            OllamaObservedPreflightError::Preflight(policy_error("invalid_http_request"))
        })?;
        self.response_attempt_in_progress = true;
        let ready_timeout =
            remaining_timeout(request_deadline).map_err(OllamaObservedPreflightError::Preflight)?;
        await_timeout(context, ready_timeout, self.sender.ready())
            .await
            .map_err(OllamaObservedPreflightError::Preflight)?
            .map_err(|_error| {
                OllamaObservedPreflightError::Preflight(retryable_error("connection_closed"))
            })?;
        let response_timeout =
            remaining_timeout(request_deadline).map_err(OllamaObservedPreflightError::Preflight)?;
        let response = await_timeout(context, response_timeout, self.sender.send_request(request))
            .await
            .map_err(OllamaObservedPreflightError::Preflight)?
            .map_err(|_error| {
                OllamaObservedPreflightError::Preflight(retryable_error("transport_failed"))
            })?;
        let bytes = self
            .read_response(response, request_deadline, context)
            .await
            .map_err(OllamaObservedPreflightError::Preflight)?;
        self.response_attempt_in_progress = false;
        self.completed_responses = self.completed_responses.checked_add(1).ok_or_else(|| {
            OllamaObservedPreflightError::Preflight(malformed_error("response_ordinal_overflow"))
        })?;
        observer(OllamaResponseObservation {
            phase: OllamaResponseObservationPhase::AfterResponse {
                ordinal: self.completed_responses,
            },
            addresses: self.addresses,
        })
        .map_err(OllamaObservedPreflightError::Observation)?;
        check_context(context).map_err(OllamaObservedPreflightError::Preflight)?;
        serde_json::from_slice(&bytes).map_err(|_error| {
            OllamaObservedPreflightError::Preflight(malformed_error("invalid_json_response"))
        })
    }

    async fn read_response(
        &mut self,
        response: Response<Incoming>,
        request_deadline: tokio::time::Instant,
        context: OperationContext<'_>,
    ) -> Result<Vec<u8>, InferenceError> {
        validate_response_head(&response, self.limits.discovery_body_bytes)?;
        let mut body = response.into_body();
        let mut bytes = Vec::new();
        while let Some(frame) = self
            .next_frame(&mut body, request_deadline, context)
            .await?
        {
            let data = frame
                .into_data()
                .map_err(|_frame| malformed_error("unexpected_response_trailers"))?;
            self.consume_bytes(data.len(), bytes.len())?;
            bytes.extend_from_slice(&data);
        }
        Ok(bytes)
    }

    async fn next_frame(
        &self,
        body: &mut Incoming,
        request_deadline: tokio::time::Instant,
        context: OperationContext<'_>,
    ) -> Result<Option<hyper::body::Frame<Bytes>>, InferenceError> {
        let remaining = remaining_timeout(request_deadline)?;
        let wait = self.limits.read_timeout.min(remaining);
        await_timeout(context, wait, body.frame())
            .await?
            .transpose()
            .map_err(|_error| retryable_error("transport_failed"))
    }

    fn consume_bytes(&mut self, chunk: usize, response_bytes: usize) -> Result<(), InferenceError> {
        let response_total = response_bytes
            .checked_add(chunk)
            .ok_or_else(|| malformed_error("response_body_too_large"))?;
        if response_total > self.limits.discovery_body_bytes {
            return Err(malformed_error("response_body_too_large"));
        }
        self.remaining_session_bytes = self
            .remaining_session_bytes
            .checked_sub(chunk)
            .ok_or_else(|| malformed_error("preflight_session_body_too_large"))?;
        Ok(())
    }

    pub(super) async fn ensure_open(
        &mut self,
        context: OperationContext<'_>,
    ) -> Result<(), InferenceError> {
        tokio::task::yield_now().await;
        if self.driver.is_finished() {
            return Err(retryable_error("connection_closed"));
        }
        await_timeout(context, self.limits.read_timeout, self.sender.ready())
            .await?
            .map_err(|_error| retryable_error("connection_closed"))
    }
}

struct ConnectionDriver {
    handle: JoinHandle<Result<(), hyper::Error>>,
}

impl ConnectionDriver {
    fn spawn<I>(connection: hyper::client::conn::http1::Connection<I, Full<Bytes>>) -> Self
    where
        I: hyper::rt::Read + hyper::rt::Write + Send + Unpin + 'static,
    {
        let handle = tokio::spawn(connection);
        Self { handle }
    }

    fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}

impl Drop for ConnectionDriver {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

fn validate_response_head(
    response: &Response<Incoming>,
    body_limit: usize,
) -> Result<(), InferenceError> {
    if response.headers().contains_key(header::TRAILER) {
        return Err(malformed_error("unexpected_response_trailers"));
    }
    if response.version() != Version::HTTP_11
        || response.status() == StatusCode::SWITCHING_PROTOCOLS
        || response.headers().contains_key(header::UPGRADE)
        || connection_token(response.headers(), "close")?
        || connection_token(response.headers(), "upgrade")?
    {
        return Err(malformed_error("non_persistent_http_response"));
    }
    if !response.status().is_success() {
        return Err(map_status(response.status()));
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !content_type
        .split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(malformed_error("unexpected_content_type"));
    }
    if response
        .body()
        .size_hint()
        .upper()
        .is_some_and(|length| length > body_limit as u64)
    {
        return Err(malformed_error("response_body_too_large"));
    }
    Ok(())
}

fn connection_token(headers: &HeaderMap, expected: &str) -> Result<bool, InferenceError> {
    for value in headers.get_all(header::CONNECTION) {
        let value = value
            .to_str()
            .map_err(|_error| malformed_error("invalid_connection_header"))?;
        if value
            .split(',')
            .any(|token| token.trim().eq_ignore_ascii_case(expected))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn remaining_timeout(deadline: tokio::time::Instant) -> Result<Duration, InferenceError> {
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    if remaining.is_zero() {
        Err(deadline_error())
    } else {
        Ok(remaining)
    }
}

async fn await_timeout<T>(
    context: OperationContext<'_>,
    timeout: Duration,
    future: impl Future<Output = T>,
) -> Result<T, InferenceError> {
    match await_context(context, tokio::time::timeout(timeout, future)).await? {
        Ok(value) => Ok(value),
        Err(_elapsed) => Err(deadline_error()),
    }
}

fn retryable_error(code: &'static str) -> InferenceError {
    InferenceError::new(InferenceErrorKind::Retryable, code)
}

fn deadline_error() -> InferenceError {
    InferenceError::new(InferenceErrorKind::Deadline, "deadline")
}
