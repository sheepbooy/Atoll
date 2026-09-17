use super::*;
use crate::core_tests::test_app_state;

#[test]
fn codex_transcript_reader_keeps_last_messages_without_full_history() {
    let dir = std::env::temp_dir().join(format!(
        "atoll-codex-transcript-window-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let transcript_path = dir.join("session.jsonl");
    let mut content = String::from(
        r#"{"type":"session_meta","payload":{"id":"session-app","cwd":"/tmp/project"}}"#,
    );
    content.push('\n');
    for i in 0..75 {
        content.push_str(&format!(
                r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"message {i}"}}]}}}}"#
            ));
        content.push('\n');
    }
    std::fs::write(&transcript_path, content).expect("write transcript");

    let state = test_app_state();
    let messages =
        read_transcript_messages_cached(&state, &transcript_path).expect("read messages");

    assert_eq!(messages.len(), TRANSCRIPT_MAX_MESSAGES);
    assert_eq!(messages[0].content, "message 25");
    assert_eq!(messages[TRANSCRIPT_MAX_MESSAGES - 1].content, "message 74");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn transcript_cache_reads_appends_and_recovers_from_truncation() {
    use std::io::Write;

    let dir = std::env::temp_dir().join(format!("atoll-transcript-cache-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("session.jsonl");
    std::fs::write(
        &path,
        "{\"type\":\"assistant\",\"message\":{\"content\":\"first\"}}\n",
    )
    .expect("initial transcript");
    let state = test_app_state();
    let first = read_transcript_messages_cached(&state, &path).expect("first read");
    assert_eq!(first.len(), 1);

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append transcript");
    writeln!(
        file,
        "{{\"type\":\"assistant\",\"message\":{{\"content\":\"second\"}}}}"
    )
    .expect("append line");
    let appended = read_transcript_messages_cached(&state, &path).expect("append read");
    assert_eq!(appended.len(), 2);
    assert_eq!(appended[1].content, "second");

    std::fs::write(
        &path,
        "{\"type\":\"assistant\",\"message\":{\"content\":\"reset\"}}\n",
    )
    .expect("truncate transcript");
    let reset = read_transcript_messages_cached(&state, &path).expect("reset read");
    assert_eq!(reset.len(), 1);
    assert_eq!(reset[0].content, "reset");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn transcript_cache_bounds_initial_read_for_large_file() {
    use std::io::{Seek, SeekFrom, Write};

    let dir = std::env::temp_dir().join(format!(
        "atoll-large-transcript-cache-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("large.jsonl");
    let mut file = std::fs::File::create(&path).expect("large transcript");
    file.set_len(100 * 1024 * 1024).expect("sparse transcript");
    file.seek(SeekFrom::End(0)).expect("seek end");
    writeln!(
        file,
        "\n{{\"type\":\"assistant\",\"message\":{{\"content\":\"tail\"}}}}"
    )
    .expect("tail message");
    drop(file);

    let state = test_app_state();
    let messages = read_transcript_messages_cached(&state, &path).expect("bounded read");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "tail");
    let cached = state.transcript_cache.lock().expect("cache");
    let entry = cached.entries.get(&path).expect("cache entry");
    assert_eq!(entry.read_offset, std::fs::metadata(&path).unwrap().len());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn transcript_path_validation_only_allows_known_transcripts() {
    let state = test_app_state();
    let dir = std::env::temp_dir().join(format!(
        "atoll-transcript-validation-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let transcript_path = dir.join("session.jsonl");
    let unknown_path = dir.join("unknown.jsonl");
    let note_path = dir.join("note.txt");
    std::fs::write(&transcript_path, "{}\n").expect("write transcript");
    std::fs::write(&unknown_path, "{}\n").expect("write unknown");
    std::fs::write(&note_path, "not a transcript").expect("write txt");

    {
        let mut known = state.known_sessions.lock().expect("lock");
        known.insert(
            "session-1".into(),
            KnownSession {
                agent: AgentKind::Claude,
                cwd: "/tmp/project".into(),
                transcript_path: Some(transcript_path.to_string_lossy().into_owned()),
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::Unknown,
                conversation_id: None,
            },
        );
    }

    assert_eq!(
        validate_trusted_transcript_path(&state, &transcript_path.to_string_lossy(),)
            .expect("valid known transcript"),
        dunce::canonicalize(&transcript_path).expect("canonical transcript"),
    );
    assert!(validate_trusted_transcript_path(&state, &unknown_path.to_string_lossy(),).is_err());
    assert!(validate_trusted_transcript_path(&state, &note_path.to_string_lossy(),).is_err());
    assert!(validate_trusted_transcript_path(
        &state,
        &dir.join("nested")
            .join("..")
            .join("session.jsonl")
            .to_string_lossy(),
    )
    .is_err());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn resolve_session_transcript_path_recovers_from_stale_path() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let projects = home.join(".cursor").join("projects");
    let Ok(entries) = std::fs::read_dir(&projects) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let transcripts = entry.path().join("agent-transcripts");
        let Ok(conv_entries) = std::fs::read_dir(&transcripts) else {
            continue;
        };
        for conv in conv_entries.flatten() {
            if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let conv_id = conv.file_name().to_string_lossy().into_owned();
            let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
            if !jsonl.is_file() {
                continue;
            }

            // Simulate a session whose stored transcript_path is broken, e.g.
            // a Windows path Cursor reported with a URI prefix or GBK mojibake.
            let state = test_app_state();
            register_known_session(
                &state,
                &conv_id,
                AgentKind::Cursor,
                ".",
                Some("/atoll-nonexistent/broken-transcript.jsonl"),
            );

            // A stale on-disk path must not short-circuit resolution: the
            // resolver should fall back to disk discovery via the full UUID.
            let resolved = resolve_session_transcript_path(&state, &conv_id, &[]);
            assert_eq!(resolved.as_deref(), Some(jsonl.to_string_lossy().as_ref()));
            return;
        }
    }
}
