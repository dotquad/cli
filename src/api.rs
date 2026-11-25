//! A small client for the Dotquad API (https://dotquad.app/docs/).

use std::fmt;
use std::time::Duration;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::Value;

pub const DEFAULT_API: &str = "https://api.dotquad.app";
pub const DEFAULT_SITE: &str = "https://dotquad.app";

/// Encodes everything but unreserved characters, as JavaScript's `encodeURIComponent` does.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

pub fn seg(s: &str) -> String {
    utf8_percent_encode(s.trim(), COMPONENT).to_string()
}

/// `?a=1&b=2`, skipping empty values.
pub fn query(params: &[(&str, Option<String>)]) -> String {
    let q: Vec<String> = params
        .iter()
        .filter_map(|(k, v)| v.as_ref().filter(|v| !v.is_empty()).map(|v| format!("{k}={}", seg(v))))
        .collect();
    if q.is_empty() { String::new() } else { format!("?{}", q.join("&")) }
}

/// An error answer from the API, with the fields that say what to do about it.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    /// On `rate_limited`: `per_second`, `per_minute`, `per_day` or `quota`.
    pub reason: Option<String>,
    pub retry_after: Option<u64>,
    pub upgrade_url: Option<String>,
    pub plan: Option<String>,
    /// The body as the API sent it, for `--json`.
    pub body: Option<Value>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

impl ApiError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        ApiError {
            code: code.into(),
            message: message.into(),
            reason: None,
            retry_after: None,
            upgrade_url: None,
            plan: None,
            body: None,
        }
    }

    fn from_body(status: u16, body: Value, retry_after: Option<u64>) -> Self {
        let err = body.get("error").filter(|e| e.is_object());
        let field = |k: &str| err.and_then(|e| e.get(k)).and_then(Value::as_str).map(String::from);
        ApiError {
            code: field("code").unwrap_or_else(|| "unexpected_response".into()),
            message: field("message").unwrap_or_else(|| format!("The Dotquad API answered {status}.")),
            reason: field("reason"),
            retry_after,
            upgrade_url: field("upgrade_url"),
            plan: field("plan"),
            body: Some(body),
        }
    }
}

pub struct Client {
    agent: ureq::Agent,
    api: String,
    key: Option<String>,
}

// ApiError is large for a Result, but a run makes one or two calls and stops at the first error.
#[allow(clippy::result_large_err)]
impl Client {
    pub fn new(api: &str, key: Option<String>) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            // The report card runs five checks and can take a while on a slow domain.
            .timeout_global(Some(Duration::from_secs(60)))
            .timeout_connect(Some(Duration::from_secs(10)))
            .http_status_as_error(false)
            .user_agent(concat!("dotquad-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Client { agent, api: api.trim_end_matches('/').to_string(), key }
    }

    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    /// `GET /v1{path}` with the key.
    pub fn v1(&self, path: &str) -> Result<Value, ApiError> {
        self.get(&format!("/v1{path}"), true)
    }

    /// One of the open terminal endpoints (`/json`, `/{ip}/json`, `/check/{domain}/json`),
    /// which need no key and are limited per visitor.
    pub fn open(&self, path: &str) -> Result<Value, ApiError> {
        self.get(path, false)
    }

    fn get(&self, path: &str, keyed: bool) -> Result<Value, ApiError> {
        let mut req = self.agent.get(format!("{}{path}", self.api)).header("accept", "application/json");
        if keyed && let Some(key) = &self.key {
            req = req.header("authorization", format!("Bearer {key}"));
        }
        let mut res = req.call().map_err(|e| {
            ApiError::new(
                "network_error",
                format!("Couldn't reach the Dotquad API ({e}). Check your connection and try again."),
            )
        })?;
        let status = res.status().as_u16();
        let retry_after = res
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|n| *n > 0);
        let text = res.body_mut().read_to_string().unwrap_or_default();
        let json: Option<Value> = serde_json::from_str(&text).ok();
        match json {
            Some(body) if (200..300).contains(&status) => Ok(body),
            Some(body) if body.get("error").is_some() => Err(ApiError::from_body(status, body, retry_after)),
            _ => {
                let snippet: String = text.chars().take(200).collect();
                let mut e = ApiError::new(
                    "unexpected_response",
                    if snippet.trim().is_empty() {
                        format!("The Dotquad API answered {status} with no data.")
                    } else {
                        format!("The Dotquad API answered {status}: {}", snippet.trim())
                    },
                );
                e.retry_after = retry_after;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_encode_uri_component() {
        assert_eq!(seg("10.0.0.0/22"), "10.0.0.0%2F22");
        assert_eq!(seg("https://a.b/c?d=e"), "https%3A%2F%2Fa.b%2Fc%3Fd%3De");
        assert_eq!(seg(" example.com "), "example.com");
    }

    #[test]
    fn builds_queries() {
        assert_eq!(query(&[("a", None), ("b", Some(String::new()))]), "");
        assert_eq!(query(&[("names", Some("www,api".into()))]), "?names=www%2Capi");
    }
}
