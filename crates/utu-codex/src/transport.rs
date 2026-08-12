use std::{
    collections::HashMap,
    ffi::OsString,
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::types::{
    CodexEvent, ResumeThreadOptions, RpcServerRequestId, ServerInfo, StartThreadOptions,
    ThreadListOptions, ThreadPage, ThreadRecord, TurnRecord, TurnStartOptions, parse_notification,
    parse_server_info, parse_thread_page, parse_thread_result, parse_turn_result,
};

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(750);
const DEFAULT_MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_EVENT_BYTES: usize = 512 * 1024;
const DEFAULT_MAX_STDERR_BYTES: usize = 64 * 1024;
const DEFAULT_EVENT_CAPACITY: usize = 64;
const DEFAULT_MAX_PENDING: usize = 8;
const MAX_THREAD_PAGE_SIZE: u32 = 500;
const MAX_ID_CHARS: usize = 1_024;
const MAX_PATH_CHARS: usize = 32 * 1024;
const MAX_OPTION_CHARS: usize = 1_024;

#[derive(Clone)]
pub struct ClientConfig {
    program: PathBuf,
    args: Vec<OsString>,
    request_timeout: Duration,
    initialize_timeout: Duration,
    shutdown_timeout: Duration,
    max_message_bytes: usize,
    max_event_bytes: usize,
    max_stderr_bytes: usize,
    event_capacity: usize,
    max_pending_requests: usize,
    allow_danger_full_access: bool,
    notification_policy: NotificationPolicy,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            program: PathBuf::from("codex"),
            args: vec![OsString::from("app-server"), OsString::from("--stdio")],
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            initialize_timeout: DEFAULT_INITIALIZE_TIMEOUT,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
            max_message_bytes: DEFAULT_MAX_MESSAGE_BYTES,
            max_event_bytes: DEFAULT_MAX_EVENT_BYTES,
            max_stderr_bytes: DEFAULT_MAX_STDERR_BYTES,
            event_capacity: DEFAULT_EVENT_CAPACITY,
            max_pending_requests: DEFAULT_MAX_PENDING,
            allow_danger_full_access: false,
            notification_policy: NotificationPolicy::Full,
        }
    }
}

