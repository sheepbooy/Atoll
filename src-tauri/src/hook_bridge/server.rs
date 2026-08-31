use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use super::*;

static ACTIVE_CONNECTIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static OBSERVER_SENDER: OnceLock<mpsc::SyncSender<ObserverJob>> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) enum ObserverKind {
    Claude,
    Codex,
    Cursor,
    Zcode,
    Gemini,
}

pub(crate) struct ObserverJob {
    pub(crate) app: AppHandle,
    pub(crate) hook_event_name: String,
    pub(crate) payload: Value,
    pub(crate) kind: ObserverKind,
}

pub(crate) struct ConnectionGuard;

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(crate) fn start_observer_worker() {
    OBSERVER_SENDER.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel::<ObserverJob>(OBSERVER_QUEUE_CAPACITY);
        thread::spawn(move || {
            while let Ok(job) = receiver.recv() {
                let event = job.hook_event_name.clone();
                let result = match job.kind {
                    ObserverKind::Claude => {
                        process_claude_observer_event(job.app, job.hook_event_name, job.payload)
                    }
                    ObserverKind::Codex => {
                        process_codex_observer_event(job.app, job.hook_event_name, job.payload)
                    }
                    ObserverKind::Cursor => {
                        process_cursor_observer_event(job.app, job.hook_event_name, job.payload)
                    }
                    ObserverKind::Zcode => {
                        process_zcode_observer_event(job.app, job.hook_event_name, job.payload)
                    }
                    ObserverKind::Gemini => {
                        process_gemini_observer_event(job.app, job.hook_event_name, job.payload)
                    }
                };
                if let Err(error) = result {
                    eprintln!("Atoll {event} observer failed: {error}");
                }
            }
        });
        sender
    });
}

pub(crate) fn enqueue_observer(job: ObserverJob) -> Result<(), String> {
    start_observer_worker();
    OBSERVER_SENDER
        .get()
        .ok_or_else(|| "Atoll observer queue unavailable".to_string())?
        .send(job)
        .map_err(|_| "Atoll observer worker stopped".to_string())
}

pub(crate) fn bridge_config_path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return dirs::data_dir().map(|dir| dir.join("Atoll").join("bridge.json"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        return dirs::data_local_dir().map(|dir| dir.join("Atoll").join("bridge.json"));
    }
}

pub(crate) fn write_bridge_config(port: u16, token: &str) -> std::io::Result<()> {
    let path = bridge_config_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "bridge config path"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let config = json!({
        "port": port,
        "claudeUrl": format!("http://{HOOK_BIND_HOST}:{port}/claude/pre-tool-use"),
        "codexUrl": format!("http://{HOOK_BIND_HOST}:{port}/codex/hook"),
        "cursorUrl": format!("http://{HOOK_BIND_HOST}:{port}/cursor/hook"),
        "zcodeUrl": format!("http://{HOOK_BIND_HOST}:{port}/zcode/hook"),
        "geminiUrl": format!("http://{HOOK_BIND_HOST}:{port}/gemini/hook"),
        "token": token,
    });
    std::fs::write(path, serde_json::to_string_pretty(&config)?)
}

pub(crate) fn cursor_hook_url(port: u16) -> String {
    format!("http://{HOOK_BIND_HOST}:{port}/cursor/hook")
}

pub(crate) fn cursor_hook_url_for_app(app: &AppHandle) -> String {
    let port = app.state::<AppState>().bridge_port.load(Ordering::SeqCst);
    if port == 0 {
        cursor_hook_url(DEFAULT_HOOK_PORT)
    } else {
        cursor_hook_url(port)
    }
}

pub(crate) fn refresh_bridge_config_file(app: &AppHandle) -> std::io::Result<()> {
    let state = app.state::<AppState>();
    let port = state.bridge_port.load(Ordering::SeqCst);
    if port == 0 {
        return Ok(());
    }
    let token = state
        .bridge_auth_token
        .lock()
        .map(|token| token.clone())
        .unwrap_or_default();
    write_bridge_config(port, &token)
}

pub(crate) fn bind_listener_on_port(port: u16) -> std::io::Result<TcpListener> {
    let addr: SocketAddr = format!("{HOOK_BIND_HOST}:{port}")
        .parse()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;
    socket.listen(128)?;
    Ok(socket.into())
}

