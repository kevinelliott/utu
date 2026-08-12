#![cfg(unix)]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use utu_codex::{
    ClientConfig, CodexClient, CodexError, CodexEvent, NotificationPolicy, ResumeThreadOptions,
    StartThreadOptions, ThreadListOptions, TurnSandboxPolicy, TurnStartOptions,
};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

fn fake_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-app-server"))
}

fn config(scenario: &str) -> ClientConfig {
    config_with_args([scenario])
}

fn config_with_args<'a>(args: impl IntoIterator<Item = &'a str>) -> ClientConfig {
    ClientConfig::default()
        .command(fake_binary(), args)
        .initialize_timeout(Duration::from_secs(5))
        .request_timeout(Duration::from_millis(250))
        .shutdown_timeout(Duration::from_millis(500))
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "utu-codex-test-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn supports_session_list_read_resume_start_and_text_turn() {
    let client = CodexClient::connect(config("vertical")).unwrap();
    assert_eq!(client.server_info().platform_os, "test");
    let page = client
        .list_threads(ThreadListOptions {
            limit: Some(20),
            ..ThreadListOptions::default()
        })
        .unwrap();
    assert_eq!(page.data[0].id, "thr_list");
    assert_eq!(client.read_thread("thr_list", true).unwrap().turns.len(), 1);
    assert_eq!(
        client
            .resume_thread("thr_list", ResumeThreadOptions::default())
            .unwrap()
            .summary
            .id,
        "thr_list"
    );
    assert_eq!(
        client
            .start_thread(StartThreadOptions::default())
            .unwrap()
            .summary
            .id,
        "thr_new"
    );
    assert_eq!(
        client
            .start_turn("thr_new", "owner direction", TurnStartOptions::default())
            .unwrap()
            .id,
        "turn_new"
    );
}

#[test]
fn notifications_can_arrive_before_correlated_response() {
    let client = CodexClient::connect(config("interleaved")).unwrap();
    assert!(
        client
            .list_threads(ThreadListOptions::default())
            .unwrap()
            .data
            .is_empty()
    );
    assert!(matches!(
        client.next_event_timeout(Duration::from_secs(1)).unwrap(),
        Some(CodexEvent::AgentMessageDelta { delta, .. }) if delta == "hello"
    ));
}

#[test]
fn metadata_only_policy_drops_message_payload_at_parse_boundary() {
    let client = CodexClient::connect(
        config("interleaved").notification_policy(NotificationPolicy::MetadataOnly),
    )
    .unwrap();
    client.list_threads(ThreadListOptions::default()).unwrap();
    assert!(
        !matches!(
            client.try_next_event().unwrap(),
            Some(CodexEvent::AgentMessageDelta { .. })
        ),
        "metadata-only mode retained a message delta"
    );
    assert_eq!(client.dropped_event_count(), 0);
}

#[test]
fn metadata_only_policy_drops_lifecycle_and_account_payloads_before_projection() {
    let client = CodexClient::connect(
        config("metadata-secrets").notification_policy(NotificationPolicy::MetadataOnly),
    )
    .unwrap();
    client.list_threads(ThreadListOptions::default()).unwrap();
    let mut serialized = String::new();
    while let Some(event) = client.try_next_event().unwrap() {
        serialized.push_str(&serde_json::to_string(&event).unwrap());
    }
    for private in [
        "private-name",
        "private-preview",
        "private-turn-payload",
        "private-account-token",
    ] {
        assert!(
            !serialized.contains(private),
            "metadata queue leaked {private}"
        );
    }
    assert!(serialized.is_empty(), "metadata mode queued payload events");
}

#[test]
fn request_errors_are_correlated_and_provider_messages_are_always_redacted() {
    let client = CodexClient::connect(config("rpc-error")).unwrap();
    assert_eq!(
        client
            .list_threads(ThreadListOptions::default())
            .unwrap_err(),
        CodexError::Rpc {
            code: 77,
            message: "[redacted error detail]".into()
        }
    );
}

#[test]
fn malformed_json_fails_closed_without_returning_payload() {
    let client = CodexClient::connect(config("malformed")).unwrap();
    assert_eq!(
        client
            .list_threads(ThreadListOptions::default())
            .unwrap_err(),
        CodexError::Protocol("stdout contained malformed JSON")
    );
}

