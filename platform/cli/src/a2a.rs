use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use layerx_mcp::server::{DeploymentMode, ToolDefinition};
use serde_json::{json, Map, Value};

use crate::config::Configuration;
use crate::encoding::hex_encode;
use crate::install::a2a::agent_card;
use crate::toolset;

const MAX_REQUEST_BYTES: usize = 1_048_576;
const MAX_TRACKED_TASKS: usize = 256;
const CARD_ROUTES: [&str; 2] = ["/.well-known/agent-card.json", "/.well-known/agent.json"];

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

struct Response {
    status: u16,
    body: Vec<u8>,
}

struct Instruction {
    skill: String,
    arguments: Value,
}

pub fn endpoint(listen: &str) -> Result<SocketAddr, String> {
    let address = listen
        .parse::<SocketAddr>()
        .map_err(|_| format!("{listen} is not a host:port endpoint"))?;
    if !address.ip().is_loopback() {
        return Err("the agent-to-agent endpoint must bind a loopback address".into());
    }
    if address.port() == 0 {
        return Err("the agent-to-agent endpoint must declare a fixed port".into());
    }
    Ok(address)
}

pub fn serve(
    configuration: &Configuration,
    subject: Option<&str>,
    listen: &str,
    mode: DeploymentMode,
) -> Result<(), String> {
    let address = endpoint(listen)?;
    let tools = toolset::surface(mode)?;
    let card = agent_card(
        &configuration.current_environment,
        &address.to_string(),
        mode,
        &tools,
    );
    let listener =
        TcpListener::bind(address).map_err(|error| format!("could not bind {listen}: {error}"))?;
    let mut tasks: BTreeMap<String, Value> = BTreeMap::new();
    for accepted in listener.incoming() {
        let Ok(mut stream) = accepted else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        let response = match read_request(&mut stream) {
            Ok(request) => route(configuration, subject, &tools, &card, &mut tasks, &request),
            Err(error) => encode(400, &json!({"error": error})),
        };
        let _ = write_response(&mut stream, &response);
    }
    Ok(())
}

fn route(
    configuration: &Configuration,
    subject: Option<&str>,
    tools: &[ToolDefinition],
    card: &Value,
    tasks: &mut BTreeMap<String, Value>,
    request: &Request,
) -> Response {
    match request.method.as_str() {
        "GET" if CARD_ROUTES.contains(&request.path.as_str()) => encode(200, card),
        "GET" => encode(404, &json!({"error": "unknown agent-to-agent route"})),
        "POST" if request.path == "/" => encode(
            200,
            &dispatch(configuration, subject, tools, tasks, &request.body),
        ),
        "POST" => encode(404, &json!({"error": "unknown agent-to-agent route"})),
        _ => encode(405, &json!({"error": "unsupported method"})),
    }
}

fn dispatch(
    configuration: &Configuration,
    subject: Option<&str>,
    tools: &[ToolDefinition],
    tasks: &mut BTreeMap<String, Value>,
    body: &[u8],
) -> Value {
    let Ok(message) = serde_json::from_slice::<Value>(body) else {
        return failure(Value::Null, -32700, "the message is not valid JSON");
    };
    let identifier = message.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return failure(identifier, -32600, "the message did not name a method");
    };
    let parameters = message.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "message/send" => send(
            configuration,
            subject,
            tools,
            tasks,
            identifier,
            &parameters,
        ),
        "tasks/get" => fetch(tasks, identifier, &parameters),
        "tasks/cancel" => cancel(tasks, identifier, &parameters),
        "message/stream" | "tasks/resubscribe" | "tasks/pushNotificationConfig/set" => failure(
            identifier,
            -32004,
            "the operation is not supported by this deployment",
        ),
        _ => failure(identifier, -32601, "the method is not implemented"),
    }
}

fn send(
    configuration: &Configuration,
    subject: Option<&str>,
    tools: &[ToolDefinition],
    tasks: &mut BTreeMap<String, Value>,
    identifier: Value,
    parameters: &Value,
) -> Value {
    let Some(message) = parameters.get("message") else {
        return failure(identifier, -32602, "the request carried no message");
    };
    let instruction = match read_instruction(message) {
        Ok(instruction) => instruction,
        Err(error) => return failure(identifier, -32602, &error),
    };
    let Some(tool) = tools
        .iter()
        .find(|tool| tool.name == instruction.skill)
        .copied()
    else {
        return failure(
            identifier,
            -32601,
            &format!(
                "skill {} is not served by this deployment",
                instruction.skill
            ),
        );
    };
    let ids = match (new_identifier(), new_identifier()) {
        (Ok(task), Ok(artifact)) => (task, artifact),
        _ => {
            return failure(
                identifier,
                -32603,
                "operating-system randomness failed while opening a task",
            )
        }
    };
    let context = message
        .get("contextId")
        .and_then(Value::as_str)
        .map_or_else(|| ids.0.clone(), str::to_owned);
    let outcome = toolset::invoke(configuration, subject, tool, &instruction.arguments);
    let task = build_task(&ids, &context, message, tool.name, outcome);
    if tasks.len() >= MAX_TRACKED_TASKS {
        if let Some(evicted) = tasks.keys().next().cloned() {
            tasks.remove(&evicted);
        }
    }
    tasks.insert(ids.0.clone(), task.clone());
    success(identifier, task)
}

