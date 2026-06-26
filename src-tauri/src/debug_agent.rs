use serde_json::{json, Value};
use std::io::Write;

const SESSION_ID: &str = "d62394";

fn debug_log_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".cursor").join("debug-d62394.log"));
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        paths.push(
            std::path::PathBuf::from(manifest)
                .join("..")
                .join(".cursor")
                .join("debug-d62394.log"),
        );
    }
    paths
}

pub fn log(hypothesis_id: &str, location: &str, message: &str, data: Value) {
    let payload = json!({
        "sessionId": SESSION_ID,
        "hypothesisId": hypothesis_id,
        "location": location,
        "message": message,
        "data": data,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
    });
    let Ok(line) = serde_json::to_string(&payload) else {
        return;
    };
    for path in debug_log_paths() {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(file, "{line}");
        }
    }
}
