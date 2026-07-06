#[cfg(debug_assertions)]
use serde_json::json;
use serde_json::Value;
#[cfg(debug_assertions)]
use std::io::Write;

#[cfg(debug_assertions)]
const SESSION_ID: &str = "d62394";

#[cfg(debug_assertions)]
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
    #[cfg(not(debug_assertions))]
    {
        let _ = (hypothesis_id, location, message, data);
    }

    #[cfg(debug_assertions)]
    {
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
}