fn build_task(
    ids: &(String, String),
    context: &str,
    message: &Value,
    tool: &str,
    outcome: Result<Value, String>,
) -> Value {
    let timestamp = timestamp();
    match outcome {
        Ok(value) => json!({
            "id": ids.0,
            "contextId": context,
            "kind": "task",
            "status": {"state": "completed", "timestamp": timestamp},
            "artifacts": [{
                "artifactId": ids.1,
                "name": tool,
                "parts": [{"kind": "data", "data": {"result": value}}],
            }],
            "history": [message],
        }),
        Err(error) => json!({
            "id": ids.0,
            "contextId": context,
            "kind": "task",
            "status": {
                "state": "failed",
                "timestamp": timestamp,
                "message": {
                    "kind": "message",
                    "role": "agent",
                    "messageId": ids.1,
                    "taskId": ids.0,
                    "contextId": context,
                    "parts": [{"kind": "text", "text": error}],
                },
            },
            "history": [message],
        }),
    }
}

fn cancel(tasks: &BTreeMap<String, Value>, identifier: Value, parameters: &Value) -> Value {
    let Some(name) = parameters.get("id").and_then(Value::as_str) else {
        return failure(identifier, -32602, "the request did not name a task");
    };
    if tasks.contains_key(name) {
        failure(
            identifier,
            -32002,
            "the task already reached a terminal state",
        )
    } else {
        failure(identifier, -32001, "the task was not found")
    }
}

fn fetch(tasks: &BTreeMap<String, Value>, identifier: Value, parameters: &Value) -> Value {
    let Some(name) = parameters.get("id").and_then(Value::as_str) else {
        return failure(identifier, -32602, "the request did not name a task");
    };
    match tasks.get(name) {
        Some(task) => success(identifier, task.clone()),
        None => failure(identifier, -32001, "the task was not found"),
    }
}

fn read_instruction(message: &Value) -> Result<Instruction, String> {
    let parts = message
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| "the message carried no parts".to_string())?;
    for part in parts {
        let payload = match part.get("kind").and_then(Value::as_str) {
            Some("data") => part.get("data").cloned(),
            Some("text") => part
                .get("text")
                .and_then(Value::as_str)
                .and_then(|text| serde_json::from_str::<Value>(text).ok()),
            _ => None,
        };
        let Some(payload) = payload else {
            continue;
        };
        let Some(skill) = payload.get("skill").and_then(Value::as_str) else {
            continue;
        };
        return Ok(Instruction {
            skill: skill.to_owned(),
            arguments: payload.get("arguments").cloned().unwrap_or(Value::Null),
        });
    }
    Err("the message did not name a skill and its arguments".into())
}

fn read_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut bytes = Vec::with_capacity(4096);
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("could not read the request: {error}"))?;
        if read == 0 {
            return Err("the connection closed before the request headers".into());
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("the request exceeds the transport limit".into());
        }
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| "the request headers are not UTF-8".to_string())?;
    let mut lines = headers.split("\r\n");
    let mut start = lines
        .next()
        .ok_or_else(|| "the request has no start line".to_string())?
        .split_whitespace();
    let method = start
        .next()
        .ok_or_else(|| "the request has no method".to_string())?
        .to_owned();
    let path = start
        .next()
        .ok_or_else(|| "the request has no path".to_string())?
        .split('?')
        .next()
        .unwrap_or("/")
        .to_owned();
    let mut length = 0_usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value
                    .trim()
                    .parse()
                    .map_err(|_| "the content length is not a number".to_string())?;
            }
        }
    }
    if length > MAX_REQUEST_BYTES {
        return Err("the request body exceeds the transport limit".into());
    }
    while bytes.len() - header_end < length {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("could not read the request body: {error}"))?;
        if read == 0 {
            return Err("the connection closed before the request body".into());
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.len() > MAX_REQUEST_BYTES.saturating_add(header_end) {
            return Err("the request exceeds the transport limit".into());
        }
    }
    let body = bytes[header_end..header_end + length].to_vec();
    Ok(Request { method, path, body })
}

fn write_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        response.status,
        response.body.len()
    )?;
    stream.write_all(&response.body)
}

fn encode(status: u16, value: &Value) -> Response {
    let body = serde_json::to_vec(value)
        .unwrap_or_else(|_| b"{\"error\":\"result encoding failed\"}".to_vec());
    Response { status, body }
}

fn success(identifier: Value, result: Value) -> Value {
    let mut envelope = Map::new();
    envelope.insert("jsonrpc".to_owned(), Value::String("2.0".to_owned()));
    envelope.insert("id".to_owned(), identifier);
    envelope.insert("result".to_owned(), result);
    Value::Object(envelope)
}

fn failure(identifier: Value, code: i32, detail: &str) -> Value {
    let mut error = Map::new();
    error.insert("code".to_owned(), Value::from(code));
    error.insert("message".to_owned(), Value::String(detail.to_owned()));
    let mut envelope = Map::new();
    envelope.insert("jsonrpc".to_owned(), Value::String("2.0".to_owned()));
    envelope.insert("id".to_owned(), identifier);
    envelope.insert("error".to_owned(), Value::Object(error));
    Value::Object(envelope)
}

fn new_identifier() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("operating-system randomness failed: {error}"))?;
    Ok(hex_encode(&bytes))
}

fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let days = seconds / 86_400;
    let remainder = seconds % 86_400;
    let (year, month, day) = civil_date(days);
    let hour = remainder / 3_600;
    let minute = (remainder % 3_600) / 60;
    let second = remainder % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_date(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let civil_year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day_of_month = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 {
        civil_year + 1
    } else {
        civil_year
    };
    (year, month, day_of_month)
}