impl ClientConfig {
    /// Override the executable and arguments. This is useful for a discovered
    /// absolute Codex path and for protocol conformance tests.
    pub fn command(
        mut self,
        program: impl Into<PathBuf>,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Self {
        self.program = program.into();
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub fn initialize_timeout(mut self, timeout: Duration) -> Self {
        self.initialize_timeout = timeout;
        self
    }

    pub fn shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    pub fn message_bounds(mut self, max_message_bytes: usize, max_event_bytes: usize) -> Self {
        self.max_message_bytes = max_message_bytes;
        self.max_event_bytes = max_event_bytes;
        self
    }

    pub fn max_stderr_bytes(mut self, max_stderr_bytes: usize) -> Self {
        self.max_stderr_bytes = max_stderr_bytes;
        self
    }

    pub fn queue_bounds(mut self, event_capacity: usize, max_pending_requests: usize) -> Self {
        self.event_capacity = event_capacity;
        self.max_pending_requests = max_pending_requests;
        self
    }

    /// Danger-full-access remains denied by default. A reviewed native policy
    /// layer must deliberately opt in before it can be requested.
    pub fn allow_danger_full_access(mut self, allowed: bool) -> Self {
        self.allow_danger_full_access = allowed;
        self
    }

    /// Metadata-only mode drops payload-bearing item, account, thread, and turn
    /// notifications at the stdout parsing boundary. Request responses remain
    /// available for explicit bounded reads.
    pub fn notification_policy(mut self, policy: NotificationPolicy) -> Self {
        self.notification_policy = policy;
        self
    }

    fn validate(&self) -> Result<(), CodexError> {
        if self.program.as_os_str().is_empty() {
            return Err(CodexError::InvalidConfig("program cannot be empty"));
        }
        if self.request_timeout.is_zero()
            || self.initialize_timeout.is_zero()
            || self.shutdown_timeout.is_zero()
        {
            return Err(CodexError::InvalidConfig("timeouts must be non-zero"));
        }
        if self.max_message_bytes < 1_024 {
            return Err(CodexError::InvalidConfig(
                "max_message_bytes must be at least 1024",
            ));
        }
        if self.max_event_bytes == 0 || self.max_event_bytes > self.max_message_bytes {
            return Err(CodexError::InvalidConfig(
                "max_event_bytes must be non-zero and no larger than max_message_bytes",
            ));
        }
        if self.max_stderr_bytes == 0 {
            return Err(CodexError::InvalidConfig(
                "max_stderr_bytes must be non-zero",
            ));
        }
        if self.event_capacity == 0 || self.event_capacity > 4_096 {
            return Err(CodexError::InvalidConfig(
                "event_capacity must be between 1 and 4096",
            ));
        }
        if self.max_pending_requests == 0 || self.max_pending_requests > 1_024 {
            return Err(CodexError::InvalidConfig(
                "max_pending_requests must be between 1 and 1024",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum CodexError {
    #[error("invalid Codex client configuration: {0}")]
    InvalidConfig(&'static str),
    #[error("invalid Codex request input: {0}")]
    InvalidInput(&'static str),
    #[error("could not spawn Codex App Server")]
    Spawn,
    #[error("Codex App Server I/O failed while {operation}")]
    Io { operation: &'static str },
    #[error("Codex App Server protocol error: {0}")]
    Protocol(&'static str),
    #[error("Codex App Server returned error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("Codex App Server request `{method}` timed out after {timeout_ms}ms")]
    Timeout {
        method: &'static str,
        timeout_ms: u64,
    },
    #[error("Codex App Server process exited")]
    ProcessExited,
    #[error("Codex App Server connection is closed")]
    Closed,
    #[error("Codex App Server request queue is full")]
    Overloaded,
    #[error("Codex App Server message exceeded the configured byte bound")]
    MessageTooLarge,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StderrStats {
    pub bytes_seen: usize,
    pub truncated: bool,
    /// App-server stderr is deliberately discarded rather than retained.
    pub bytes_retained: usize,
}

pub struct CodexClient {
    config: ClientConfig,
    shared: Arc<Shared>,
    events: Mutex<Receiver<CodexEvent>>,
    child: Mutex<Option<Child>>,
    server_info: OnceLock<ServerInfo>,
    shutdown_started: AtomicBool,
}

impl CodexClient {
    pub fn connect(config: ClientConfig) -> Result<Self, CodexError> {
        config.validate()?;

        let mut command = Command::new(&config.program);
        command
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let mut child = command.spawn().map_err(|_| CodexError::Spawn)?;
        let stdin = child.stdin.take().ok_or(CodexError::Io {
            operation: "opening stdin",
        })?;
        let stdout = child.stdout.take().ok_or(CodexError::Io {
            operation: "opening stdout",
        })?;
        let stderr = child.stderr.take().ok_or(CodexError::Io {
            operation: "opening stderr",
        })?;

        let (event_tx, event_rx) = mpsc::sync_channel(config.event_capacity);
        let (outbound_tx, outbound_rx) =
            mpsc::sync_channel(config.max_pending_requests.saturating_add(2));
        let shared = Arc::new(Shared {
            pending: Mutex::new(HashMap::new()),
            outbound: Mutex::new(Some(outbound_tx)),
            event_tx,
            disconnected: AtomicBool::new(false),
            disconnect_reason: Mutex::new(None),
            next_id: AtomicU64::new(1),
            dropped_events: AtomicU64::new(0),
            stderr_bytes_seen: AtomicUsize::new(0),
            stderr_truncated: AtomicBool::new(false),
            max_message_bytes: config.max_message_bytes,
            max_event_bytes: config.max_event_bytes,
            max_stderr_bytes: config.max_stderr_bytes,
            max_pending_requests: config.max_pending_requests,
            notification_policy: config.notification_policy,
            process_id: child.id(),
        });

        spawn_writer(stdin, outbound_rx, Arc::clone(&shared));
        spawn_stdout_reader(stdout, Arc::clone(&shared));
        spawn_stderr_reader(stderr, Arc::clone(&shared));

        let client = Self {
            config,
            shared,
            events: Mutex::new(event_rx),
            child: Mutex::new(Some(child)),
            server_info: OnceLock::new(),
            shutdown_started: AtomicBool::new(false),
        };

        match client.initialize() {
            Ok(info) => {
                let _ = client.server_info.set(info);
                Ok(client)
            }
            Err(error) => {
                let _ = client.shutdown_inner();
                Err(error)
            }
        }
    }

    pub fn connect_default() -> Result<Self, CodexError> {
        Self::connect(ClientConfig::default())
    }

    pub fn server_info(&self) -> &ServerInfo {
        self.server_info
            .get()
            .expect("server info is set before a connected client is returned")
    }

    pub fn is_closed(&self) -> bool {
        self.shared.disconnected.load(Ordering::Acquire)
    }

    pub fn dropped_event_count(&self) -> u64 {
        self.shared.dropped_events.load(Ordering::Relaxed)
    }

    /// Atomically observe and reset the loss counter. A consumer can drain,
    /// take the count, resync with thread/list or thread/read, then require the
    /// next drained interval to report zero before presenting current state.
    pub fn take_dropped_event_count(&self) -> u64 {
        self.shared.dropped_events.swap(0, Ordering::AcqRel)
    }

    pub fn stderr_stats(&self) -> StderrStats {
        StderrStats {
            bytes_seen: self.shared.stderr_bytes_seen.load(Ordering::Relaxed),
            truncated: self.shared.stderr_truncated.load(Ordering::Relaxed),
            bytes_retained: 0,
        }
    }

    pub fn try_next_event(&self) -> Result<Option<CodexEvent>, CodexError> {
        match lock(&self.events).try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(CodexError::ProcessExited),
        }
    }

    pub fn next_event_timeout(&self, timeout: Duration) -> Result<Option<CodexEvent>, CodexError> {
        match lock(&self.events).recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(CodexError::ProcessExited),
        }
    }

    pub fn list_threads(&self, options: ThreadListOptions) -> Result<ThreadPage, CodexError> {
        if options
            .limit
            .is_some_and(|limit| limit == 0 || limit > MAX_THREAD_PAGE_SIZE)
        {
            return Err(CodexError::InvalidInput(
                "thread list limit must be between 1 and 500",
            ));
        }
        validate_optional_text(
            options.cursor.as_deref(),
            MAX_OPTION_CHARS,
            "invalid cursor",
        )?;
        validate_optional_cwd(options.cwd.as_deref())?;
        validate_optional_text(
            options.search_term.as_deref(),
            MAX_OPTION_CHARS,
            "invalid search term",
        )?;
        for source in &options.source_kinds {
            if !matches!(
                source.as_str(),
                "cli"
                    | "vscode"
                    | "exec"
                    | "appServer"
                    | "subAgent"
                    | "subAgentReview"
                    | "subAgentCompact"
                    | "subAgentThreadSpawn"
                    | "subAgentOther"
                    | "unknown"
            ) {
                return Err(CodexError::InvalidInput("unknown thread source kind"));
            }
        }

        let mut params = Map::new();
        insert_option(&mut params, "cursor", options.cursor)?;
        insert_option(&mut params, "limit", options.limit)?;
        insert_option(&mut params, "archived", options.archived)?;
        insert_option(&mut params, "cwd", options.cwd)?;
        insert_option(&mut params, "searchTerm", options.search_term)?;
        if !options.source_kinds.is_empty() {
            params.insert("sourceKinds".into(), json!(options.source_kinds));
        }
        let result = self.request(
            "thread/list",
            Value::Object(params),
            self.config.request_timeout,
        )?;
        parse_thread_page(&result).map_err(CodexError::Protocol)
    }

    /// Read persisted history without loading or subscribing to the thread.
    pub fn read_thread(
        &self,
        thread_id: &str,
        include_turns: bool,
    ) -> Result<ThreadRecord, CodexError> {
        validate_id(thread_id, "invalid thread id")?;
        let result = self.request(
            "thread/read",
            json!({"threadId": thread_id, "includeTurns": include_turns}),
            self.config.request_timeout,
        )?;
        parse_thread_result(&result).map_err(CodexError::Protocol)
    }

    pub fn resume_thread(
        &self,
        thread_id: &str,
        options: ResumeThreadOptions,
    ) -> Result<ThreadRecord, CodexError> {
        validate_id(thread_id, "invalid thread id")?;
        validate_optional_cwd(options.cwd.as_deref())?;
        validate_optional_text(options.model.as_deref(), MAX_OPTION_CHARS, "invalid model")?;
        self.validate_sandbox_mode(options.sandbox)?;
        let mut params = Map::new();
        params.insert("threadId".into(), json!(thread_id));
        insert_option(&mut params, "cwd", options.cwd)?;
        insert_option(&mut params, "model", options.model)?;
        insert_option(&mut params, "sandbox", options.sandbox)?;
        insert_option(&mut params, "approvalPolicy", options.approval_policy)?;
        let result = self.request(
            "thread/resume",
            Value::Object(params),
            self.config.request_timeout,
        )?;
        parse_thread_result(&result).map_err(CodexError::Protocol)
    }

    pub fn start_thread(&self, options: StartThreadOptions) -> Result<ThreadRecord, CodexError> {
        validate_optional_cwd(options.cwd.as_deref())?;
        validate_optional_text(options.model.as_deref(), MAX_OPTION_CHARS, "invalid model")?;
        self.validate_sandbox_mode(options.sandbox)?;
        let mut params = Map::new();
        insert_option(&mut params, "cwd", options.cwd)?;
        insert_option(&mut params, "model", options.model)?;
        insert_option(&mut params, "ephemeral", options.ephemeral)?;
        insert_option(&mut params, "sandbox", options.sandbox)?;
        insert_option(&mut params, "approvalPolicy", options.approval_policy)?;
        let result = self.request(
            "thread/start",
            Value::Object(params),
            self.config.request_timeout,
        )?;
        parse_thread_result(&result).map_err(CodexError::Protocol)
    }

    /// Submit one owner-authored text direction. This does not grant approvals;
    /// server-initiated requests are rejected until a reviewed UI contract is
    /// wired by the desktop layer.
    pub fn start_turn(
        &self,
        thread_id: &str,
        text: &str,
        options: TurnStartOptions,
    ) -> Result<TurnRecord, CodexError> {
        validate_id(thread_id, "invalid thread id")?;
        if text.trim().is_empty() {
            return Err(CodexError::InvalidInput("turn text cannot be empty"));
        }
        if text.len() > self.config.max_message_bytes / 2 {
            return Err(CodexError::InvalidInput("turn text is too large"));
        }
        validate_optional_cwd(options.cwd.as_deref())?;
        validate_optional_text(options.model.as_deref(), MAX_OPTION_CHARS, "invalid model")?;
        validate_optional_text(
            options.reasoning_effort.as_deref(),
            MAX_OPTION_CHARS,
            "invalid reasoning effort",
        )?;
        validate_optional_text(
            options.client_user_message_id.as_deref(),
            MAX_ID_CHARS,
            "invalid client message id",
        )?;
        validate_turn_sandbox(options.sandbox_policy.as_ref(), options.cwd.as_deref())?;
        if matches!(
            options.sandbox_policy,
            Some(crate::types::TurnSandboxPolicy::DangerFullAccess)
        ) && !self.config.allow_danger_full_access
        {
            return Err(CodexError::InvalidInput(
                "danger full access is disabled by client policy",
            ));
        }

        let mut params = Map::new();
        params.insert("threadId".into(), json!(thread_id));
        params.insert("input".into(), json!([{"type": "text", "text": text}]));
        insert_option(&mut params, "cwd", options.cwd)?;
        insert_option(&mut params, "model", options.model)?;
        insert_option(&mut params, "effort", options.reasoning_effort)?;
        insert_option(&mut params, "sandboxPolicy", options.sandbox_policy)?;
        insert_option(&mut params, "approvalPolicy", options.approval_policy)?;
        insert_option(
            &mut params,
            "clientUserMessageId",
            options.client_user_message_id,
        )?;
        let result = self.request(
            "turn/start",
            Value::Object(params),
            self.config.request_timeout,
        )?;
        parse_turn_result(&result).map_err(CodexError::Protocol)
    }

    pub fn shutdown(&self) -> Result<(), CodexError> {
        self.shutdown_inner()
    }

    fn initialize(&self) -> Result<ServerInfo, CodexError> {
        let result = self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "utu",
                    "title": "Utu",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
            self.config.initialize_timeout,
        )?;
        let info = parse_server_info(&result).map_err(CodexError::Protocol)?;
        self.shared
            .enqueue(json!({"method": "initialized", "params": {}}))?;
        Ok(info)
    }

    fn validate_sandbox_mode(
        &self,
        mode: Option<crate::types::SandboxMode>,
    ) -> Result<(), CodexError> {
        if matches!(mode, Some(crate::types::SandboxMode::DangerFullAccess))
            && !self.config.allow_danger_full_access
        {
            return Err(CodexError::InvalidInput(
                "danger full access is disabled by client policy",
            ));
        }
        Ok(())
    }

    fn request(
        &self,
        method: &'static str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, CodexError> {
        if self.shared.disconnected.load(Ordering::Acquire) {
            return Err(self.shared.disconnect_error());
        }
        let id = self.shared.allocate_id()?;
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        {
            let mut pending = lock(&self.shared.pending);
            if pending.len() >= self.shared.max_pending_requests {
                return Err(CodexError::Overloaded);
            }
            pending.insert(id, reply_tx);
        }

        if let Err(error) = self
            .shared
            .enqueue(json!({"method": method, "id": id, "params": params}))
        {
            lock(&self.shared.pending).remove(&id);
            return Err(error);
        }

        match reply_rx.recv_timeout(timeout) {
            Ok(PendingReply::Result(value)) => Ok(value),
            Ok(PendingReply::RpcError { code, message }) => Err(CodexError::Rpc { code, message }),
            Ok(PendingReply::Disconnected(reason)) => Err(reason.into_error()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                lock(&self.shared.pending).remove(&id);
                Err(CodexError::Timeout {
                    method,
                    timeout_ms: duration_ms(timeout),
                })
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(CodexError::ProcessExited),
        }
    }

    fn shutdown_inner(&self) -> Result<(), CodexError> {
        if self.shutdown_started.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.shared.disconnect(DisconnectReason::Closed, false);
        lock(&self.shared.outbound).take();

        let mut child_guard = lock(&self.child);
        let Some(child) = child_guard.as_mut() else {
            return Ok(());
        };
        let deadline = Instant::now() + self.config.shutdown_timeout;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    terminate_process_group(child, TerminationSignal::Term);
                    child_guard.take();
                    return Ok(());
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Ok(None) => break,
                Err(_) => {
                    return Err(CodexError::Io {
                        operation: "waiting",
                    });
                }
            }
        }

        force_terminate(child)?;
        child.wait().map_err(|_| CodexError::Io {
            operation: "reaping",
        })?;
        child_guard.take();
        Ok(())
    }
}

impl Drop for CodexClient {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

struct Shared {
    pending: Mutex<HashMap<u64, SyncSender<PendingReply>>>,
    outbound: Mutex<Option<SyncSender<Vec<u8>>>>,
    event_tx: SyncSender<CodexEvent>,
    disconnected: AtomicBool,
    disconnect_reason: Mutex<Option<DisconnectReason>>,
    next_id: AtomicU64,
    dropped_events: AtomicU64,
    stderr_bytes_seen: AtomicUsize,
    stderr_truncated: AtomicBool,
    max_message_bytes: usize,
    max_event_bytes: usize,
    max_stderr_bytes: usize,
    max_pending_requests: usize,
    notification_policy: NotificationPolicy,
    process_id: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NotificationPolicy {
    #[default]
    Full,
    MetadataOnly,
}

impl Shared {
    fn allocate_id(&self) -> Result<u64, CodexError> {
        self.next_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| {
                (id < u64::MAX).then_some(id + 1)
            })
            .map_err(|_| CodexError::Protocol("request id space exhausted"))
    }

    fn enqueue(&self, value: Value) -> Result<(), CodexError> {
        if self.disconnected.load(Ordering::Acquire) {
            return Err(self.disconnect_error());
        }
        let mut message = serde_json::to_vec(&value)
            .map_err(|_| CodexError::Protocol("request serialization failed"))?;
        if message.len() > self.max_message_bytes {
            return Err(CodexError::MessageTooLarge);
        }
        message.push(b'\n');
        let outbound = lock(&self.outbound);
        let sender = outbound.as_ref().ok_or(CodexError::Closed)?;
        match sender.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(CodexError::Overloaded),
            Err(TrySendError::Disconnected(_)) => Err(CodexError::Closed),
        }
    }

    fn emit(&self, event: CodexEvent) {
        match self.event_tx.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.dropped_events.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn disconnect(&self, reason: DisconnectReason, emit_exit: bool) {
        if self.disconnected.swap(true, Ordering::AcqRel) {
            return;
        }
        *lock(&self.disconnect_reason) = Some(reason.clone());
        if matches!(
            reason,
            DisconnectReason::Protocol(_) | DisconnectReason::Io(_)
        ) {
            terminate_process_group_id(self.process_id, TerminationSignal::Term);
        }
        let pending = std::mem::take(&mut *lock(&self.pending));
        for sender in pending.into_values() {
            let _ = sender.try_send(PendingReply::Disconnected(reason.clone()));
        }
        if emit_exit {
            self.emit(CodexEvent::ProcessExited);
        }
    }

    fn disconnect_error(&self) -> CodexError {
        lock(&self.disconnect_reason)
            .clone()
            .unwrap_or(DisconnectReason::Closed)
            .into_error()
    }
}

#[derive(Clone)]
enum DisconnectReason {
    ProcessExited,
    Protocol(&'static str),
    Io(&'static str),
    Closed,
}

impl DisconnectReason {
    fn into_error(self) -> CodexError {
        match self {
            Self::ProcessExited => CodexError::ProcessExited,
            Self::Protocol(message) => CodexError::Protocol(message),
            Self::Io(operation) => CodexError::Io { operation },
            Self::Closed => CodexError::Closed,
        }
    }
}

enum PendingReply {
    Result(Value),
    RpcError { code: i64, message: String },
    Disconnected(DisconnectReason),
}

fn spawn_writer(
    stdin: impl Write + Send + 'static,
    receiver: Receiver<Vec<u8>>,
    shared: Arc<Shared>,
) {
    thread::spawn(move || {
        let mut writer = io::BufWriter::new(stdin);
        while let Ok(message) = receiver.recv() {
            if writer
                .write_all(&message)
                .and_then(|()| writer.flush())
                .is_err()
            {
                shared.disconnect(DisconnectReason::Io("writing stdin"), true);
                return;
            }
        }
    });
}

fn spawn_stdout_reader(stdout: impl Read + Send + 'static, shared: Arc<Shared>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let line = match read_bounded_line(&mut reader, shared.max_message_bytes) {
                Ok(Some(line)) => line,
                Ok(None) => {
                    shared.disconnect(DisconnectReason::ProcessExited, true);
                    return;
                }
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                    shared.disconnect(
                        DisconnectReason::Protocol("stdout message exceeded configured bound"),
                        true,
                    );
                    return;
                }
                Err(_) => {
                    shared.disconnect(DisconnectReason::Io("reading stdout"), true);
                    return;
                }
            };
            if line.is_empty() {
                continue;
            }
            let value: Value = match serde_json::from_slice(&line) {
                Ok(value) => value,
                Err(_) => {
                    shared.disconnect(
                        DisconnectReason::Protocol("stdout contained malformed JSON"),
                        true,
                    );
                    return;
                }
            };
            if !dispatch_message(&shared, value, line.len()) {
                return;
            }
        }
    });
}

fn spawn_stderr_reader(stderr: impl Read + Send + 'static, shared: Arc<Shared>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => return,
                Ok(read) => {
                    let prior = shared
                        .stderr_bytes_seen
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                            Some(current.saturating_add(read))
                        })
                        .unwrap_or(usize::MAX);
                    if prior.saturating_add(read) > shared.max_stderr_bytes {
                        shared.stderr_truncated.store(true, Ordering::Relaxed);
                    }
                }
                Err(_) => return,
            }
        }
    });
}

