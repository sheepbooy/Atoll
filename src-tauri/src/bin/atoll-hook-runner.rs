//! Spawns a Node hook script without creating a visible console on Windows.
//! Claude/Codex/Cursor invoke: atoll-hook-runner.exe "<node.exe>" "<hook.mjs>"
//!
//! This executable is built with the `windows` GUI subsystem so it never flashes a
//! console, then spawns `node <script>` with `CREATE_NO_WINDOW`. Stdio is forwarded
//! explicitly via pipes and threads rather than `Stdio::inherit()`: some hook hosts
//! (notably Cursor) spawn this runner with pipe handles that are not reliably
//! inheritable by a grandchild, so `Stdio::inherit()` would let node's stdout/stdin
//! vanish silently — the host would see "no output" and node would never receive the
//! hook payload. By owning the pipes here and copying bytes both ways, the runner
//! guarantees the payload reaches node and node's response reaches the host
//! regardless of handle-inheritance quirks.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("atoll-hook-runner is only supported on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    use std::env;
    use std::io::{Read, Write};
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::thread;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut args = env::args().skip(1);
    let node_path = args.next().unwrap_or_default();
    let script_path = args.next().unwrap_or_default();

    if node_path.is_empty() || script_path.is_empty() {
        std::process::exit(1);
    }

    let mut child = match Command::new(&node_path)
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(child) => child,
        Err(_) => std::process::exit(1),
    };

    let child_stdin = child.stdin.take();
    let child_stdout = child.stdout.take();
    let child_stderr = child.stderr.take();

    // Forward the host's stdin (the hook payload) into node.
    let stdin_thread = thread::spawn(move || {
        let mut reader = std::io::stdin();
        let mut writer = match child_stdin {
            Some(w) => w,
            None => return,
        };
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if writer.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = writer.flush();
        // Dropping `writer` closes node's stdin so `process.stdin` sees EOF.
    });

    // Forward node's stdout/stderr into the host's pipes.
    let stdout_thread = thread::spawn(move || {
        let mut writer = std::io::stdout();
        let mut reader = match child_stdout {
            Some(r) => r,
            None => return,
        };
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = writer.write_all(&buf[..n]);
                    let _ = writer.flush();
                }
                Err(_) => break,
            }
        }
    });

    let stderr_thread = thread::spawn(move || {
        let mut writer = std::io::stderr();
        let mut reader = match child_stderr {
            Some(r) => r,
            None => return,
        };
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = writer.write_all(&buf[..n]);
                    let _ = writer.flush();
                }
                Err(_) => break,
            }
        }
    });

    let code = match child.wait() {
        Ok(status) => status.code().unwrap_or(1),
        Err(_) => 1,
    };

    let _ = stdin_thread.join();
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    std::process::exit(code);
}
