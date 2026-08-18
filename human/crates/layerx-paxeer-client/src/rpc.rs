use std::io::{Read as _, Write as _};
use std::net::{TcpStream, ToSocketAddrs as _};
use std::time::Duration;

use crate::json::{self, Json};

/// One declared Paxeer JSON-RPC endpoint with its request timeout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointConfig {
    pub url: String,
    pub request_timeout: Duration,
}

/// Typed reason one endpoint could not serve one request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EndpointFault {
    UnsupportedUrl,
    Connect { detail: String },
    Transport { detail: String },
    Http { status: u16 },
    MalformedResponse,
    Rpc { code: i64, message: String },
    UnexpectedValue { detail: String },
}

/// One endpoint's failure, naming the endpoint it came from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointFailure {
    pub url: String,
    pub fault: EndpointFault,
}

pub(crate) struct Target {
    host: String,
    port: u16,
    path: String,
}

pub(crate) fn parse_url(url: &str) -> Option<Target> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(index) => rest.split_at(index),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rfind(':') {
        Some(index) => {
            let host = authority.get(..index)?;
            let port = authority.get(index.saturating_add(1)..)?.parse().ok()?;
            (host, port)
        }
        None => (authority, 80),
    };
    if host.is_empty() {
        return None;
    }
    Some(Target {
        host: host.to_owned(),
        port,
        path: path.to_owned(),
    })
}

/// Issues one JSON-RPC request against one configured endpoint.
///
/// # Errors
///
/// Returns the endpoint's typed fault: an unsupported URL, a connect or
/// transport failure, a non-200 HTTP status, a malformed response body, or
/// the JSON-RPC error the endpoint reported.
pub fn raw_call(
    endpoint: &EndpointConfig,
    method: &str,
    params: &[Json],
) -> Result<Json, EndpointFailure> {
    call_target(endpoint, method, params).map_err(|fault| EndpointFailure {
        url: endpoint.url.clone(),
        fault,
    })
}

fn call_target(
    endpoint: &EndpointConfig,
    method: &str,
    params: &[Json],
) -> Result<Json, EndpointFault> {
    let target = parse_url(&endpoint.url).ok_or(EndpointFault::UnsupportedUrl)?;
    let body = envelope(method, params);
    let response = exchange(&target, endpoint.request_timeout, &body)?;
    let (status, payload) = split_response(&response)?;
    if status != 200 {
        return Err(EndpointFault::Http { status });
    }
    let value = json::parse(&payload).map_err(|_| EndpointFault::MalformedResponse)?;
    if let Some(error) = value.member("error") {
        if !error.is_null() {
            let code = error.member("code").and_then(Json::as_integer).unwrap_or(0);
            let message = error
                .member("message")
                .and_then(Json::as_text)
                .unwrap_or("")
                .to_owned();
            return Err(EndpointFault::Rpc { code, message });
        }
    }
    value
        .member("result")
        .cloned()
        .ok_or(EndpointFault::MalformedResponse)
}

fn envelope(method: &str, params: &[Json]) -> String {
    Json::Object(vec![
        ("jsonrpc".to_owned(), Json::Text("2.0".to_owned())),
        ("id".to_owned(), Json::Number("1".to_owned())),
        ("method".to_owned(), Json::Text(method.to_owned())),
        ("params".to_owned(), Json::Array(params.to_vec())),
    ])
    .render()
}

fn connect(target: &Target, timeout: Duration) -> Result<TcpStream, EndpointFault> {
    let addresses = (target.host.as_str(), target.port)
        .to_socket_addrs()
        .map_err(|error| EndpointFault::Connect {
            detail: error.to_string(),
        })?;
    let mut last = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = Some(error),
        }
    }
    Err(EndpointFault::Connect {
        detail: last.map_or_else(
            || "no resolved addresses".to_owned(),
            |error| error.to_string(),
        ),
    })
}

fn exchange(target: &Target, timeout: Duration, body: &str) -> Result<Vec<u8>, EndpointFault> {
    let mut stream = connect(target, timeout)?;
    let transport = |error: std::io::Error| EndpointFault::Transport {
        detail: error.to_string(),
    };
    stream.set_read_timeout(Some(timeout)).map_err(transport)?;
    stream.set_write_timeout(Some(timeout)).map_err(transport)?;
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        target.path,
        target.host,
        target.port,
        body.len(),
        body
    );
    stream.write_all(request.as_bytes()).map_err(transport)?;
    stream.flush().map_err(transport)?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(transport)?;
    Ok(response)
}

fn split_response(response: &[u8]) -> Result<(u16, String), EndpointFault> {
    let boundary = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(EndpointFault::MalformedResponse)?;
    let header_bytes = response
        .get(..boundary)
        .ok_or(EndpointFault::MalformedResponse)?;
    let raw_body = response
        .get(boundary.saturating_add(4)..)
        .ok_or(EndpointFault::MalformedResponse)?;
    let headers =
        std::str::from_utf8(header_bytes).map_err(|_| EndpointFault::MalformedResponse)?;
    let mut lines = headers.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or(EndpointFault::MalformedResponse)?;
    let mut content_length = None;
    let mut chunked = false;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().ok();
            } else if name.eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
            {
                chunked = true;
            }
        }
    }
    let body_bytes = if chunked {
        decode_chunked(raw_body)?
    } else if let Some(length) = content_length {
        raw_body
            .get(..length)
            .ok_or(EndpointFault::MalformedResponse)?
            .to_vec()
    } else {
        raw_body.to_vec()
    };
    String::from_utf8(body_bytes)
        .map_err(|_| EndpointFault::MalformedResponse)
        .map(|payload| (status, payload))
}

fn decode_chunked(raw: &[u8]) -> Result<Vec<u8>, EndpointFault> {
    let mut output = Vec::new();
    let mut rest = raw;
    loop {
        let line_end = rest
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or(EndpointFault::MalformedResponse)?;
        let size_line = rest
            .get(..line_end)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .ok_or(EndpointFault::MalformedResponse)?;
        let size_text = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| EndpointFault::MalformedResponse)?;
        rest = rest
            .get(line_end.saturating_add(2)..)
            .ok_or(EndpointFault::MalformedResponse)?;
        if size == 0 {
            return Ok(output);
        }
        let chunk = rest.get(..size).ok_or(EndpointFault::MalformedResponse)?;
        output.extend_from_slice(chunk);
        rest = rest.get(size..).ok_or(EndpointFault::MalformedResponse)?;
        rest = rest
            .strip_prefix(b"\r\n")
            .ok_or(EndpointFault::MalformedResponse)?;
    }
}
