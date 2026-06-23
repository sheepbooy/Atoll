#[cfg(windows)]
fn build_hook_runner_resource() {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = manifest_dir.join("src/bin/atoll-hook-runner.rs");
    let generated_dir = manifest_dir.join("generated");
    let resource = generated_dir.join("atoll-hook-runner.exe");

    println!("cargo:rerun-if-changed={}", source.display());

    if resource.is_file() && !is_newer(&source, &resource) {
        return;
    }

    fs::create_dir_all(&generated_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create {}: {error}",
            generated_dir.display()
        );
    });

    let status = Command::new("rustc")
        .current_dir(&manifest_dir)
        .args([
            source.to_str().expect("hook runner source path"),
            "-o",
            resource.to_str().expect("hook runner resource path"),
            "--edition",
            "2021",
        ])
        .status()
        .expect("failed to invoke rustc for atoll-hook-runner");

    if !status.success() {
        panic!("rustc atoll-hook-runner failed with status {status}");
    }
}

#[cfg(windows)]
fn is_newer(left: &std::path::Path, right: &std::path::Path) -> bool {
    let (Ok(left_time), Ok(right_time)) = (
        left.metadata().and_then(|m| m.modified()),
        right.metadata().and_then(|m| m.modified()),
    ) else {
        return true;
    };
    left_time > right_time
}

fn main() {
    #[cfg(windows)]
    build_hook_runner_resource();
    #[cfg(not(windows))]
    ensure_hook_runner_resource_placeholder();

    tauri_build::build();
}

#[cfg(not(windows))]
fn ensure_hook_runner_resource_placeholder() {
    use std::fs;
    use std::path::PathBuf;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let generated_dir = manifest_dir.join("generated");
    let resource = generated_dir.join("atoll-hook-runner.exe");
    if resource.is_file() {
        return;
    }
    fs::create_dir_all(&generated_dir).ok();
    fs::write(&resource, []).ok();
}
