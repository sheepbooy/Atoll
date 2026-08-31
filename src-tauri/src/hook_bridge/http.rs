use std::io::{BufRead, Write};
use std::net::TcpStream;

use serde_json::Value;

use super::*;

pub(crate) fn strip_utf8_bom(body: &[u8]) -> &[u8] {
    body.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(body)
}

pub(crate) fn is_peer_disconnected(stream: &TcpStream) -> bool {
    let _ = stream.set_nonblocking(true);
    let mut buf = [0u8; 1];
    let disconnected = match stream.peek(&mut buf) {
        Ok(0) => true,
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(_) => true,
        Ok(_) => false,
    };
    let _ = stream.set_nonblocking(false);
    disconnected
}

pub(crate) struct HttpRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: std::collections::HashMap<String, String>,
    pub(crate) body: Vec<u8>,
}

pub(crate) fn read_limited_line<R: BufRead>(
    reader: &mut R,
    limit: usize,
) -> Result<String, String> {
    let mut bytes = Vec::new();
    let read = reader
        .take((limit + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(|error| format!("Failed to read hook request: {error}"))?;
    if read > limit {
        return Err("Atoll hook request line is too large".into());
    }
    String::from_utf8(bytes).map_err(|_| "Atoll hook request is not valid UTF-8".into())
}

pub(crate) fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut reader = BufReader::new(stream);
    let request_line = read_limited_line(&mut reader, MAX_HOOK_REQUEST_LINE_BYTES)?;

    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "Missing hook request method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "Missing hook request path".to_string())?
        .to_string();

    let mut content_length = 0usize;
    let mut headers = std::collections::HashMap::new();
    let mut header_bytes = 0usize;
    loop {
        let remaining = MAX_HOOK_HEADER_BYTES.saturating_sub(header_bytes);
        if remaining == 0 {
            return Err("Atoll hook request headers are too large".into());
        }
        let header = read_limited_line(&mut reader, remaining)?;
        header_bytes = header_bytes.saturating_add(header.len());
        let trimmed = header.trim_end();
        if trimmed.is_empty() {
            break;
        }

        if let Some((name, value)) = trimmed.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if name == "content-length" {
                content_length = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| format!("Invalid content-length: {error}"))?;
                if content_length > MAX_HOOK_BODY_BYTES {
                    return Err(format!(
                        "Atoll hook request body exceeds {} bytes",
                        MAX_HOOK_BODY_BYTES
                    ));
                }
            }
            headers.insert(name, value);
        }
    }

    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .map_err(|error| format!("Failed to read hook request body: {error}"))?;

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

pub(crate) fn write_json_response(stream: &mut TcpStream, body: Value) -> std::io::Result<()> {
    let body = serde_json::to_string(&body).unwrap_or_else(|_| "{}".into());
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

pub(crate) fn command_label(tool_name: &str, tool_input: &Value) -> String {
    if tool_name == "Bash"
        || tool_name == "Shell"
        || tool_name == "exec_command"
        || tool_name == "run_shell_command"
    {
        if let Some(command) = tool_input.get("command").and_then(Value::as_str) {
            return format!("Bash: {command}");
        }
    }

    if tool_name == "apply_patch" {
        if let Some(command) = tool_input.get("command").and_then(Value::as_str) {
            return format!("Edit: {command}");
        }
    }

    if let Some(file_path) = tool_input.get("file_path").and_then(Value::as_str) {
        return format!("{tool_name}: {file_path}");
    }

    tool_name.to_string()
}

pub(crate) fn detail_label(tool_name: &str, tool_input: &Value) -> String {
    if let Some(description) = tool_input.get("description").and_then(Value::as_str) {
        return description.to_string();
    }

    if let Some(command) = tool_input.get("command").and_then(Value::as_str) {
        return command.to_string();
    }

    if let Some(file_path) = tool_input.get("file_path").and_then(Value::as_str) {
        return format!("{tool_name} wants to access {file_path}.");
    }

    format!("{tool_name} is requesting approval.")
}
