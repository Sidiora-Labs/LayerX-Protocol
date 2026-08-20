//! The loopback control surface shared by the hosted webhook and dashboard
//! services. Authentication is terminated by the hosted gateway in front, which
//! forwards the authenticated principal, so this listener refuses to bind
//! anything other than a loopback address.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use crate::error::WebhookError;

/// Largest request the control surface reads.
pub const MAXIMUM_REQUEST_BYTES: usize = 128 * 1024;
/// Header carrying the principal the hosted gateway authenticated.
pub const PRINCIPAL_HEADER: &str = "x-layerx-principal";

const IO_TIMEOUT: Duration = Duration::from_secs(15);

/// One parsed request.
#[derive(Clone, Debug)]
pub struct Request {
    /// Request method in upper case.
    pub method: String,
    /// Request path without its query string.
    pub path: String,
    /// Raw query string without the leading question mark.
    pub query: String,
    /// Header names lowercased.
    pub headers: BTreeMap<String, String>,
    /// Exact request body bytes.
    pub body: Vec<u8>,
}

impl Request {
    /// Borrows one header by its lowercase name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    /// Borrows one query parameter value.
    #[must_use]
    pub fn parameter(&self, name: &str) -> Option<&str> {
        self.query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == name).then_some(value)
        })
    }

    /// Returns the non-empty path segments.
    #[must_use]
    pub fn segments(&self) -> Vec<&str> {
        self.path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect()
    }
}

/// One reply.
#[derive(Clone, Debug)]
pub struct Reply {
    /// HTTP status.
    pub status: u16,
    /// JSON body.
    pub body: String,
    /// Retry timing carried on a refusal that is worth retrying.
    pub retry_after: Option<u64>,
}

impl Reply {
    /// Builds a JSON reply.
    #[must_use]
    pub fn json(status: u16, body: String) -> Self {
        Self {
            status,
            body,
            retry_after: None,
        }
    }

    /// Builds a typed refusal carrying explicit retry timing.
    #[must_use]
    pub fn refusal(status: u16, code: &str, retry_after: Option<u64>) -> Self {
        let timing = retry_after.map_or_else(
            || "\"never\"".to_owned(),
            |seconds| format!("\"after\",\"retry_after_seconds\":{seconds}"),
        );
        Self {
            status,
            body: format!("{{\"error\":{{\"code\":\"{code}\",\"retry\":{timing}}}}}"),
            retry_after,
        }
    }
}

/// Returns true when the listen address is a loopback address.
#[must_use]
pub fn loopback(listen: &str) -> bool {
    let host = listen.rsplit_once(':').map_or(listen, |(host, _)| host);
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

/// Reads one request from the stream.
///
/// # Errors
/// Returns [`WebhookError::InvalidRequest`] for a malformed or oversized request
/// and [`WebhookError::Io`] when the stream fails.
pub fn read_request(stream: &mut TcpStream) -> Result<Request, WebhookError> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 || bytes.len().saturating_add(count) > MAXIMUM_REQUEST_BYTES {
            return Err(WebhookError::InvalidRequest);
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position.saturating_add(4);
        }
    };
    let source =
        std::str::from_utf8(&bytes[..header_end]).map_err(|_| WebhookError::InvalidRequest)?;
    let mut lines = source.split("\r\n");
    let mut first = lines
        .next()
        .ok_or(WebhookError::InvalidRequest)?
        .split_whitespace();
    let method = first
        .next()
        .ok_or(WebhookError::InvalidRequest)?
        .to_ascii_uppercase();
    let target = first.next().ok_or(WebhookError::InvalidRequest)?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let path = path.to_owned();
    let query = query.to_owned();
    let mut headers = BTreeMap::new();
    let mut content_length = 0_usize;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or(WebhookError::InvalidRequest)?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        if name == "content-length" {
            content_length = value.parse().map_err(|_| WebhookError::InvalidRequest)?;
        }
        headers.insert(name, value);
    }
    let total = header_end.saturating_add(content_length);
    if total > MAXIMUM_REQUEST_BYTES {
        return Err(WebhookError::InvalidRequest);
    }
    while bytes.len() < total {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(WebhookError::InvalidRequest);
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(Request {
        method,
        path,
        query,
        headers,
        body: bytes
            .get(header_end..total)
            .ok_or(WebhookError::InvalidRequest)?
            .to_vec(),
    })
}

/// Writes one reply and closes the connection.
///
/// # Errors
/// Returns [`WebhookError::Io`] when the stream fails.
pub fn write_reply(stream: &mut TcpStream, reply: &Reply) -> Result<(), WebhookError> {
    let reason = match reply.status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Not Found",
    };
    let retry = reply
        .retry_after
        .map_or_else(String::new, |seconds| format!("Retry-After: {seconds}\r\n"));
    write!(
        stream,
        "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n{retry}Connection: close\r\n\r\n{}",
        reply.status,
        reply.body.len(),
        reply.body
    )?;
    stream.flush()?;
    Ok(())
}

/// Serves the loopback control surface until the process is stopped.
///
/// # Errors
/// Returns [`WebhookError::InvalidRequest`] when the address is not loopback and
/// [`WebhookError::Io`] when the address cannot be bound.
pub fn serve<F>(listen: &str, handler: F) -> Result<(), WebhookError>
where
    F: Fn(&Request) -> Reply,
{
    if !loopback(listen) {
        return Err(WebhookError::InvalidRequest);
    }
    let listener = TcpListener::bind(listen)?;
    for connection in listener.incoming() {
        let Ok(mut stream) = connection else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
        let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
        let reply = read_request(&mut stream).map_or_else(
            |_| Reply::refusal(400, "invalid_request", None),
            |request| handler(&request),
        );
        let _ = write_reply(&mut stream, &reply);
    }
    Ok(())
}