pub(crate) fn bind_hook_listener() -> std::io::Result<(TcpListener, u16)> {
    for attempt in 0..HOOK_BIND_RETRY_COUNT {
        match bind_listener_on_port(DEFAULT_HOOK_PORT) {
            Ok(listener) => return Ok((listener, DEFAULT_HOOK_PORT)),
            Err(error) if attempt + 1 < HOOK_BIND_RETRY_COUNT => {
                eprintln!(
                    "Atoll hook bridge bind attempt {} on {DEFAULT_HOOK_PORT} failed: {error}",
                    attempt + 1
                );
                thread::sleep(HOOK_BIND_RETRY_DELAY);
            }
            Err(error) => {
                eprintln!(
                    "Atoll hook bridge failed to bind {HOOK_BIND_HOST}:{DEFAULT_HOOK_PORT} after {HOOK_BIND_RETRY_COUNT} attempts: {error}"
                );
            }
        }
    }

    for port in HOOK_FALLBACK_PORT_START..=HOOK_FALLBACK_PORT_END {
        if let Ok(listener) = bind_listener_on_port(port) {
            eprintln!(
                "Atoll hook bridge using fallback port {port} ({DEFAULT_HOOK_PORT} unavailable)"
            );
            return Ok((listener, port));
        }
    }

    for port in HOOK_SECONDARY_FALLBACK_START..=HOOK_SECONDARY_FALLBACK_END {
        if let Ok(listener) = bind_listener_on_port(port) {
            eprintln!(
                "Atoll hook bridge using secondary fallback port {port} (primary range reserved, e.g. by WSL/Hyper-V)"
            );
            return Ok((listener, port));
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AddrInUse,
        format!(
            "no available hook bridge port in {DEFAULT_HOOK_PORT}..{HOOK_FALLBACK_PORT_END} or {HOOK_SECONDARY_FALLBACK_START}..{HOOK_SECONDARY_FALLBACK_END} (WSL/Hyper-V may reserve lower ports — try `wsl --shutdown` and restart Atoll)"
        ),
    ))
}

pub(crate) fn bridge_socket_addr(port: u16) -> Option<SocketAddr> {
    format!("{HOOK_BIND_HOST}:{port}").parse().ok()
}

pub(crate) fn bridge_port_from_config_file() -> Option<u16> {
    let path = bridge_config_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&content).ok()?;
    value
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
}

pub(crate) fn refresh_listening_snapshot(app: &AppHandle) {
    let state = app.state::<AppState>();
    let online = crate::compute_listening_online(app);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(online);
    }
    let snapshot = build_snapshot(app, &state);
    let _ = app.emit("snapshot-changed", &snapshot);
}

/// True when the local hook bridge accepts TCP connections on its bind address.
pub(crate) fn is_bridge_reachable(app: &AppHandle) -> bool {
    let stored_port = app.state::<AppState>().bridge_port.load(Ordering::SeqCst);
    let mut ports = Vec::new();
    if stored_port != 0 {
        ports.push(stored_port);
    }
    if !ports.contains(&DEFAULT_HOOK_PORT) {
        ports.push(DEFAULT_HOOK_PORT);
    }
    if let Some(config_port) = bridge_port_from_config_file() {
        if !ports.contains(&config_port) {
            ports.push(config_port);
        }
    }

    ports.into_iter().any(|port| {
        bridge_socket_addr(port)
            .and_then(|addr| TcpStream::connect_timeout(&addr, BRIDGE_PROBE_TIMEOUT).ok())
            .is_some()
    })
}

pub(crate) fn mark_bridge_reachable(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(mut last) = state.last_bridge_reachable.lock() {
        *last = Some(Instant::now());
    };
}

pub(crate) fn bridge_reachable_within_grace(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let Ok(last) = state.last_bridge_reachable.lock() else {
        return false;
    };
    last.map(|instant| instant.elapsed() < BRIDGE_ONLINE_GRACE)
        .unwrap_or(false)
}

/// Like [`is_bridge_reachable`], but keeps `online` true briefly after a probe failure
/// so dev hot-reloads / bridge rebinds do not flash the logo dead.
pub(crate) fn is_bridge_online(app: &AppHandle) -> bool {
    if is_bridge_reachable(app) {
        mark_bridge_reachable(app);
        return true;
    }
    bridge_reachable_within_grace(app)
}