fn dispatch_message(shared: &Shared, value: Value, encoded_len: usize) -> bool {
    let Some(object) = value.as_object() else {
        shared.disconnect(
            DisconnectReason::Protocol("stdout JSON message was not an object"),
            true,
        );
        return false;
    };
    let method = object.get("method").and_then(Value::as_str);
    let id = object.get("id");
    let has_result = object.contains_key("result");
    let has_error = object.contains_key("error");

    if id.is_some() && (has_result || has_error) && method.is_none() {
        dispatch_response(shared, object);
        return true;
    }
    if let (Some(method), Some(id)) = (method, id) {
        dispatch_server_request(shared, method, id);
        return !shared.disconnected.load(Ordering::Acquire);
    }
    if let Some(method) = method {
        if encoded_len > shared.max_event_bytes {
            shared.dropped_events.fetch_add(1, Ordering::Relaxed);
            return true;
        }
        let safe_method = sanitize_method(method);
        if shared.notification_policy == NotificationPolicy::MetadataOnly
            && is_payload_notification(&safe_method)
        {
            return true;
        }
        let params = object.get("params").unwrap_or(&Value::Null);
        shared.emit(parse_notification(&safe_method, params));
        return true;
    }

    shared.disconnect(
        DisconnectReason::Protocol("stdout JSON message had no recognizable shape"),
        true,
    );
    false
}