#[test]
fn fatal_protocol_error_terminates_the_dedicated_process_group() {
    let temp = TempDirectory::new();
    let pid_file = temp.0.join("descendant.pid");
    let client = CodexClient::connect(config_with_args([
        "malformed-descendant",
        pid_file.to_str().unwrap(),
    ]))
    .unwrap();
    let descendant = wait_for_pid(&pid_file);
    assert_eq!(
        client
            .list_threads(ThreadListOptions::default())
            .unwrap_err(),
        CodexError::Protocol("stdout contained malformed JSON")
    );
    let deadline = Instant::now() + Duration::from_secs(1);
    while process_exists(descendant) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !process_exists(descendant),
        "descendant {descendant} survived"
    );
}

#[test]
fn invalid_correlated_response_closes_and_terminates_the_process_group() {
    let temp = TempDirectory::new();
    let pid_file = temp.0.join("descendant.pid");
    let client = CodexClient::connect(config_with_args([
        "invalid-response-descendant",
        pid_file.to_str().unwrap(),
    ]))
    .unwrap();
    let descendant = wait_for_pid(&pid_file);
    assert_eq!(
        client
            .list_threads(ThreadListOptions::default())
            .unwrap_err(),
        CodexError::Protocol("response must contain exactly one of result or error")
    );
    assert!(client.is_closed());
    let deadline = Instant::now() + Duration::from_secs(1);
    while process_exists(descendant) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !process_exists(descendant),
        "descendant {descendant} survived"
    );
}

#[test]
fn request_timeout_is_bounded_and_does_not_block_shutdown() {
    let client =
        CodexClient::connect(config("timeout").request_timeout(Duration::from_millis(80))).unwrap();
    let started = Instant::now();
    assert!(matches!(
        client.list_threads(ThreadListOptions::default()),
        Err(CodexError::Timeout {
            method: "thread/list",
            ..
        })
    ));
    assert!(started.elapsed() < Duration::from_secs(1));
    client.shutdown().unwrap();
}

#[test]
fn process_exit_wakes_waiters_with_process_exit_evidence() {
    let client = CodexClient::connect(config("exit")).unwrap();
    assert!(matches!(
        client.next_event_timeout(Duration::from_secs(1)).unwrap(),
        Some(CodexEvent::ProcessExited)
    ));
    assert_eq!(
        client
            .list_threads(ThreadListOptions::default())
            .unwrap_err(),
        CodexError::ProcessExited
    );
}

#[test]
fn server_requests_are_rejected_without_retaining_private_params() {
    let client = CodexClient::connect(config("server-request")).unwrap();
    let event = client
        .next_event_timeout(Duration::from_secs(1))
        .unwrap()
        .unwrap();
    let serialized = serde_json::to_string(&event).unwrap();
    assert!(matches!(event, CodexEvent::ServerRequestRejected { .. }));
    assert!(!serialized.contains("private-token"));
    assert!(!serialized.contains("dangerous"));
    client.list_threads(ThreadListOptions::default()).unwrap();
}

#[test]
fn oversized_stdout_message_fails_closed_at_configured_bound() {
    let client = CodexClient::connect(config("oversized").message_bounds(1024, 512)).unwrap();
    assert_eq!(
        client
            .list_threads(ThreadListOptions::default())
            .unwrap_err(),
        CodexError::Protocol("stdout message exceeded configured bound")
    );
}

