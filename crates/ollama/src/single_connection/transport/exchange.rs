use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::{
    Method, Request, Response, StatusCode, Version,
    body::{Body as _, Incoming},
    header::{self, HeaderMap},
};
use rewrite_inference::{InferenceError, OperationContext};
use serde::de::DeserializeOwned;

use super::{
    ResponseHeadCheckpoint, SingleConnectionTransport, await_timeout, remaining_timeout,
    retryable_error,
};
use crate::{
    response::{check_context, malformed_error, map_status, policy_error},
    single_connection::{
        OllamaObservedPreflightError, OllamaResponseObservation, OllamaResponseObservationPhase,
    },
};

impl SingleConnectionTransport {
    pub(super) async fn request_json<T, F, E>(
        &mut self,
        method: Method,
        path: &str,
        body: Bytes,
        response_body_limit: usize,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<T, OllamaObservedPreflightError<E>>
    where
        T: DeserializeOwned,
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        self.request_json_with_response_head(
            method,
            path,
            body,
            response_body_limit,
            context,
            observer,
        )
        .await
        .map(|(decoded, _checkpoint, _body)| decoded)
    }

    pub(super) async fn request_json_with_response_head<T, F, E>(
        &mut self,
        method: Method,
        path: &str,
        body: Bytes,
        response_body_limit: usize,
        context: OperationContext<'_>,
        observer: &mut F,
    ) -> Result<(T, ResponseHeadCheckpoint, Vec<u8>), OllamaObservedPreflightError<E>>
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
        let response_head = ResponseHeadCheckpoint {
            at: std::time::Instant::now(),
            ordinal: self.completed_responses.checked_add(1).ok_or_else(|| {
                OllamaObservedPreflightError::Preflight(malformed_error(
                    "response_ordinal_overflow",
                ))
            })?,
        };
        let bytes = self
            .read_response(response, response_body_limit, request_deadline, context)
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
        let decoded = serde_json::from_slice(&bytes).map_err(|_error| {
            OllamaObservedPreflightError::Preflight(malformed_error("invalid_json_response"))
        })?;
        Ok((decoded, response_head, bytes))
    }

    async fn read_response(
        &mut self,
        response: Response<Incoming>,
        body_limit: usize,
        request_deadline: tokio::time::Instant,
        context: OperationContext<'_>,
    ) -> Result<Vec<u8>, InferenceError> {
        validate_response_head(&response, body_limit)?;
        let mut body = response.into_body();
        let mut bytes = Vec::new();
        while let Some(frame) = self
            .next_frame(&mut body, request_deadline, context)
            .await?
        {
            let data = frame
                .into_data()
                .map_err(|_frame| malformed_error("unexpected_response_trailers"))?;
            self.consume_bytes(data.len(), bytes.len(), body_limit)?;
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

    fn consume_bytes(
        &mut self,
        chunk: usize,
        response_bytes: usize,
        body_limit: usize,
    ) -> Result<(), InferenceError> {
        let response_total = response_bytes
            .checked_add(chunk)
            .ok_or_else(|| malformed_error("response_body_too_large"))?;
        if response_total > body_limit {
            return Err(malformed_error("response_body_too_large"));
        }
        self.remaining_session_bytes = self
            .remaining_session_bytes
            .checked_sub(chunk)
            .ok_or_else(|| malformed_error("preflight_session_body_too_large"))?;
        Ok(())
    }

    pub(in crate::single_connection) async fn ensure_open(
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
