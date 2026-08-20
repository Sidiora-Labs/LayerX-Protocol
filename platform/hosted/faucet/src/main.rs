use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_REQUEST: usize = 16 * 1024;

struct Config {
    listen: String,
    upstream: String,
    state: PathBuf,
    identity_limit: u64,
    address_limit: u64,
    ip_limit: u64,
    window_seconds: u64,
    amount: u64,
}

#[derive(Clone)]
struct Claim {
    window: u64,
    identity: String,
    address: String,
    ip: String,
    amount: u64,
}

struct Ledger {
    path: PathBuf,
    claims: Vec<Claim>,
}

struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

struct Response {
    status: u16,
    body: String,
    retry_after: Option<u64>,
}

fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_secs())
}

fn json_string(body: &[u8], field: &str) -> Option<String> {
    let source = std::str::from_utf8(body).ok()?;
    let marker = format!("\"{field}\"");
    let after = source.get(source.find(&marker)? + marker.len()..)?;
    let value = after
        .get(after.find(':')? + 1..)?
        .trim_start()
        .strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

fn valid_did(value: &str) -> bool {
    value.starts_with("did:") && value.len() <= 512 && !value.contains(['\0', '\n', '\r'])
}
fn valid_hex32(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl Ledger {
    fn open(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut claims = Vec::new();
        if path.exists() {
            let source = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            for (line_number, line) in source.lines().enumerate() {
                let fields: Vec<_> = line.split('\t').collect();
                if fields.len() != 5 {
                    return Err(format!("corrupt faucet ledger at line {}", line_number + 1));
                }
                claims.push(Claim {
                    window: fields[0].parse().map_err(|_| {
                        format!("corrupt faucet ledger at line {}", line_number + 1)
                    })?,
                    identity: fields[1].to_string(),
                    address: fields[2].to_string(),
                    ip: fields[3].to_string(),
                    amount: fields[4].parse().map_err(|_| {
                        format!("corrupt faucet ledger at line {}", line_number + 1)
                    })?,
                });
            }
        }
        Ok(Self { path, claims })
    }

    fn allowance(
        &self,
        window: u64,
        identity: &str,
        address: &str,
        ip: &str,
        config: &Config,
    ) -> Result<(), &'static str> {
        let mut identity_total = 0_u64;
        let mut address_total = 0_u64;
        let mut ip_total = 0_u64;
        for claim in self.claims.iter().filter(|claim| claim.window == window) {
            if claim.identity == identity {
                identity_total = identity_total.saturating_add(claim.amount);
            }
            if claim.address == address {
                address_total = address_total.saturating_add(claim.amount);
            }
            if claim.ip == ip {
                ip_total = ip_total.saturating_add(claim.amount);
            }
        }
        if identity_total.saturating_add(config.amount) > config.identity_limit {
            return Err("identity_quota");
        }
        if address_total.saturating_add(config.amount) > config.address_limit {
            return Err("address_quota");
        }
        if ip_total.saturating_add(config.amount) > config.ip_limit {
            return Err("network_quota");
        }
        Ok(())
    }

    fn record(&mut self, claim: Claim) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| error.to_string())?;
        writeln!(
            file,
            "{}\t{}\t{}\t{}\t{}",
            claim.window, claim.identity, claim.address, claim.ip, claim.amount
        )
        .map_err(|error| error.to_string())?;
        file.sync_data().map_err(|error| error.to_string())?;
        self.claims.push(claim);
        Ok(())
    }
}