fn is_payload_notification(method: &str) -> bool {
    matches!(method, "thread/started" | "turn/started" | "turn/completed")
        || method.starts_with("item/")
        || method.starts_with("rawResponse/")
        || method.starts_with("commandExecution/")
        || method.starts_with("process/")
        || method.starts_with("reasoning/")
        || method.starts_with("plan/")
        || method.starts_with("hook/")
        || method.starts_with("account/")
        || method.starts_with("config/")
        || method.starts_with("mcpServer/")
        || method.starts_with("mcpToolCall/")
        || method.starts_with("workspace/")
        || method.starts_with("app/")
        || method.starts_with("plugin/")
        || method.starts_with("skills/")
        || method.starts_with("environment/")
        || method.starts_with("error")
        || method.starts_with("deprecation")
        || method.starts_with("guardian")
        || method.starts_with("terminal")
        || method.starts_with("fs/")
        || method.starts_with("fuzzyFileSearch/")
}

fn dispatch_response(shared: &Shared, object: &Map<String, Value>) {
    let Some(id) = object.get("id").and_then(Value::as_u64) else {
        shared.emit(CodexEvent::ProtocolWarning {
            code: "unexpected_response_id".into(),
        });
        return;
    };
    let has_result = object.contains_key("result");
    let has_error = object.contains_key("error");
    if has_result == has_error {
        shared.disconnect(
            DisconnectReason::Protocol("response must contain exactly one of result or error"),
            true,
        );
        return;
    }
    let Some(sender) = lock(&shared.pending).remove(&id) else {
        shared.emit(CodexEvent::ProtocolWarning {
            code: "late_or_unknown_response".into(),
        });
        return;
    };
    let reply = if has_result {
        PendingReply::Result(object.get("result").cloned().unwrap_or(Value::Null))
    } else if object.contains_key("error") && !object.contains_key("result") {
        let error = object.get("error").and_then(Value::as_object);
        PendingReply::RpcError {
            code: error
                .and_then(|error| error.get("code"))
                .and_then(Value::as_i64)
                .unwrap_or(-32_000),
            message: sanitize_error_message(
                error
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("request failed"),
            ),
        }
    } else {
        unreachable!("response shape was validated before dispatch")
    };
    let _ = sender.try_send(reply);
}

