//! Spawns a Node hook script without creating a visible console on Windows.
//! Claude/Codex invoke: atoll-hook-runner.exe "<node.exe>" "<hook.mjs>"

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("atoll-hook-runner is only supported on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    use std::env;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut args = env::args().skip(1);
    let node_path = args.next().unwrap_or_default();
    let script_path = args.next().unwrap_or_default();

    if node_path.is_empty() || script_path.is_empty() {
        std::process::exit(1);
    }

    let status = Command::new(&node_path)
        .arg(&script_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .creation_flags(CREATE_NO_WINDOW)
        .status();

    let code = match status {
        Ok(status) => status.code().unwrap_or(1),
        Err(_) => 1,
    };
    std::process::exit(code);
}
