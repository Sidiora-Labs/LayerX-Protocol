//! Bounded HTTP/1.1 framing for the hosted registry service.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

use crate::routes::{Request, Response};

const MAX_REQUEST: usize = 32 * 1024 * 1024;
const READ_CHUNK: usize = 8 * 1024;

/// Reads one bounded request from an accepted connection.
///
/// # Errors
///
/// Refuses oversized, truncated and malformed requests.
pub fn parse_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; READ_CHUNK];
    let header_end = loop {
        let count = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if count == 0 || bytes.len().saturating_add(count) > MAX_REQUEST {
            return Err("invalid request size".to_owned());
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position.saturating_add(4);
        }
    };
    let source =
        std::str::from_utf8(&bytes[..header_end]).map_err(|_| "invalid headers".to_owned())?;
    let mut lines = source.split("\r\n");
    let mut start = lines
        .next()
        .ok_or_else(|| "missing request line".to_owned())?
        .split_whitespace();
    let method = start
        .next()
        .ok_or_else(|| "missing method".to_owned())?
        .to_owned();
    let path = start
        .next()
        .ok_or_else(|| "missing path".to_owned())?
        .split('?')
        .next()
        .unwrap_or_default()
        .to_owned();
    let mut headers = HashMap::new();
    let mut content_length = 0_usize;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "invalid header".to_owned())?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        if name == "content-length" {
            content_length = value
                .parse()
                .map_err(|_| "invalid content length".to_owned())?;
        }
        headers.insert(name, value);
    }
    let body_end = header_end
        .checked_add(content_length)
        .ok_or_else(|| "invalid request size".to_owned())?;
    if body_end > MAX_REQUEST {
        return Err("invalid request size".to_owned());
    }
    while bytes.len() < body_end {
        let count = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("truncated body".to_owned());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(Request {
        method,
        path,
        headers,
        body: bytes[header_end..body_end].to_vec(),
    })
}

/// Writes one response and closes the connection.
///
/// # Errors
///
/// Returns the transport error that prevented the response from being sent.
pub fn write_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Content Too Large",
        422 => "Unprocessable Content",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",
        response.status,
        response.body.len(),
        response.body
    )
}