pub(crate) fn start_server(app: AppHandle) {
    start_observer_worker();
    thread::spawn(move || {
        let listener = match bind_hook_listener() {
            Ok((listener, port)) => {
                app.state::<AppState>()
                    .bridge_port
                    .store(port, Ordering::SeqCst);
                let token = app
                    .state::<AppState>()
                    .bridge_auth_token
                    .lock()
                    .map(|token| token.clone())
                    .unwrap_or_default();
                if let Err(error) = write_bridge_config(port, &token) {
                    eprintln!("Atoll hook bridge failed to write bridge.json: {error}");
                } else {
                    eprintln!("Atoll hook bridge listening on {HOOK_BIND_HOST}:{port}");
                    mark_bridge_reachable(&app);
                    // #region agent log
                    crate::debug_agent::log(
                        "H-C",
                        "hook_bridge.rs:start_hook_bridge",
                        "hook bridge listening",
                        json!({
                            "port": port,
                            "defaultPort": DEFAULT_HOOK_PORT,
                            "cursorUrl": cursor_hook_url(port),
                            "bridgeConfigPath": bridge_config_path()
                                .map(|p| p.to_string_lossy().into_owned()),
                        }),
                    );
                    // #endregion
                }
                refresh_listening_snapshot(&app);
                crate::sync_cursor_hook_bridge_urls(&app, port);
                listener
            }
            Err(error) => {
                eprintln!("Atoll hook bridge failed to bind any port: {error}");
                refresh_listening_snapshot(&app);
                return;
            }
        };

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    if ACTIVE_CONNECTIONS.fetch_add(1, Ordering::AcqRel) >= MAX_INFLIGHT_CONNECTIONS
                    {
                        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::AcqRel);
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                        continue;
                    }
                    let app = app.clone();
                    thread::spawn(move || {
                        let _guard = ConnectionGuard;
                        handle_connection(app, stream);
                    });
                }
                Err(error) => eprintln!("Atoll hook bridge connection failed: {error}"),
            }
        }
    });
}

pub(crate) fn handle_connection(app: AppHandle, mut stream: TcpStream) {
    touch_hook_activity(&app.state::<AppState>());
    let _ = stream.set_read_timeout(Some(HOOK_REQUEST_READ_TIMEOUT));
    let result = read_http_request(&mut stream)
        .and_then(|request| route_request(app, request, &stream))
        .unwrap_or_else(|error| fallback_hook_response("PreToolUse", &error));
    let _ = stream.set_read_timeout(None);
    let _ = stream.set_write_timeout(Some(HOOK_RESPONSE_WRITE_TIMEOUT));

    let _ = write_json_response(&mut stream, result);
}

pub(crate) fn route_request(
    app: AppHandle,
    request: HttpRequest,
    stream: &TcpStream,
) -> Result<Value, String> {
    if let Some(response) = crate::capture::route_http(&app, &request.path) {
        return Ok(response);
    }

    if request.method != "POST" {
        return Err("Unsupported Atoll hook endpoint".into());
    }

    match request.path.as_str() {
        "/claude/pre-tool-use" => {
            require_hook_auth(&app, &request)?;
            route_claude_request(app, request, stream)
        }
        "/codex/hook" => {
            require_hook_auth(&app, &request)?;
            route_codex_request(app, request, stream)
        }
        "/zcode/hook" => {
            require_hook_auth(&app, &request)?;
            route_zcode_request(app, request, stream)
        }
        "/gemini/hook" => {
            require_hook_auth(&app, &request)?;
            route_gemini_request(app, request, stream)
        }
        "/cursor/hook" => {
            require_hook_auth(&app, &request)?;
            route_cursor_request(app, request, stream)
        }
        _ => Err("Unsupported Atoll hook endpoint".into()),
    }
}

pub(crate) fn require_hook_auth(app: &AppHandle, request: &HttpRequest) -> Result<(), String> {
    let state = app.state::<AppState>();
    let token = state
        .bridge_auth_token
        .lock()
        .map(|token| token.clone())
        .map_err(|_| "Atoll hook bridge auth unavailable".to_string())?;
    if hook_auth_matches(&token, request) {
        return Ok(());
    }
    Err("Unauthorized Atoll hook request".into())
}

