//! Deterministic HTTP-shaped transport scaffolding.
//!
//! Provider connectors describe real endpoint paths, query parameters, and
//! JSON bodies. Tests supply [`MockHttp`]; credentials and a live network
//! implementation deliberately do not exist yet.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestBody {
    Empty,
    Json(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: RequestBody,
}

impl HttpRequest {
    #[must_use]
    pub fn get(path: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Get,
            path: path.into(),
            query: Vec::new(),
            body: RequestBody::Empty,
        }
    }

    #[must_use]
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.push((key.into(), value.into()));
        self
    }

    pub fn post_json(
        path: impl Into<String>,
        body: impl Serialize,
    ) -> Result<Self, TransportError> {
        Self::json(HttpMethod::Post, path, body)
    }

    pub fn put_json(path: impl Into<String>, body: impl Serialize) -> Result<Self, TransportError> {
        Self::json(HttpMethod::Put, path, body)
    }

    fn json(
        method: HttpMethod,
        path: impl Into<String>,
        body: impl Serialize,
    ) -> Result<Self, TransportError> {
        Ok(Self {
            method,
            path: path.into(),
            query: Vec::new(),
            body: RequestBody::Json(
                serde_json::to_value(body).map_err(TransportError::FixtureJson)?,
            ),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpStatus(u16);

impl HttpStatus {
    pub fn parse(code: u16) -> Result<Self, TransportError> {
        (100..=599)
            .contains(&code)
            .then_some(Self(code))
            .ok_or(TransportError::InvalidStatus(code))
    }

    #[must_use]
    pub const fn code(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_success(self) -> bool {
        self.0 >= 200 && self.0 < 300
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: HttpStatus,
    pub body: Box<[u8]>,
}

impl HttpResponse {
    pub fn json<T: DeserializeOwned>(self) -> Result<T, TransportError> {
        if !self.status.is_success() {
            return Err(TransportError::Http(self.status));
        }
        serde_json::from_slice(&self.body).map_err(TransportError::ResponseJson)
    }
}

pub trait HttpTransport: Send {
    fn execute(&mut self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}

#[derive(Debug, Clone)]
struct Exchange {
    request: HttpRequest,
    response: HttpResponse,
}

#[derive(Debug, Default)]
pub struct MockHttp {
    exchanges: VecDeque<Exchange>,
    observed: Vec<HttpRequest>,
}

impl MockHttp {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn expect_json(
        &mut self,
        request: HttpRequest,
        status: u16,
        body: impl Serialize,
    ) -> Result<(), TransportError> {
        let body = serde_json::to_vec(&body).map_err(TransportError::FixtureJson)?;
        self.exchanges.push_back(Exchange {
            request,
            response: HttpResponse {
                status: HttpStatus::parse(status)?,
                body: body.into_boxed_slice(),
            },
        });
        Ok(())
    }

    pub fn finish(self) -> Result<Vec<HttpRequest>, TransportError> {
        if self.exchanges.is_empty() {
            Ok(self.observed)
        } else {
            Err(TransportError::UnconsumedExchanges(self.exchanges.len()))
        }
    }
}

impl HttpTransport for MockHttp {
    fn execute(&mut self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let exchange = self
            .exchanges
            .pop_front()
            .ok_or_else(|| TransportError::UnexpectedRequest(Box::new(request.clone())))?;
        if exchange.request != request {
            return Err(TransportError::RequestMismatch {
                expected: Box::new(exchange.request),
                actual: Box::new(request),
            });
        }
        self.observed.push(request);
        Ok(exchange.response)
    }
}

#[derive(Debug)]
pub enum TransportError {
    InvalidStatus(u16),
    FixtureJson(serde_json::Error),
    ResponseJson(serde_json::Error),
    Http(HttpStatus),
    UnexpectedRequest(Box<HttpRequest>),
    RequestMismatch {
        expected: Box<HttpRequest>,
        actual: Box<HttpRequest>,
    },
    UnconsumedExchanges(usize),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStatus(code) => write!(f, "invalid HTTP status {code}"),
            Self::FixtureJson(error) => write!(f, "could not encode mock JSON: {error}"),
            Self::ResponseJson(error) => write!(f, "could not decode response JSON: {error}"),
            Self::Http(status) => write!(f, "provider returned HTTP {}", status.code()),
            Self::UnexpectedRequest(request) => write!(
                f,
                "unexpected request {:?} {}",
                request.method, request.path
            ),
            Self::RequestMismatch { expected, actual } => write!(
                f,
                "request mismatch: expected {:?} {}, got {:?} {}",
                expected.method, expected.path, actual.method, actual.path
            ),
            Self::UnconsumedExchanges(count) => {
                write!(f, "{count} mock HTTP exchanges were not consumed")
            }
        }
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FixtureJson(error) | Self::ResponseJson(error) => Some(error),
            Self::InvalidStatus(_)
            | Self::Http(_)
            | Self::UnexpectedRequest(_)
            | Self::RequestMismatch { .. }
            | Self::UnconsumedExchanges(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Fixture {
        value: u64,
    }

    #[test]
    fn mock_http_is_an_exact_protocol_transcript() {
        let request = HttpRequest::get("/v1/items").query("after", "41");
        let mut http = MockHttp::new();
        http.expect_json(request.clone(), 200, Fixture { value: 42 })
            .expect("fixture");
        let response = http.execute(request).expect("exchange");
        assert_eq!(
            response.json::<Fixture>().expect("json"),
            Fixture { value: 42 }
        );
        assert_eq!(http.finish().expect("complete").len(), 1);
    }
}
