use std::{future::Future, net::TcpStream as StandardTcpStream, time::Duration};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    Method,
    client::conn::http1::{SendRequest, handshake},
    header::HeaderValue,
};
use hyper_util::rt::TokioIo;
use rewrite_inference::{InferenceError, InferenceErrorKind, OperationContext};
use rewrite_model::RuntimeIdentity;
use serde::{Serialize, de::DeserializeOwned};
use tokio::{net::TcpStream as TokioTcpStream, task::JoinHandle};

use super::{OllamaConnectionAddresses, OllamaObservedPreflightError, OllamaResponseObservation};
use crate::{
    OllamaEndpoint,
    contract::{
        BACKEND_ID, MAX_VERSION_BYTES, OllamaLimits, OllamaModelDetails, OllamaRunningModel,
    },
    response::{
        await_context, malformed_error, parse_running_models, parse_show_details, policy_error,
        valid_text,
    },
    wire::{PsResponse, ShowRequest, ShowResponse, TagsResponse, VersionResponse},
};

mod exchange;

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

#[derive(Clone, Copy)]
pub(super) struct ResponseHeadCheckpoint {
    pub(super) at: std::time::Instant,
    pub(super) ordinal: usize,
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
            TokioTcpStream::connect(endpoint.socket_addr()),
        )
        .await?
        .map_err(|_error| retryable_error("connection_failed"))?;
        Self::from_tokio_stream(endpoint, limits, session_body_bytes, context, stream).await
    }

    pub(super) async fn from_connected_stream(
        endpoint: &OllamaEndpoint,
        limits: OllamaLimits,
        session_body_bytes: usize,
        context: OperationContext<'_>,
        stream: StandardTcpStream,
    ) -> Result<Self, InferenceError> {
        stream
            .set_nonblocking(true)
            .map_err(|_error| retryable_error("connection_configuration_failed"))?;
        let stream = TokioTcpStream::from_std(stream)
            .map_err(|_error| retryable_error("connection_configuration_failed"))?;
        Self::from_tokio_stream(endpoint, limits, session_body_bytes, context, stream).await
    }

    async fn from_tokio_stream(
        endpoint: &OllamaEndpoint,
        limits: OllamaLimits,
        session_body_bytes: usize,
        context: OperationContext<'_>,
        stream: TokioTcpStream,
    ) -> Result<Self, InferenceError> {
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

    pub(super) async fn generate<B, T, F, E>(
        &mut self,
        body: &B,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<T, OllamaObservedPreflightError<E>>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        self.send_json_with_limit(
            "api/generate",
            body,
            self.limits.generation_body_bytes,
            context,
            observer,
        )
        .await
    }

    pub(super) async fn generate_with_response_head<B, T, F, E>(
        &mut self,
        body: &B,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<(T, ResponseHeadCheckpoint, Vec<u8>), OllamaObservedPreflightError<E>>
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
        self.request_json_with_response_head(
            Method::POST,
            "api/generate",
            body,
            self.limits.generation_body_bytes,
            context,
            observer,
        )
        .await
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
        self.request_json(
            Method::GET,
            path,
            Bytes::new(),
            self.limits.discovery_body_bytes,
            context,
            observer,
        )
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
        self.send_json_with_limit(
            path,
            body,
            self.limits.discovery_body_bytes,
            context,
            observer,
        )
        .await
    }

    async fn send_json_with_limit<B, T, F, E>(
        &mut self,
        path: &str,
        body: &B,
        response_body_limit: usize,
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
        self.request_json(
            Method::POST,
            path,
            body,
            response_body_limit,
            context,
            observer,
        )
        .await
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