pub(crate) fn hook_auth_matches(expected_token: &str, request: &HttpRequest) -> bool {
    let provided = request.headers.get(HOOK_AUTH_HEADER);
    !expected_token.is_empty() && provided.map(String::as_str) == Some(expected_token)
}

#[cfg(test)]
mod bridge_bind_tests {
    use super::*;

    #[test]
    fn request_line_reader_rejects_oversized_input() {
        let payload = vec![b'a'; MAX_HOOK_REQUEST_LINE_BYTES + 1];
        let mut reader = std::io::Cursor::new(payload);
        assert!(read_limited_line(&mut reader, MAX_HOOK_REQUEST_LINE_BYTES).is_err());
    }

    #[test]
    fn request_history_pruning_keeps_pending_requests() {
        let mut requests = Vec::new();
        for index in 0..(MAX_RESOLVED_REQUESTS_PER_SESSION + 20) {
            requests.push(PermissionRequest {
                id: format!("resolved-{index}"),
                tool_use_id: None,
                agent: AgentKind::Codex,
                session: "session-a".into(),
                command: "Bash: true".into(),
                detail: String::new(),
                cwd: ".".into(),
                requested_at: String::new(),
                status: PermissionStatus::Approved,
                archived: false,
                supports_always: false,
                transcript_path: None,
                tool_input: None,
            });
        }
        let mut pending = requests[0].clone();
        pending.id = "pending".into();
        pending.status = PermissionStatus::Pending;
        requests.insert(0, pending);
        prune_request_history(&mut requests);
        assert!(requests.iter().any(|request| request.id == "pending"));
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.status != PermissionStatus::Pending)
                .count(),
            MAX_RESOLVED_REQUESTS_PER_SESSION
        );
    }

    #[test]
    fn fallback_port_range_starts_after_default() {
        assert_eq!(HOOK_FALLBACK_PORT_START, DEFAULT_HOOK_PORT + 1);
        assert!(HOOK_FALLBACK_PORT_END >= HOOK_FALLBACK_PORT_START);
    }

    #[test]
    fn write_bridge_config_json_shape() {
        let temp = std::env::temp_dir().join(format!("atoll-bridge-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).expect("temp dir");
        let config_path = temp.join("bridge.json");

        let port = 47_778_u16;
        let token = "test-token";
        let config = json!({
            "port": port,
            "claudeUrl": format!("http://{HOOK_BIND_HOST}:{port}/claude/pre-tool-use"),
            "codexUrl": format!("http://{HOOK_BIND_HOST}:{port}/codex/hook"),
            "cursorUrl": format!("http://{HOOK_BIND_HOST}:{port}/cursor/hook"),
            "zcodeUrl": format!("http://{HOOK_BIND_HOST}:{port}/zcode/hook"),
            "geminiUrl": format!("http://{HOOK_BIND_HOST}:{port}/gemini/hook"),
            "token": token,
        });
        std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .expect("write config");

        let parsed: Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            parsed.get("port").and_then(Value::as_u64),
            Some(port as u64)
        );
        assert!(parsed
            .get("claudeUrl")
            .and_then(Value::as_str)
            .unwrap()
            .contains("/claude/pre-tool-use"));
        assert!(parsed
            .get("codexUrl")
            .and_then(Value::as_str)
            .unwrap()
            .contains("/codex/hook"));
        assert!(parsed
            .get("cursorUrl")
            .and_then(Value::as_str)
            .unwrap()
            .contains("/cursor/hook"));
        assert_eq!(parsed.get("token").and_then(Value::as_str), Some(token));

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn hook_auth_requires_matching_token_header() {
        let mut headers = std::collections::HashMap::new();
        headers.insert(HOOK_AUTH_HEADER.to_string(), "secret-token".to_string());
        let request = HttpRequest {
            method: "POST".into(),
            path: "/codex/hook".into(),
            headers,
            body: Vec::new(),
        };

        assert!(hook_auth_matches("secret-token", &request));
        assert!(!hook_auth_matches("other-token", &request));
        assert!(!hook_auth_matches("", &request));

        let request_without_header = HttpRequest {
            method: "POST".into(),
            path: "/codex/hook".into(),
            headers: std::collections::HashMap::new(),
            body: Vec::new(),
        };
        assert!(!hook_auth_matches("secret-token", &request_without_header));
    }
}