fn dispatch_server_request(shared: &Shared, method: &str, id: &Value) {
    let request_id = if let Some(number) = id.as_u64() {
        Some(RpcServerRequestId::Number(number))
    } else {
        id.as_str()
            .map(|value| RpcServerRequestId::String(value.chars().take(MAX_ID_CHARS).collect()))
    };
    let Some(request_id) = request_id else {
        shared.emit(CodexEvent::ProtocolWarning {
            code: "invalid_server_request_id".into(),
        });
        return;
    };
    let safe_method = sanitize_method(method);
    let response_id = match &request_id {
        RpcServerRequestId::Number(number) => json!(number),
        RpcServerRequestId::String(string) => json!(string),
    };
    if shared
        .enqueue(json!({
            "id": response_id,
            "error": {
                "code": -32601,
                "message": "Utu does not implement server-initiated requests"
            }
        }))
        .is_err()
    {
        shared.disconnect(DisconnectReason::Io("rejecting server request"), true);
        return;
    }
    shared.emit(CodexEvent::ServerRequestRejected {
        id: request_id,
        method: safe_method,
    });
}

fn read_bounded_line<R: BufRead>(reader: &mut R, max_bytes: usize) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |position| position);
        if line.len().saturating_add(take) > max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bounded line exceeded",
            ));
        }
        line.extend_from_slice(&available[..take]);
        let consumed = take + usize::from(newline.is_some());
        reader.consume(consumed);
        if newline.is_some() {
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return Ok(Some(line));
        }
    }
}