fn parse_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 2048];
    let header_end = loop {
        let count = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if count == 0 || bytes.len() + count > MAX_REQUEST {
            return Err("invalid request size".into());
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let source = std::str::from_utf8(&bytes[..header_end]).map_err(|_| "invalid headers")?;
    let mut lines = source.split("\r\n");
    let mut first = lines.next().ok_or("missing request")?.split_whitespace();
    let method = first.next().ok_or("missing method")?.to_string();
    let path = first
        .next()
        .ok_or("missing path")?
        .split('?')
        .next()
        .unwrap_or("")
        .to_string();
    let mut headers = HashMap::new();
    let mut content_length = 0_usize;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or("invalid header")?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        if name == "content-length" {
            content_length = value.parse().map_err(|_| "invalid content length")?;
        }
        headers.insert(name, value);
    }
    if header_end + content_length > MAX_REQUEST {
        return Err("invalid request size".into());
    }
    while bytes.len() < header_end + content_length {
        let count = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("truncated body".into());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(Request {
        method,
        path,
        headers,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

fn upstream_prefund(config: &Config, did: &str, public_key: &str) -> Result<(), String> {
    let body = format!(
        "{{\"did\":\"{did}\",\"public_key\":\"{public_key}\",\"amount_hi\":0,\"amount_lo\":{}}}",
        config.amount
    );
    let mut stream = TcpStream::connect(&config.upstream).map_err(|error| error.to_string())?;
    write!(stream, "POST /__emulator/accounts/prefund HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", config.upstream, body.len(), body).map_err(|error| error.to_string())?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| error.to_string())?;
    let text = String::from_utf8_lossy(&response);
    if !text.starts_with("HTTP/1.1 200 ") || !text.contains("\"prefunded\":true") {
        return Err("testnet refused funding".into());
    }
    Ok(())
}

fn route(request: &Request, ledger: &mut Ledger, config: &Config) -> Response {
    if request.method == "GET" && request.path == "/healthz" {
        return Response {
            status: 200,
            body: "{\"status\":\"ready\",\"service\":\"faucet\"}".into(),
            retry_after: None,
        };
    }
    if request.method != "POST" || request.path != "/v1/faucet/claims" {
        return Response {
            status: 404,
            body: "{\"error\":{\"code\":\"not_found\",\"retry\":\"never\"}}".into(),
            retry_after: None,
        };
    }
    let Some(principal) = request.headers.get("x-layerx-principal") else {
        return refusal(401, "identity_required", None);
    };
    let Some(ip) = request.headers.get("x-layerx-client-ip") else {
        return refusal(401, "identity_required", None);
    };
    if principal.len() > 512 || ip.len() > 64 {
        return refusal(400, "invalid_argument", None);
    }
    let Some(did) = json_string(&request.body, "did") else {
        return refusal(400, "invalid_argument", None);
    };
    let Some(public_key) = json_string(&request.body, "public_key") else {
        return refusal(400, "invalid_argument", None);
    };
    if !valid_did(&did) || !valid_hex32(&public_key) {
        return refusal(400, "invalid_argument", None);
    }
    let window = now() / config.window_seconds;
    let identity_hash = hash(principal);
    let address_hash = hash(&public_key);
    let ip_hash = hash(ip);
    if let Err(code) = ledger.allowance(window, &identity_hash, &address_hash, &ip_hash, config) {
        let retry = config.window_seconds - now() % config.window_seconds;
        return refusal(429, code, Some(retry));
    }
    let claim = Claim {
        window,
        identity: identity_hash,
        address: address_hash,
        ip: ip_hash,
        amount: config.amount,
    };
    if ledger.record(claim).is_err() {
        return refusal(503, "persistence_unavailable", Some(10));
    }
    if upstream_prefund(config, &did, &public_key).is_err() {
        return refusal(503, "testnet_unavailable", Some(10));
    }
    Response {
        status: 200,
        body: format!(
            "{{\"funded\":true,\"amount\":\"{}\",\"network\":\"layerx-testnet\"}}",
            config.amount
        ),
        retry_after: None,
    }
}

fn refusal(status: u16, code: &str, retry_after: Option<u64>) -> Response {
    Response {
        status,
        body: format!(
            "{{\"error\":{{\"code\":\"{code}\",\"retry\":\"{}\"{}}}}}",
            if retry_after.is_some() {
                "after"
            } else {
                "never"
            },
            retry_after.map_or(String::new(), |seconds| format!(
                ",\"retry_after_seconds\":{seconds}"
            ))
        ),
        retry_after,
    }
}

fn write_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Not Found",
    };
    let retry = response.retry_after.map_or(String::new(), |seconds| {
        format!("Retry-After: {seconds}\r\n")
    });
    write!(stream, "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{}Connection: close\r\n\r\n{}", response.status, reason, response.body.len(), retry, response.body)
}

fn parse_u64(name: &str, default: u64) -> Result<u64, String> {
    env::var(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|_| format!("{name} must be an integer"))
    })
}
fn config() -> Result<Config, String> {
    let window_seconds = parse_u64("LAYERX_FAUCET_WINDOW_SECONDS", 86_400)?;
    if window_seconds == 0 {
        return Err("window must be positive".into());
    }
    Ok(Config {
        listen: env::var("LAYERX_FAUCET_LISTEN").unwrap_or_else(|_| "0.0.0.0:9410".into()),
        upstream: env::var("LAYERX_TESTNET_ADMIN_ADDRESS")
            .map_err(|_| "LAYERX_TESTNET_ADMIN_ADDRESS is required")?,
        state: PathBuf::from(
            env::var("LAYERX_FAUCET_STATE")
                .unwrap_or_else(|_| "/var/lib/layerx-faucet/claims.tsv".into()),
        ),
        identity_limit: parse_u64("LAYERX_FAUCET_IDENTITY_LIMIT", 10_000_000)?,
        address_limit: parse_u64("LAYERX_FAUCET_ADDRESS_LIMIT", 10_000_000)?,
        ip_limit: parse_u64("LAYERX_FAUCET_IP_LIMIT", 50_000_000)?,
        window_seconds,
        amount: parse_u64("LAYERX_FAUCET_CLAIM_AMOUNT", 1_000_000)?,
    })
}

fn platform_faucet(config: &Config) -> Result<(), String> {
    let mut ledger = Ledger::open(config.state.clone())?;
    let listener = TcpListener::bind(&config.listen).map_err(|error| error.to_string())?;
    eprintln!(
        "LayerX faucet ready on {} with durable state {}",
        config.listen,
        config.state.display()
    );
    for connection in listener.incoming() {
        match connection {
            Ok(mut stream) => {
                let response = parse_request(&mut stream).map_or_else(
                    |_| refusal(400, "invalid_request", None),
                    |request| route(&request, &mut ledger, config),
                );
                let _ = write_response(&mut stream, &response);
            }
            Err(error) => eprintln!("faucet accept error: {error}"),
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = config().and_then(|config| platform_faucet(&config)) {
        eprintln!("layerx-faucet: {error}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotas_cover_identity_address_and_network() {
        let config = Config {
            listen: String::new(),
            upstream: String::new(),
            state: PathBuf::new(),
            identity_limit: 2,
            address_limit: 2,
            ip_limit: 2,
            window_seconds: 60,
            amount: 1,
        };
        let ledger = Ledger {
            path: PathBuf::new(),
            claims: vec![Claim {
                window: 7,
                identity: "i".into(),
                address: "a".into(),
                ip: "n".into(),
                amount: 2,
            }],
        };
        assert_eq!(
            ledger.allowance(7, "i", "b", "m", &config),
            Err("identity_quota")
        );
        assert_eq!(
            ledger.allowance(7, "j", "a", "m", &config),
            Err("address_quota")
        );
        assert_eq!(
            ledger.allowance(7, "j", "b", "n", &config),
            Err("network_quota")
        );
        assert!(ledger.allowance(8, "i", "a", "n", &config).is_ok());
    }
}
