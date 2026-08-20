use std::time::Duration;

use serde_json::Value;
use zeroize::Zeroizing;

pub struct Client {
    agent: ureq::Agent,
    endpoint: String,
    token: Option<Zeroizing<String>>,
}

impl Client {
    pub fn new(endpoint: &str, token: Option<Zeroizing<String>>) -> Result<Self, String> {
        let endpoint = endpoint.trim_end_matches('/');
        if !(endpoint.starts_with("http://") || endpoint.starts_with("https://")) {
            return Err("environment endpoint must use http:// or https://".into());
        }
        if let Some(authority) = endpoint.strip_prefix("http://") {
            let host = authority.split('/').next().unwrap_or_default();
            let local = host == "localhost"
                || host.starts_with("localhost:")
                || host == "127.0.0.1"
                || host.starts_with("127.0.0.1:")
                || host == "[::1]"
                || host.starts_with("[::1]:");
            if !local {
                return Err("non-loopback environments must use https://".into());
            }
        }
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .build();
        Ok(Self {
            agent: config.into(),
            endpoint: endpoint.to_owned(),
            token,
        })
    }

    pub fn get(&self, path: &str) -> Result<Value, String> {
        let url = self.url(path)?;
        let response = match &self.token {
            Some(token) => self
                .agent
                .get(&url)
                .header("Authorization", &format!("Bearer {}", token.as_str()))
                .call(),
            None => self.agent.get(&url).call(),
        }
        .map_err(|error| format!("GET {path} failed: {error}"))?;
        decode(response, "GET", path)
    }

    pub fn post(
        &self,
        path: &str,
        body: &Value,
        idempotency: Option<&str>,
    ) -> Result<Value, String> {
        let url = self.url(path)?;
        let mut request = self.agent.post(&url);
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {}", token.as_str()));
        }
        if let Some(key) = idempotency {
            request = request.header("Idempotency-Key", key);
        }
        let response = request
            .send_json(body)
            .map_err(|error| format!("POST {path} failed: {error}"))?;
        decode(response, "POST", path)
    }

    fn url(&self, path: &str) -> Result<String, String> {
        if !path.starts_with('/') || path.starts_with("//") {
            return Err("request path must be absolute and cannot contain an authority".into());
        }
        Ok(format!("{}{path}", self.endpoint))
    }
}

fn decode(
    mut response: ureq::http::Response<ureq::Body>,
    method: &str,
    path: &str,
) -> Result<Value, String> {
    let status = response.status();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| format!("{method} {path} returned an unreadable response: {error}"))?;
    let value = if body.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&body)
            .map_err(|error| format!("{method} {path} returned non-JSON data: {error}"))?
    };
    if status.is_success() {
        Ok(value)
    } else {
        Err(format!(
            "{method} {path} returned HTTP {}: {}",
            status.as_u16(),
            concise(&value)
        ))
    }
}

fn concise(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "response encoding failed".into())
}

pub fn validate_resource_id(value: &str, name: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(format!(
            "{name} contains characters unsafe for an endpoint path"
        ));
    }
    Ok(())
}

pub fn validate_idempotency_key(value: &str) -> Result<(), String> {
    if value.len() < 16
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(
            "idempotency key must be 16-128 ASCII letters, digits, dashes, or underscores".into(),
        );
    }
    Ok(())
}