fn validate_id(value: &str, message: &'static str) -> Result<(), CodexError> {
    if value.is_empty() || value.len() > MAX_ID_CHARS || value.chars().any(char::is_control) {
        return Err(CodexError::InvalidInput(message));
    }
    Ok(())
}

fn validate_optional_text(
    value: Option<&str>,
    max_chars: usize,
    message: &'static str,
) -> Result<(), CodexError> {
    if let Some(value) = value
        && (value.is_empty() || value.len() > max_chars || value.chars().any(char::is_control))
    {
        return Err(CodexError::InvalidInput(message));
    }
    Ok(())
}

fn validate_optional_cwd(value: Option<&str>) -> Result<(), CodexError> {
    validate_optional_text(value, MAX_PATH_CHARS, "invalid cwd")?;
    if value.is_some_and(|value| !Path::new(value).is_absolute()) {
        return Err(CodexError::InvalidInput("cwd must be absolute"));
    }
    Ok(())
}

fn validate_turn_sandbox(
    policy: Option<&crate::types::TurnSandboxPolicy>,
    cwd: Option<&str>,
) -> Result<(), CodexError> {
    let Some(crate::types::TurnSandboxPolicy::WorkspaceWrite { writable_roots, .. }) = policy
    else {
        return Ok(());
    };
    if writable_roots.len() > 64 {
        return Err(CodexError::InvalidInput("too many writable roots"));
    }
    let cwd = cwd.map(Path::new);
    for root in writable_roots {
        validate_optional_cwd(Some(root))?;
        let root = Path::new(root);
        if root
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(CodexError::InvalidInput(
                "writable roots cannot contain parent traversal",
            ));
        }
        if root.parent().is_none() {
            return Err(CodexError::InvalidInput(
                "filesystem root cannot be a writable root",
            ));
        }
        if let Some(cwd) = cwd {
            let canonical_cwd = cwd
                .canonicalize()
                .map_err(|_| CodexError::InvalidInput("cwd must exist"))?;
            let canonical_root = root
                .canonicalize()
                .map_err(|_| CodexError::InvalidInput("writable roots must exist"))?;
            if !canonical_root.starts_with(&canonical_cwd) {
                return Err(CodexError::InvalidInput(
                    "writable roots must stay under the requested cwd",
                ));
            }
        } else {
            return Err(CodexError::InvalidInput(
                "workspace-write requires an explicit cwd",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum TerminationSignal {
    Term,
    Kill,
}

#[cfg(unix)]
fn terminate_process_group(child: &Child, signal: TerminationSignal) {
    terminate_process_group_id(child.id(), signal);
}

#[cfg(unix)]
fn terminate_process_group_id(process_id: u32, signal: TerminationSignal) {
    let Ok(process_id) = i32::try_from(process_id) else {
        return;
    };
    let Some(pid) = rustix::process::Pid::from_raw(process_id) else {
        return;
    };
    let signal = match signal {
        TerminationSignal::Term => rustix::process::Signal::TERM,
        TerminationSignal::Kill => rustix::process::Signal::KILL,
    };
    let _ = rustix::process::kill_process_group(pid, signal);
}

#[cfg(not(unix))]
fn terminate_process_group(_child: &Child, _signal: TerminationSignal) {}

#[cfg(not(unix))]
fn terminate_process_group_id(_process_id: u32, _signal: TerminationSignal) {}

#[cfg(unix)]
fn force_terminate(child: &mut Child) -> Result<(), CodexError> {
    terminate_process_group(child, TerminationSignal::Kill);
    // The process-group signal is authoritative; this direct fallback covers
    // an unlikely setup race before the new group exists.
    let _ = child.kill();
    Ok(())
}

#[cfg(not(unix))]
fn force_terminate(child: &mut Child) -> Result<(), CodexError> {
    child.kill().map_err(|_| CodexError::Io {
        operation: "terminating",
    })
}

fn insert_option<T: Serialize>(
    params: &mut Map<String, Value>,
    key: &'static str,
    value: Option<T>,
) -> Result<(), CodexError> {
    if let Some(value) = value {
        params.insert(
            key.into(),
            serde_json::to_value(value)
                .map_err(|_| CodexError::Protocol("request field serialization failed"))?,
        );
    }
    Ok(())
}

fn sanitize_method(method: &str) -> String {
    if method.len() > 256
        || method
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || "/_.-".contains(character)))
    {
        return "invalid/method".into();
    }
    method.to_owned()
}

fn sanitize_error_message(message: &str) -> String {
    let _ = message;
    "[redacted error detail]".into()
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn bounded_reader_accepts_crlf_and_eof_tail() {
        let mut reader = BufReader::new(Cursor::new(b"one\r\ntwo"));
        assert_eq!(
            read_bounded_line(&mut reader, 8).unwrap(),
            Some(b"one".to_vec())
        );
        assert_eq!(
            read_bounded_line(&mut reader, 8).unwrap(),
            Some(b"two".to_vec())
        );
        assert_eq!(read_bounded_line(&mut reader, 8).unwrap(), None);
    }

    #[test]
    fn bounded_reader_rejects_oversized_unterminated_lines() {
        let mut reader = BufReader::new(Cursor::new(vec![b'x'; 65]));
        let error = read_bounded_line(&mut reader, 64).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn error_sanitizer_never_returns_likely_credentials_or_identity() {
        for private in [
            "Authorization: Bearer secret.value",
            "access_token=secret",
            "owner@example.com is unauthorized",
        ] {
            assert_eq!(sanitize_error_message(private), "[redacted error detail]");
        }
    }
}
