//! The outbound leg of a delivery attempt.

use std::time::Duration;

use crate::deliveries::FailureKind;
use crate::error::WebhookError;

/// One prepared, already signed attempt.
#[derive(Clone, Copy, Debug)]
pub struct Attempt<'a> {
    /// Destination the body is posted to.
    pub url: &'a str,
    /// Signed delivery headers.
    pub headers: &'a [(String, String)],
    /// Exact bytes the signature commits to.
    pub payload: &'a [u8],
}

/// The outbound leg of one attempt. Implementations return the exact status the
/// destination answered with, or the exact reason no status was observed.
pub trait Transport: Send + Sync {
    /// Posts one signed delivery.
    ///
    /// # Errors
    /// Returns the exact [`FailureKind`] when no status was observed.
    fn post(&self, attempt: &Attempt<'_>) -> Result<u16, FailureKind>;
}

/// The production transport: one blocking HTTPS request per attempt with a hard
/// deadline, no redirect following and no response body retained.
pub struct HttpTransport {
    agent: ureq::Agent,
}

impl HttpTransport {
    /// Builds the transport with a per-attempt deadline.
    ///
    /// # Errors
    /// Returns [`WebhookError::InvalidRequest`] when the deadline is zero.
    pub fn new(deadline: Duration) -> Result<Self, WebhookError> {
        if deadline.is_zero() {
            return Err(WebhookError::InvalidRequest);
        }
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(deadline))
            .http_status_as_error(false)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .user_agent("LayerX-Webhooks/1")
            .build();
        Ok(Self {
            agent: config.into(),
        })
    }
}

impl Transport for HttpTransport {
    fn post(&self, attempt: &Attempt<'_>) -> Result<u16, FailureKind> {
        let mut request = self.agent.post(attempt.url);
        for (name, value) in attempt.headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let mut response = match request.send(attempt.payload) {
            Ok(response) => response,
            Err(ureq::Error::Timeout(_)) => return Err(FailureKind::Timeout),
            Err(ureq::Error::Protocol(_) | ureq::Error::Http(_) | ureq::Error::BadUri(_)) => {
                return Err(FailureKind::Protocol)
            }
            Err(_) => return Err(FailureKind::Unreachable),
        };
        let status = response.status().as_u16();
        if response.body_mut().read_to_string().is_err() {
            return Err(FailureKind::Protocol);
        }
        Ok(status)
    }
}