#[test]
fn stderr_is_drained_counted_and_never_retained() {
    let client = CodexClient::connect(config("stderr").max_stderr_bytes(1024)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    while client.stderr_stats().bytes_seen <= 1024 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let stats = client.stderr_stats();
    assert!(stats.bytes_seen > 1024);
    assert!(stats.truncated);
    assert_eq!(stats.bytes_retained, 0);
}

#[test]
fn graceful_shutdown_closes_stdin_and_allows_server_cleanup() {
    let temp = TempDirectory::new();
    let marker = temp.0.join("stopped");
    let client =
        CodexClient::connect(config_with_args(["graceful", marker.to_str().unwrap()])).unwrap();
    client.shutdown().unwrap();
    assert_eq!(fs::read_to_string(marker).unwrap(), "stopped");
}

#[test]
fn shutdown_terminates_descendants_in_the_dedicated_process_group() {
    let temp = TempDirectory::new();
    let pid_file = temp.0.join("descendant.pid");
    let client =
        CodexClient::connect(config_with_args(["descendant", pid_file.to_str().unwrap()])).unwrap();
    let descendant = wait_for_pid(&pid_file);
    client.shutdown().unwrap();

    let deadline = Instant::now() + Duration::from_secs(1);
    while process_exists(descendant) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !process_exists(descendant),
        "descendant {descendant} survived"
    );
}

#[test]
fn lexical_or_symlink_writable_root_escape_is_rejected_before_send() {
    let temp = TempDirectory::new();
    let project = temp.0.join("project");
    fs::create_dir(&project).unwrap();
    let client = CodexClient::connect(config("timeout")).unwrap();
    let traversal = format!("{}/..", project.display());
    let error = client
        .start_turn(
            "thr_1",
            "hello",
            TurnStartOptions {
                cwd: Some(project.to_string_lossy().into_owned()),
                sandbox_policy: Some(TurnSandboxPolicy::WorkspaceWrite {
                    writable_roots: vec![traversal],
                    network_access: false,
                    exclude_slash_tmp: true,
                    exclude_tmpdir_env_var: true,
                }),
                ..TurnStartOptions::default()
            },
        )
        .unwrap_err();
    assert_eq!(
        error,
        CodexError::InvalidInput("writable roots cannot contain parent traversal")
    );

    let outside = temp.0.join("outside");
    fs::create_dir(&outside).unwrap();
    let symlink = project.join("escape");
    std::os::unix::fs::symlink(&outside, &symlink).unwrap();
    let error = client
        .start_turn(
            "thr_1",
            "hello",
            TurnStartOptions {
                cwd: Some(project.to_string_lossy().into_owned()),
                sandbox_policy: Some(TurnSandboxPolicy::WorkspaceWrite {
                    writable_roots: vec![symlink.to_string_lossy().into_owned()],
                    network_access: false,
                    exclude_slash_tmp: true,
                    exclude_tmpdir_env_var: true,
                }),
                ..TurnStartOptions::default()
            },
        )
        .unwrap_err();
    assert_eq!(
        error,
        CodexError::InvalidInput("writable roots must stay under the requested cwd")
    );
}

#[test]
fn danger_full_access_is_denied_for_thread_and_turn_by_default() {
    use utu_codex::{SandboxMode, StartThreadOptions};

    let client = CodexClient::connect(config("timeout")).unwrap();
    assert_eq!(
        client
            .start_thread(StartThreadOptions {
                sandbox: Some(SandboxMode::DangerFullAccess),
                ..StartThreadOptions::default()
            })
            .unwrap_err(),
        CodexError::InvalidInput("danger full access is disabled by client policy")
    );
    assert_eq!(
        client
            .resume_thread(
                "thr_1",
                ResumeThreadOptions {
                    sandbox: Some(SandboxMode::DangerFullAccess),
                    ..ResumeThreadOptions::default()
                },
            )
            .unwrap_err(),
        CodexError::InvalidInput("danger full access is disabled by client policy")
    );
    assert_eq!(
        client
            .start_turn(
                "thr_1",
                "hello",
                TurnStartOptions {
                    sandbox_policy: Some(TurnSandboxPolicy::DangerFullAccess),
                    ..TurnStartOptions::default()
                },
            )
            .unwrap_err(),
        CodexError::InvalidInput("danger full access is disabled by client policy")
    );
}

fn wait_for_pid(path: &Path) -> i32 {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if let Ok(value) = fs::read_to_string(path) {
            return value.parse().unwrap();
        }
        assert!(Instant::now() < deadline, "fake server did not write pid");
        thread::sleep(Duration::from_millis(10));
    }
}

fn process_exists(raw_pid: i32) -> bool {
    rustix::process::Pid::from_raw(raw_pid)
        .is_some_and(|pid| rustix::process::test_kill_process(pid).is_ok())
}
