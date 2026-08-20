use std::net::{IpAddr, SocketAddr};

use thiserror::Error;

/// Validated loopback-only Ollama base URL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaEndpoint {
    url: reqwest::Url,
    socket: SocketAddr,
}

impl OllamaEndpoint {
    /// Parses and normalizes an HTTP endpoint with an IP-literal loopback host.
    ///
    /// Hostnames are intentionally rejected before DNS resolution. The endpoint
    /// cannot contain credentials, query, fragment, or a non-root path.
    ///
    /// # Errors
    ///
    /// Returns [`OllamaEndpointError`] unless every local-only invariant holds.
    pub fn parse(value: &str) -> Result<Self, OllamaEndpointError> {
        let mut url = reqwest::Url::parse(value).map_err(|_error| OllamaEndpointError)?;
        if url.scheme() != "http"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !matches!(url.path(), "" | "/")
        {
            return Err(OllamaEndpointError);
        }
        let host = url
            .host_str()
            .ok_or(OllamaEndpointError)?
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .map_err(|_error| OllamaEndpointError)?;
        if !host.is_loopback() {
            return Err(OllamaEndpointError);
        }
        if url.port() == Some(0) {
            return Err(OllamaEndpointError);
        }
        if url.port().is_none() {
            url.set_port(Some(11_434))
                .map_err(|()| OllamaEndpointError)?;
        }
        url.set_path("/");
        let port = url.port().ok_or(OllamaEndpointError)?;
        Ok(Self {
            url,
            socket: SocketAddr::new(host, port),
        })
    }

    pub(crate) fn join(&self, path: &str) -> Result<reqwest::Url, OllamaEndpointError> {
        self.url.join(path).map_err(|_error| OllamaEndpointError)
    }

    /// Returns the normalized endpoint string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    /// Returns the exact normalized loopback socket address.
    #[must_use]
    pub const fn socket_addr(&self) -> SocketAddr {
        self.socket
    }
}

/// Endpoint is malformed or could contact a non-loopback host.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Ollama endpoint must be an HTTP IP-literal loopback URL without credentials or path")]
pub struct OllamaEndpointError;

#[cfg(test)]
mod tests {
    use super::OllamaEndpoint;

    #[test]
    fn accepts_only_normalized_ip_literal_loopback_urls() {
        assert_eq!(
            OllamaEndpoint::parse("http://127.0.0.1")
                .expect("loopback endpoint")
                .as_str(),
            "http://127.0.0.1:11434/"
        );
        assert!(OllamaEndpoint::parse("http://[::1]:11434").is_ok());
        assert!(OllamaEndpoint::parse("http://localhost:11434").is_err());
        assert!(OllamaEndpoint::parse("http://192.168.1.10:11434").is_err());
        assert!(OllamaEndpoint::parse("https://127.0.0.1:11434").is_err());
        assert!(OllamaEndpoint::parse("http://user@127.0.0.1:11434").is_err());
        assert!(OllamaEndpoint::parse("http://127.0.0.1:11434/api").is_err());
        assert!(OllamaEndpoint::parse("http://127.0.0.1:0").is_err());
        assert_eq!(
            OllamaEndpoint::parse("http://127.0.0.1")
                .expect("loopback endpoint")
                .socket_addr(),
            "127.0.0.1:11434".parse().expect("socket address")
        );
    }
}
