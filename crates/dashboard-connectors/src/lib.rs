//! Provider-neutral discovery and diagnostics for locally installed agent CLIs.
//!
//! The runtime deliberately separates installation, version, and authentication
//! evidence. Finding a binary is never treated as authentication evidence, and
//! a provider without a documented non-interactive auth-status command remains
//! explicitly unsupported rather than being guessed from config files.

use std::{
    env,
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use utu_core::{AuthState, EvidenceKind, Severity};

const DEFAULT_PROBE_TIMEOUT_MS: u64 = 2_000;
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
const MAX_EVIDENCE_CHARS: usize = 4_096;
const MAX_VERSION_CHARS: usize = 160;
const PROCESS_REAP_GRACE: Duration = Duration::from_millis(100);
const READER_DRAIN_GRACE: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterImplementation {
    Available,
    Planned,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    Cli,
    JsonRpcStdio,
    AcpStdio,
    StructuredOutput,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterCapabilities {
    pub discover: bool,
    pub version_probe: bool,
    pub auth_probe: bool,
    pub sessions: bool,
    pub chat: bool,
    pub files: bool,
    pub event_stream: bool,
    pub logs: bool,
    pub costs: bool,
}

impl AdapterCapabilities {
    const fn diagnostics(version_probe: bool, auth_probe: bool) -> Self {
        Self {
            discover: true,
            version_probe,
            auth_probe,
            sessions: false,
            chat: false,
            files: false,
            event_stream: false,
            logs: false,
            costs: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub kind: TransportKind,
    pub implementation: AdapterImplementation,
    pub capabilities: AdapterCapabilities,
    pub note: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub provider_id: &'static str,
    pub executable: &'static str,
    pub executable_aliases: &'static [&'static str],
    pub current_capabilities: AdapterCapabilities,
    pub transports: &'static [TransportDescriptor],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthProbe {
    CodexLoginStatus,
    ClaudeAuthStatusJson,
    CursorStatus,
    Unsupported(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CliDefinition {
    pub id: &'static str,
    pub display_name: &'static str,
    pub executable: &'static str,
    pub executable_aliases: &'static [&'static str],
    version_args: Option<&'static [&'static str]>,
    auth_probe: AuthProbe,
    transports: &'static [TransportDescriptor],
}

const CLI_DIAGNOSTICS_WITH_AUTH: AdapterCapabilities = AdapterCapabilities::diagnostics(true, true);
const CLI_DIAGNOSTICS_NO_AUTH: AdapterCapabilities = AdapterCapabilities::diagnostics(true, false);
const DISCOVERY_ONLY: AdapterCapabilities = AdapterCapabilities::diagnostics(false, false);

const CODEX_APP_SERVER_CAPABILITIES: AdapterCapabilities = AdapterCapabilities {
    discover: true,
    version_probe: true,
    auth_probe: true,
    sessions: true,
    chat: true,
    files: true,
    event_stream: true,
    logs: true,
    costs: false,
};

const ACP_AGENT_CAPABILITIES: AdapterCapabilities = AdapterCapabilities {
    discover: true,
    version_probe: true,
    auth_probe: false,
    sessions: true,
    chat: true,
    files: true,
    event_stream: true,
    logs: true,
    costs: false,
};

const STRUCTURED_AGENT_CAPABILITIES: AdapterCapabilities = AdapterCapabilities {
    discover: true,
    version_probe: true,
    auth_probe: false,
    sessions: true,
    chat: true,
    files: true,
    event_stream: true,
    logs: true,
    costs: false,
};

const CODEX_TRANSPORTS: &[TransportDescriptor] = &[
    TransportDescriptor {
        id: "cli-diagnostics",
        display_name: "CLI diagnostics",
        kind: TransportKind::Cli,
        implementation: AdapterImplementation::Available,
        capabilities: CLI_DIAGNOSTICS_WITH_AUTH,
        note: "Utu currently performs bounded version and `codex login status` probes only.",
    },
    TransportDescriptor {
        id: "app-server-stdio",
        display_name: "Codex App Server",
        kind: TransportKind::JsonRpcStdio,
        implementation: AdapterImplementation::Available,
        capabilities: CODEX_APP_SERVER_CAPABILITIES,
        note: "Bounded local transport: initialize and thread/list are runtime-verified against Codex CLI 0.147.0; read/resume/start, text turns, events, and file changes pass fake-process conformance tests. Server requests are rejected, and costs are not inferred.",
    },
];

const CLAUDE_TRANSPORTS: &[TransportDescriptor] = &[
    TransportDescriptor {
        id: "session-files",
        display_name: "Local session files",
        kind: TransportKind::StructuredOutput,
        implementation: AdapterImplementation::Available,
        capabilities: AdapterCapabilities {
            discover: true,
            version_probe: true,
            auth_probe: true,
            sessions: true,
            chat: false,
            files: false,
            event_stream: true,
            logs: false,
            costs: false,
        },
        note: "Utu observes Claude Code session metadata from ~/.claude/projects; transcripts are not imported and orchestration is not implemented.",
    },
    TransportDescriptor {
        id: "structured-output",
        display_name: "Structured CLI output",
        kind: TransportKind::StructuredOutput,
        implementation: AdapterImplementation::Planned,
        capabilities: STRUCTURED_AGENT_CAPABILITIES,
        note: "Claude Code documents JSON and stream-JSON output; orchestration is not implemented in Utu yet.",
    },
];

const CURSOR_TRANSPORTS: &[TransportDescriptor] = &[TransportDescriptor {
    id: "structured-output",
    display_name: "Structured CLI output",
    kind: TransportKind::StructuredOutput,
    implementation: AdapterImplementation::Planned,
    capabilities: STRUCTURED_AGENT_CAPABILITIES,
    note: "Cursor Agent advertises JSON and stream-JSON output locally; orchestration is not implemented in Utu yet.",
}];

const GROK_TRANSPORTS: &[TransportDescriptor] = &[TransportDescriptor {
    id: "structured-output",
    display_name: "Structured CLI output",
    kind: TransportKind::StructuredOutput,
    implementation: AdapterImplementation::Planned,
    capabilities: STRUCTURED_AGENT_CAPABILITIES,
    note: "Grok Build advertises JSON streaming locally; orchestration is not implemented in Utu yet.",
}];

const GEMINI_TRANSPORTS: &[TransportDescriptor] = &[TransportDescriptor {
    id: "acp-stdio",
    display_name: "Agent Client Protocol",
    kind: TransportKind::AcpStdio,
    implementation: AdapterImplementation::Planned,
    capabilities: ACP_AGENT_CAPABILITIES,
    note: "Gemini CLI advertises ACP mode locally; Utu has not implemented or runtime-verified the transport yet.",
}];

const OPENCODE_TRANSPORTS: &[TransportDescriptor] = &[TransportDescriptor {
    id: "acp-stdio",
    display_name: "Agent Client Protocol",
    kind: TransportKind::AcpStdio,
    implementation: AdapterImplementation::Planned,
    capabilities: ACP_AGENT_CAPABILITIES,
    note: "OpenCode advertises an ACP server locally; Utu has not implemented or runtime-verified the transport yet.",
}];

const NO_TRANSPORTS: &[TransportDescriptor] = &[];

pub const KNOWN_LOCAL_CLIS: [CliDefinition; 8] = [
    CliDefinition {
        id: "codex",
        display_name: "Codex",
        executable: "codex",
        executable_aliases: &[],
        version_args: Some(&["--version"]),
        auth_probe: AuthProbe::CodexLoginStatus,
        transports: CODEX_TRANSPORTS,
    },
    CliDefinition {
        id: "claude",
        display_name: "Claude Code",
        executable: "claude",
        executable_aliases: &[],
        version_args: Some(&["--version"]),
        auth_probe: AuthProbe::ClaudeAuthStatusJson,
        transports: CLAUDE_TRANSPORTS,
    },
    CliDefinition {
        id: "grok",
        display_name: "Grok Build",
        executable: "grok",
        executable_aliases: &[],
        version_args: Some(&["--version"]),
        auth_probe: AuthProbe::Unsupported(
            "Login status cannot be verified from Utu without an interactive command.",
        ),
        transports: GROK_TRANSPORTS,
    },
    CliDefinition {
        id: "cursor",
        display_name: "Cursor Agent",
        executable: "cursor-agent",
        executable_aliases: &[],
        version_args: Some(&["--version"]),
        auth_probe: AuthProbe::CursorStatus,
        transports: CURSOR_TRANSPORTS,
    },
    CliDefinition {
        id: "antigravity",
        display_name: "Antigravity",
        executable: "antigravity",
        executable_aliases: &[],
        version_args: None,
        auth_probe: AuthProbe::Unsupported(
            "A stable Antigravity CLI diagnostic and authentication contract has not been verified.",
        ),
        transports: NO_TRANSPORTS,
    },
    CliDefinition {
        id: "gemini",
        display_name: "Gemini CLI",
        executable: "gemini",
        executable_aliases: &[],
        version_args: Some(&["--version"]),
        auth_probe: AuthProbe::Unsupported(
            "Gemini CLI does not expose a documented non-interactive global authentication-status command.",
        ),
        transports: GEMINI_TRANSPORTS,
    },
    CliDefinition {
        id: "aider",
        display_name: "Aider",
        executable: "aider",
        executable_aliases: &[],
        version_args: Some(&["--version"]),
        auth_probe: AuthProbe::Unsupported(
            "Aider can route to multiple providers and has no single safe global authentication-status probe.",
        ),
        transports: NO_TRANSPORTS,
    },
    CliDefinition {
        id: "opencode",
        display_name: "OpenCode",
        executable: "opencode",
        executable_aliases: &[],
        version_args: Some(&["--version"]),
        auth_probe: AuthProbe::Unsupported(
            "OpenCode credentials are provider-specific; Utu does not infer a global authenticated state from a credential listing.",
        ),
        transports: OPENCODE_TRANSPORTS,
    },
];

impl CliDefinition {
    pub const fn descriptor(self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            id: self.id,
            display_name: self.display_name,
            provider_id: self.id,
            executable: self.executable,
            executable_aliases: self.executable_aliases,
            current_capabilities: match self.auth_probe {
                AuthProbe::CodexLoginStatus
                | AuthProbe::ClaudeAuthStatusJson
                | AuthProbe::CursorStatus => CLI_DIAGNOSTICS_WITH_AUTH,
                AuthProbe::Unsupported(_) if self.version_args.is_some() => CLI_DIAGNOSTICS_NO_AUTH,
                AuthProbe::Unsupported(_) => DISCOVERY_ONLY,
            },
            transports: self.transports,
        }
    }
}

pub fn known_connector_descriptors() -> Vec<ConnectorDescriptor> {
    KNOWN_LOCAL_CLIS
        .iter()
        .copied()
        .map(CliDefinition::descriptor)
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCliProbe {
    pub id: &'static str,
    pub display_name: &'static str,
    pub executable: &'static str,
    pub installed_path: Option<PathBuf>,
    /// Binary discovery is not authentication evidence.
    pub auth_state: AuthState,
    pub install_evidence: EvidenceKind,
}

pub trait ExecutableLookup {
    fn find(&self, executable: &str) -> Option<PathBuf>;
}

/// PATH lookup with stable, first-match semantics and duplicate-directory removal.
#[derive(Clone, Debug)]
pub struct DeterministicPathLookup {
    directories: Vec<PathBuf>,
    #[cfg(windows)]
    extensions: Vec<OsString>,
}

impl DeterministicPathLookup {
    pub fn new(directories: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut unique = Vec::new();
        for directory in directories {
            if !unique.contains(&directory) {
                unique.push(directory);
            }
        }

        #[cfg(windows)]
        let extensions = env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .filter(|extension| !extension.is_empty())
                    .map(OsString::from)
                    .collect()
            })
            .unwrap_or_else(|| vec![OsString::from(".EXE")]);

        Self {
            directories: unique,
            #[cfg(windows)]
            extensions,
        }
    }

    pub fn from_path(path: Option<OsString>) -> Self {
        let directories = path
            .as_deref()
            .map(env::split_paths)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        Self::new(directories)
    }

    fn candidates(&self, executable: &str) -> Vec<PathBuf> {
        #[cfg(windows)]
        {
            let has_extension = Path::new(executable).extension().is_some();
            self.directories
                .iter()
                .flat_map(|directory| {
                    if has_extension {
                        vec![directory.join(executable)]
                    } else {
                        self.extensions
                            .iter()
                            .map(|extension| {
                                let mut name = OsString::from(executable);
                                name.push(extension);
                                directory.join(name)
                            })
                            .collect()
                    }
                })
                .collect()
        }

        #[cfg(not(windows))]
        {
            self.directories
                .iter()
                .map(|directory| directory.join(executable))
                .collect()
        }
    }
}

impl ExecutableLookup for DeterministicPathLookup {
    fn find(&self, executable: &str) -> Option<PathBuf> {
        self.candidates(executable)
            .into_iter()
            .find(|candidate| is_executable_file(candidate))
    }
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EnvironmentPath;

impl ExecutableLookup for EnvironmentPath {
    fn find(&self, executable: &str) -> Option<PathBuf> {
        DeterministicPathLookup::from_path(env::var_os("PATH")).find(executable)
    }
}

fn find_definition_executable(
    lookup: &impl ExecutableLookup,
    definition: &CliDefinition,
) -> Option<PathBuf> {
    std::iter::once(definition.executable)
        .chain(definition.executable_aliases.iter().copied())
        .find_map(|executable| lookup.find(executable))
}

/// Compatibility discovery API. Prefer [`diagnose_known_connectors`] for new callers.
pub fn probe_known_local_clis(lookup: &impl ExecutableLookup) -> Vec<LocalCliProbe> {
    KNOWN_LOCAL_CLIS
        .iter()
        .map(|definition| {
            let installed_path = find_definition_executable(lookup, definition);
            LocalCliProbe {
                id: definition.id,
                display_name: definition.display_name,
                executable: definition.executable,
                install_evidence: if installed_path.is_some() {
                    EvidenceKind::Observed
                } else {
                    EvidenceKind::Inferred
                },
                installed_path,
                auth_state: AuthState::Unknown,
            }
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandRequest {
    pub program: PathBuf,
    pub display_program: String,
    pub args: Vec<String>,
    pub timeout: Duration,
}

impl CommandRequest {
    fn new(program: PathBuf, display_program: &str, args: &[&str]) -> Self {
        Self {
            program,
            display_program: display_program.to_owned(),
            args: args.iter().map(|argument| (*argument).to_owned()).collect(),
            timeout: Duration::from_millis(DEFAULT_PROBE_TIMEOUT_MS),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandFailure {
    Spawn(String),
    Wait(String),
    TimedOut {
        stdout: String,
        stderr: String,
        duration_ms: u64,
    },
}

pub trait CommandRunner: Send + Sync {
    fn run(&self, request: &CommandRequest) -> Result<CommandOutput, CommandFailure>;
}

#[derive(Clone, Copy, Debug)]
pub struct SystemCommandRunner {
    poll_interval: Duration,
}

impl Default for SystemCommandRunner {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(10),
        }
    }
}

impl CommandRunner for SystemCommandRunner {
    fn run(&self, request: &CommandRequest) -> Result<CommandOutput, CommandFailure> {
        let started = Instant::now();
        let mut command = Command::new(&request.program);
        command
            .args(&request.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("NO_COLOR", "1")
            .env("NO_OPEN_BROWSER", "1")
            .env("TERM", "dumb");

        // Each Unix probe owns a fresh process group so a timed-out CLI cannot
        // leave descendants holding our capture pipes open. On Windows the
        // child is terminated directly; capture collection remains bounded by
        // the channel-based drain grace below even if a descendant survives.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let mut child = command
            .spawn()
            .map_err(|error| CommandFailure::Spawn(error.to_string()))?;

        let mut stdout_reader = child.stdout.take().map(spawn_reader);
        let mut stderr_reader = child.stderr.take().map(spawn_reader);

        loop {
            drain_available(&mut stdout_reader);
            drain_available(&mut stderr_reader);
            match child.try_wait() {
                Ok(Some(status)) => {
                    // A probe is not allowed to daemonize. Clean up any
                    // descendants which inherited its process group and pipes.
                    terminate_process_tree(&mut child);
                    drain_readers_until(
                        &mut stdout_reader,
                        &mut stderr_reader,
                        Instant::now() + READER_DRAIN_GRACE,
                    );
                    let stdout = captured_string(stdout_reader);
                    let stderr = captured_string(stderr_reader);
                    return Ok(CommandOutput {
                        exit_code: status.code(),
                        stdout,
                        stderr,
                        duration_ms: elapsed_millis(started),
                    });
                }
                Ok(None) if started.elapsed() >= request.timeout => {
                    terminate_process_tree(&mut child);
                    reap_child_until(&mut child, Instant::now() + PROCESS_REAP_GRACE);
                    drain_readers_until(
                        &mut stdout_reader,
                        &mut stderr_reader,
                        Instant::now() + READER_DRAIN_GRACE,
                    );
                    let stdout = captured_string(stdout_reader);
                    let stderr = captured_string(stderr_reader);
                    return Err(CommandFailure::TimedOut {
                        stdout,
                        stderr,
                        duration_ms: elapsed_millis(started),
                    });
                }
                Ok(None) => {
                    let remaining = request.timeout.saturating_sub(started.elapsed());
                    thread::sleep(self.poll_interval.min(remaining));
                }
                Err(error) => {
                    terminate_process_tree(&mut child);
                    reap_child_until(&mut child, Instant::now() + PROCESS_REAP_GRACE);
                    drain_readers_until(
                        &mut stdout_reader,
                        &mut stderr_reader,
                        Instant::now() + READER_DRAIN_GRACE,
                    );
                    return Err(CommandFailure::Wait(error.to_string()));
                }
            }
        }
    }
}

enum ReaderMessage {
    Chunk(Vec<u8>),
    Finished,
}

struct ReaderCapture {
    receiver: Receiver<ReaderMessage>,
    captured: Vec<u8>,
    finished: bool,
}

fn spawn_reader<R>(mut reader: R) -> ReaderCapture
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut captured_bytes = 0_usize;
        let mut buffer = [0_u8; 4_096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let captured = read.min(MAX_CAPTURE_BYTES.saturating_sub(captured_bytes));
                    if captured > 0 {
                        captured_bytes += captured;
                        if sender
                            .send(ReaderMessage::Chunk(buffer[..captured].to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            }
        }
        let _ = sender.send(ReaderMessage::Finished);
    });
    ReaderCapture {
        receiver,
        captured: Vec::new(),
        finished: false,
    }
}

fn drain_available(reader: &mut Option<ReaderCapture>) {
    let Some(reader) = reader else {
        return;
    };
    loop {
        match reader.receiver.try_recv() {
            Ok(ReaderMessage::Chunk(chunk)) => reader.captured.extend_from_slice(&chunk),
            Ok(ReaderMessage::Finished) | Err(TryRecvError::Disconnected) => {
                reader.finished = true;
                break;
            }
            Err(TryRecvError::Empty) => break,
        }
    }
}

fn drain_readers_until(
    stdout: &mut Option<ReaderCapture>,
    stderr: &mut Option<ReaderCapture>,
    deadline: Instant,
) {
    loop {
        drain_available(stdout);
        drain_available(stderr);
        let stdout_finished = stdout.as_ref().is_none_or(|reader| reader.finished);
        let stderr_finished = stderr.as_ref().is_none_or(|reader| reader.finished);
        let now = Instant::now();
        if (stdout_finished && stderr_finished) || now >= deadline {
            return;
        }
        thread::sleep(Duration::from_millis(1).min(deadline.saturating_duration_since(now)));
    }
}

fn captured_string(mut reader: Option<ReaderCapture>) -> String {
    drain_available(&mut reader);
    reader
        .map(|reader| String::from_utf8_lossy(&reader.captured).into_owned())
        .unwrap_or_default()
}

fn terminate_process_tree(child: &mut Child) {
    #[cfg(unix)]
    kill_process_group(child.id());
    let _ = child.kill();
}

#[cfg(unix)]
fn kill_process_group(process_id: u32) {
    let Ok(process_group) = i32::try_from(process_id) else {
        return;
    };
    const SIGKILL: i32 = 9;
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    // SAFETY: `process_group` came from a live child ID and the negative PID
    // form of POSIX `kill` targets only the isolated group created above.
    let _ = unsafe { kill(-process_group, SIGKILL) };
}

fn reap_child_until(child: &mut Child, deadline: Instant) {
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => {
                let now = Instant::now();
                if now >= deadline {
                    return;
                }
                thread::sleep(
                    Duration::from_millis(2).min(deadline.saturating_duration_since(now)),
                );
            }
        }
    }
}

fn elapsed_millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    Observed,
    Absent,
    Failed,
    TimedOut,
    Malformed,
    Unsupported,
    Skipped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvidence<T> {
    pub status: ProbeStatus,
    pub kind: EvidenceKind,
    pub value: Option<T>,
    pub source: String,
    pub observed_at_unix_ms: Option<u64>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthDiagnostic {
    pub state: AuthState,
    pub status: ProbeStatus,
    pub kind: EvidenceKind,
    pub source: String,
    pub observed_at_unix_ms: Option<u64>,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    Ready,
    InstalledUnverified,
    NeedsAttention,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemCode {
    ExecutableMissing,
    VersionProbeUnsupported,
    VersionProbeFailed,
    VersionProbeTimedOut,
    VersionOutputMalformed,
    AuthProbeUnsupported,
    AuthProbeFailed,
    AuthProbeTimedOut,
    AuthOutputMalformed,
    AuthMissing,
    AuthExpired,
    AuthUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticProblem {
    pub code: ProblemCode,
    pub severity: Severity,
    pub summary: String,
    pub recovery: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOutcome {
    Succeeded,
    Failed,
    TimedOut,
    SpawnFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEvidence {
    /// Logical executable name only; the absolute host path is intentionally omitted.
    pub program: String,
    pub args: Vec<String>,
    pub outcome: CommandOutcome,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorDiagnostic {
    pub descriptor: ConnectorDescriptor,
    pub installation: DiagnosticEvidence<PathBuf>,
    pub version: DiagnosticEvidence<String>,
    pub auth: AuthDiagnostic,
    pub readiness: Readiness,
    pub health: Severity,
    pub problems: Vec<DiagnosticProblem>,
    pub command_evidence: Vec<CommandEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub checked_at_unix_ms: u64,
    pub connectors: Vec<ConnectorDiagnostic>,
}

/// Runs bounded local diagnostics. Call this from a blocking worker, not a UI thread.
pub fn diagnose_known_connectors() -> DiagnosticReport {
    diagnose_known_connectors_with(
        &EnvironmentPath,
        &SystemCommandRunner::default(),
        unix_time_millis(),
    )
}

/// Test seam and embedding API for deterministic lookup, command execution, and time.
pub fn diagnose_known_connectors_with(
    lookup: &impl ExecutableLookup,
    runner: &impl CommandRunner,
    checked_at_unix_ms: u64,
) -> DiagnosticReport {
    DiagnosticReport {
        checked_at_unix_ms,
        connectors: KNOWN_LOCAL_CLIS
            .iter()
            .map(|definition| diagnose_connector(definition, lookup, runner, checked_at_unix_ms))
            .collect(),
    }
}

fn unix_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn diagnose_connector(
    definition: &CliDefinition,
    lookup: &impl ExecutableLookup,
    runner: &impl CommandRunner,
    checked_at_unix_ms: u64,
) -> ConnectorDiagnostic {
    let descriptor = definition.descriptor();
    let Some(installed_path) = find_definition_executable(lookup, definition) else {
        return ConnectorDiagnostic {
            descriptor,
            installation: DiagnosticEvidence {
                status: ProbeStatus::Absent,
                kind: EvidenceKind::Inferred,
                value: None,
                source: "PATH".into(),
                observed_at_unix_ms: Some(checked_at_unix_ms),
                detail: Some(format!(
                    "`{}` was not found on PATH.",
                    definition.executable
                )),
            },
            version: DiagnosticEvidence {
                status: ProbeStatus::Skipped,
                kind: EvidenceKind::Inferred,
                value: None,
                source: "not run".into(),
                observed_at_unix_ms: None,
                detail: Some("Version probe skipped because the executable is absent.".into()),
            },
            auth: AuthDiagnostic {
                state: AuthState::Unknown,
                status: ProbeStatus::Skipped,
                kind: EvidenceKind::Inferred,
                source: "not run".into(),
                observed_at_unix_ms: None,
                detail: Some(
                    "Authentication probe skipped because the executable is absent.".into(),
                ),
            },
            readiness: Readiness::Unavailable,
            health: Severity::Unknown,
            problems: vec![DiagnosticProblem {
                code: ProblemCode::ExecutableMissing,
                severity: Severity::Unknown,
                summary: format!(
                    "{} is not installed or not on PATH.",
                    definition.display_name
                ),
                recovery: Some(format!(
                    "Install {} or add `{}` to PATH, then run diagnostics again.",
                    definition.display_name, definition.executable
                )),
            }],
            command_evidence: Vec::new(),
        };
    };

    let installation = DiagnosticEvidence {
        status: ProbeStatus::Observed,
        kind: EvidenceKind::Observed,
        value: Some(installed_path.clone()),
        source: "PATH".into(),
        observed_at_unix_ms: Some(checked_at_unix_ms),
        detail: None,
    };
    let mut command_evidence = Vec::new();
    let (version, version_problem) = probe_version(
        definition,
        &installed_path,
        runner,
        checked_at_unix_ms,
        &mut command_evidence,
    );
    let (auth, auth_problem) = probe_auth(
        definition,
        &installed_path,
        runner,
        checked_at_unix_ms,
        &mut command_evidence,
    );

    let mut problems = Vec::new();
    if let Some(problem) = version_problem {
        problems.push(problem);
    }
    if let Some(problem) = auth_problem {
        problems.push(problem);
    }

    let version_ok = version.status == ProbeStatus::Observed;
    let (readiness, health) = if auth.state == AuthState::Confirmed && version_ok {
        (Readiness::Ready, Severity::Healthy)
    } else if matches!(auth.state, AuthState::Expired | AuthState::Missing)
        || (!version_ok && version.status != ProbeStatus::Unsupported)
    {
        (Readiness::NeedsAttention, Severity::NeedsAttention)
    } else {
        (Readiness::InstalledUnverified, Severity::Unknown)
    };

    ConnectorDiagnostic {
        descriptor,
        installation,
        version,
        auth,
        readiness,
        health,
        problems,
        command_evidence,
    }
}

fn probe_version(
    definition: &CliDefinition,
    installed_path: &Path,
    runner: &impl CommandRunner,
    checked_at_unix_ms: u64,
    evidence: &mut Vec<CommandEvidence>,
) -> (DiagnosticEvidence<String>, Option<DiagnosticProblem>) {
    let Some(args) = definition.version_args else {
        let detail = "No verified non-interactive version command is known for this connector.";
        return (
            DiagnosticEvidence {
                status: ProbeStatus::Unsupported,
                kind: EvidenceKind::Unsupported,
                value: None,
                source: "connector registry".into(),
                observed_at_unix_ms: None,
                detail: Some(detail.into()),
            },
            Some(DiagnosticProblem {
                code: ProblemCode::VersionProbeUnsupported,
                severity: Severity::Unknown,
                summary: detail.into(),
                recovery: None,
            }),
        );
    };

    let request = CommandRequest::new(installed_path.to_owned(), definition.executable, args);
    match runner.run(&request) {
        Ok(output) => {
            evidence.push(command_evidence(&request, &output));
            if output.exit_code != Some(0) {
                let detail = diagnostic_failure_detail(&output);
                return (
                    DiagnosticEvidence {
                        status: ProbeStatus::Failed,
                        kind: EvidenceKind::Observed,
                        value: None,
                        source: command_label(&request),
                        observed_at_unix_ms: Some(checked_at_unix_ms),
                        detail: Some(detail.clone()),
                    },
                    Some(DiagnosticProblem {
                        code: ProblemCode::VersionProbeFailed,
                        severity: Severity::NeedsAttention,
                        summary: format!("{} version probe failed.", definition.display_name),
                        recovery: Some(detail),
                    }),
                );
            }

            let version = extract_version_line(&output);

            match version {
                Some(version) => (
                    DiagnosticEvidence {
                        status: ProbeStatus::Observed,
                        kind: EvidenceKind::Observed,
                        value: Some(version),
                        source: command_label(&request),
                        observed_at_unix_ms: Some(checked_at_unix_ms),
                        detail: None,
                    },
                    None,
                ),
                None => (
                    DiagnosticEvidence {
                        status: ProbeStatus::Malformed,
                        kind: EvidenceKind::Observed,
                        value: None,
                        source: command_label(&request),
                        observed_at_unix_ms: Some(checked_at_unix_ms),
                        detail: Some(
                            "The command succeeded but returned no recognizable version.".into(),
                        ),
                    },
                    Some(DiagnosticProblem {
                        code: ProblemCode::VersionOutputMalformed,
                        severity: Severity::NeedsAttention,
                        summary: format!(
                            "{} returned malformed version output.",
                            definition.display_name
                        ),
                        recovery: Some(
                            "Upgrade or reinstall the CLI, then run diagnostics again.".into(),
                        ),
                    }),
                ),
            }
        }
        Err(failure) => {
            version_failure(definition, &request, failure, checked_at_unix_ms, evidence)
        }
    }
}

fn extract_version_line(output: &CommandOutput) -> Option<String> {
    // stdout is preferred because several CLIs emit harmless startup warnings to
    // stderr. Every accepted version line must contain a digit and must not look
    // like a warning/error; this prevents a successful command with only noisy
    // diagnostics from becoming false version evidence.
    [&output.stdout, &output.stderr]
        .into_iter()
        .flat_map(|stream| {
            sanitize_command_output(stream, MAX_EVIDENCE_CHARS)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .find(|line| {
            let normalized = line.to_ascii_lowercase();
            !normalized.starts_with("warning")
                && !normalized.starts_with("error")
                && line.chars().any(|character| character.is_ascii_digit())
                && line.chars().any(char::is_alphanumeric)
        })
        .map(|line| truncate_chars(&line, MAX_VERSION_CHARS))
}

fn version_failure(
    definition: &CliDefinition,
    request: &CommandRequest,
    failure: CommandFailure,
    checked_at_unix_ms: u64,
    evidence: &mut Vec<CommandEvidence>,
) -> (DiagnosticEvidence<String>, Option<DiagnosticProblem>) {
    let (status, code, detail) = match &failure {
        CommandFailure::TimedOut { .. } => (
            ProbeStatus::TimedOut,
            ProblemCode::VersionProbeTimedOut,
            format!("Version probe exceeded {} ms.", request.timeout.as_millis()),
        ),
        CommandFailure::Spawn(error) | CommandFailure::Wait(error) => (
            ProbeStatus::Failed,
            ProblemCode::VersionProbeFailed,
            sanitize_command_output(error, 400),
        ),
    };
    evidence.push(command_failure_evidence(request, &failure));
    (
        DiagnosticEvidence {
            status,
            kind: EvidenceKind::Observed,
            value: None,
            source: command_label(request),
            observed_at_unix_ms: Some(checked_at_unix_ms),
            detail: Some(detail.clone()),
        },
        Some(DiagnosticProblem {
            code,
            severity: Severity::NeedsAttention,
            summary: format!(
                "{} version probe did not complete.",
                definition.display_name
            ),
            recovery: Some(detail),
        }),
    )
}

fn probe_auth(
    definition: &CliDefinition,
    installed_path: &Path,
    runner: &impl CommandRunner,
    checked_at_unix_ms: u64,
    evidence: &mut Vec<CommandEvidence>,
) -> (AuthDiagnostic, Option<DiagnosticProblem>) {
    let (args, parser): (&[&str], fn(&CommandOutput) -> ParsedAuth) = match definition.auth_probe {
        AuthProbe::CodexLoginStatus => (&["login", "status"], parse_codex_auth),
        AuthProbe::ClaudeAuthStatusJson => (&["auth", "status", "--json"], parse_claude_auth),
        AuthProbe::CursorStatus => (&["status"], parse_cursor_auth),
        AuthProbe::Unsupported(reason) => {
            return (
                AuthDiagnostic {
                    state: AuthState::Unsupported,
                    status: ProbeStatus::Unsupported,
                    kind: EvidenceKind::Unsupported,
                    source: "connector registry".into(),
                    observed_at_unix_ms: None,
                    detail: Some(reason.into()),
                },
                None,
            );
        }
    };

    let request = CommandRequest::new(installed_path.to_owned(), definition.executable, args);
    match runner.run(&request) {
        Ok(output) => {
            let parsed = parser(&output);
            evidence.push(auth_command_evidence(&request, &output));
            let problem = auth_problem(definition, parsed.state, parsed.status, &parsed.detail);
            (
                AuthDiagnostic {
                    state: parsed.state,
                    status: parsed.status,
                    kind: EvidenceKind::Observed,
                    source: command_label(&request),
                    observed_at_unix_ms: Some(checked_at_unix_ms),
                    detail: parsed.detail,
                },
                problem,
            )
        }
        Err(failure) => {
            let (status, code, detail) = match &failure {
                CommandFailure::TimedOut { .. } => (
                    ProbeStatus::TimedOut,
                    ProblemCode::AuthProbeTimedOut,
                    format!(
                        "Authentication probe exceeded {} ms.",
                        request.timeout.as_millis()
                    ),
                ),
                CommandFailure::Spawn(_) | CommandFailure::Wait(_) => (
                    ProbeStatus::Failed,
                    ProblemCode::AuthProbeFailed,
                    "Authentication probe could not be started or awaited.".into(),
                ),
            };
            evidence.push(auth_command_failure_evidence(&request, &failure));
            (
                AuthDiagnostic {
                    state: AuthState::Unknown,
                    status,
                    kind: EvidenceKind::Observed,
                    source: command_label(&request),
                    observed_at_unix_ms: Some(checked_at_unix_ms),
                    detail: Some(detail.clone()),
                },
                Some(DiagnosticProblem {
                    code,
                    severity: Severity::NeedsAttention,
                    summary: format!(
                        "{} authentication probe did not complete.",
                        definition.display_name
                    ),
                    recovery: Some(detail),
                }),
            )
        }
    }
}

#[derive(Clone, Debug)]
struct ParsedAuth {
    state: AuthState,
    status: ProbeStatus,
    detail: Option<String>,
}

fn parse_codex_auth(output: &CommandOutput) -> ParsedAuth {
    parse_text_auth(output, &["logged in"])
}

fn parse_cursor_auth(output: &CommandOutput) -> ParsedAuth {
    parse_text_auth(output, &["logged in as", "authenticated as"])
}

fn parse_text_auth(output: &CommandOutput, confirmed_markers: &[&str]) -> ParsedAuth {
    let combined = format!("{}\n{}", output.stdout, output.stderr);
    let normalized = strip_ansi_and_controls(&combined).to_ascii_lowercase();
    if contains_expired_marker(&normalized) {
        return ParsedAuth {
            state: AuthState::Expired,
            status: ProbeStatus::Observed,
            detail: Some("The CLI explicitly reported expired or invalid credentials.".into()),
        };
    }
    if contains_missing_marker(&normalized) {
        return ParsedAuth {
            state: AuthState::Missing,
            status: ProbeStatus::Observed,
            detail: Some("The CLI explicitly reported that authentication is missing.".into()),
        };
    }
    if output.exit_code == Some(0)
        && confirmed_markers
            .iter()
            .any(|marker| normalized.contains(marker))
    {
        return ParsedAuth {
            state: AuthState::Confirmed,
            status: ProbeStatus::Observed,
            detail: Some(
                "The CLI status command directly confirmed credentials are present.".into(),
            ),
        };
    }

    ParsedAuth {
        state: AuthState::Unknown,
        status: if output.exit_code == Some(0) {
            ProbeStatus::Malformed
        } else {
            ProbeStatus::Failed
        },
        detail: Some(if output.exit_code == Some(0) {
            "The authentication command succeeded without a recognized status.".into()
        } else {
            match output.exit_code {
                Some(code) => format!(
                    "The authentication command exited with status {code} without a recognized status."
                ),
                None => "The authentication command ended without a recognized status.".into(),
            }
        }),
    }
}

fn parse_claude_auth(output: &CommandOutput) -> ParsedAuth {
    let combined = format!("{}\n{}", output.stdout, output.stderr);
    let normalized = strip_ansi_and_controls(&combined).to_ascii_lowercase();
    if contains_expired_marker(&normalized) {
        return ParsedAuth {
            state: AuthState::Expired,
            status: ProbeStatus::Observed,
            detail: Some("The CLI explicitly reported expired or invalid credentials.".into()),
        };
    }

    if output.exit_code == Some(0)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(output.stdout.trim())
        && let Some(logged_in) = value.get("loggedIn").and_then(serde_json::Value::as_bool)
    {
        return ParsedAuth {
            state: if logged_in {
                AuthState::Confirmed
            } else {
                AuthState::Missing
            },
            status: ProbeStatus::Observed,
            detail: Some(if logged_in {
                "Claude Code directly reported loggedIn=true.".into()
            } else {
                "Claude Code directly reported loggedIn=false.".into()
            }),
        };
    }

    ParsedAuth {
        state: AuthState::Unknown,
        status: if output.exit_code == Some(0) {
            ProbeStatus::Malformed
        } else {
            ProbeStatus::Failed
        },
        detail: Some(if output.exit_code == Some(0) {
            "Claude Code returned JSON without a boolean loggedIn field.".into()
        } else {
            match output.exit_code {
                Some(code) => format!(
                    "Claude Code authentication status exited with status {code} without a recognized status."
                ),
                None => {
                    "Claude Code authentication status ended without a recognized status.".into()
                }
            }
        }),
    }
}

fn contains_expired_marker(normalized: &str) -> bool {
    [
        "token expired",
        "credentials expired",
        "authentication expired",
        "invalid token",
        "invalid credentials",
        "unauthorized",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn contains_missing_marker(normalized: &str) -> bool {
    [
        "not logged in",
        "not authenticated",
        "no credentials",
        "login required",
        "authentication required",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn auth_problem(
    definition: &CliDefinition,
    state: AuthState,
    status: ProbeStatus,
    detail: &Option<String>,
) -> Option<DiagnosticProblem> {
    let (code, severity, summary, recovery) = match state {
        AuthState::Confirmed => return None,
        AuthState::Missing => (
            ProblemCode::AuthMissing,
            Severity::NeedsAttention,
            format!("{} is not logged in.", definition.display_name),
            Some(format!(
                "Log in with `{0} login`, then run diagnostics again.",
                definition.executable
            )),
        ),
        AuthState::Expired => (
            ProblemCode::AuthExpired,
            Severity::NeedsAttention,
            format!("{} credentials appear expired.", definition.display_name),
            Some(format!(
                "Authenticate `{}` again, then run diagnostics.",
                definition.executable
            )),
        ),
        AuthState::Unknown => {
            let code = match status {
                ProbeStatus::Malformed => ProblemCode::AuthOutputMalformed,
                ProbeStatus::TimedOut => ProblemCode::AuthProbeTimedOut,
                ProbeStatus::Failed => ProblemCode::AuthProbeFailed,
                _ => ProblemCode::AuthUnknown,
            };
            (
                code,
                Severity::Unknown,
                format!(
                    "{} authentication could not be verified.",
                    definition.display_name
                ),
                detail.clone(),
            )
        }
        AuthState::Unsupported => return None,
    };

    Some(DiagnosticProblem {
        code,
        severity,
        summary,
        recovery,
    })
}

fn command_label(request: &CommandRequest) -> String {
    std::iter::once(request.display_program.as_str())
        .chain(request.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

fn command_evidence(request: &CommandRequest, output: &CommandOutput) -> CommandEvidence {
    CommandEvidence {
        program: request.display_program.clone(),
        args: request
            .args
            .iter()
            .map(|argument| sanitize_command_output(argument, 200))
            .collect(),
        outcome: if output.exit_code == Some(0) {
            CommandOutcome::Succeeded
        } else {
            CommandOutcome::Failed
        },
        exit_code: output.exit_code,
        stdout: sanitize_command_output(&output.stdout, MAX_EVIDENCE_CHARS),
        stderr: sanitize_command_output(&output.stderr, MAX_EVIDENCE_CHARS),
        duration_ms: output.duration_ms,
    }
}

/// Authentication command payloads may contain identities, organization
/// metadata, credentials, and opaque provider identifiers. Parsing happens in
/// memory, but raw streams never cross the connector boundary.
fn auth_command_evidence(request: &CommandRequest, output: &CommandOutput) -> CommandEvidence {
    CommandEvidence {
        program: request.display_program.clone(),
        args: request
            .args
            .iter()
            .map(|argument| sanitize_command_output(argument, 200))
            .collect(),
        outcome: if output.exit_code == Some(0) {
            CommandOutcome::Succeeded
        } else {
            CommandOutcome::Failed
        },
        exit_code: output.exit_code,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: output.duration_ms,
    }
}

fn command_failure_evidence(request: &CommandRequest, failure: &CommandFailure) -> CommandEvidence {
    let (outcome, stdout, stderr, duration_ms) = match failure {
        CommandFailure::TimedOut {
            stdout,
            stderr,
            duration_ms,
        } => (
            CommandOutcome::TimedOut,
            sanitize_command_output(stdout, MAX_EVIDENCE_CHARS),
            sanitize_command_output(stderr, MAX_EVIDENCE_CHARS),
            *duration_ms,
        ),
        CommandFailure::Spawn(error) | CommandFailure::Wait(error) => (
            CommandOutcome::SpawnFailed,
            String::new(),
            sanitize_command_output(error, MAX_EVIDENCE_CHARS),
            0,
        ),
    };
    CommandEvidence {
        program: request.display_program.clone(),
        args: request
            .args
            .iter()
            .map(|argument| sanitize_command_output(argument, 200))
            .collect(),
        outcome,
        exit_code: None,
        stdout,
        stderr,
        duration_ms,
    }
}

fn auth_command_failure_evidence(
    request: &CommandRequest,
    failure: &CommandFailure,
) -> CommandEvidence {
    let (outcome, duration_ms) = match failure {
        CommandFailure::TimedOut { duration_ms, .. } => (CommandOutcome::TimedOut, *duration_ms),
        CommandFailure::Spawn(_) | CommandFailure::Wait(_) => (CommandOutcome::SpawnFailed, 0),
    };
    CommandEvidence {
        program: request.display_program.clone(),
        args: request
            .args
            .iter()
            .map(|argument| sanitize_command_output(argument, 200))
            .collect(),
        outcome,
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms,
    }
}

fn diagnostic_failure_detail(output: &CommandOutput) -> String {
    let raw = if output.stderr.trim().is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    let detail = sanitize_command_output(raw, 400);
    if detail.trim().is_empty() {
        match output.exit_code {
            Some(code) => format!("Command exited with status {code}."),
            None => "Command terminated without an exit status.".into(),
        }
    } else {
        detail
    }
}

/// Removes terminal control sequences, redacts likely credentials, and bounds output.
pub fn sanitize_command_output(input: &str, max_chars: usize) -> String {
    let mut sanitized = strip_ansi_and_controls(input);
    redact_marker_values(&mut sanitized);
    redact_known_token_prefixes(&mut sanitized);
    truncate_chars(&sanitized, max_chars)
}

fn strip_ansi_and_controls(input: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Text,
        Escape,
        Csi,
        Osc,
        OscEscape,
    }

    let mut state = State::Text;
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        state = match state {
            State::Text if character == '\u{1b}' => State::Escape,
            State::Text => {
                if character == '\n' || character == '\t' || !character.is_control() {
                    output.push(character);
                }
                State::Text
            }
            State::Escape if character == '[' => State::Csi,
            State::Escape if character == ']' => State::Osc,
            State::Escape => State::Text,
            State::Csi if ('@'..='~').contains(&character) => State::Text,
            State::Csi => State::Csi,
            State::Osc if character == '\u{7}' => State::Text,
            State::Osc if character == '\u{1b}' => State::OscEscape,
            State::Osc => State::Osc,
            State::OscEscape if character == '\\' => State::Text,
            State::OscEscape => State::Osc,
        };
    }
    output
}

fn redact_marker_values(value: &mut String) {
    const FIELD_MARKERS: &[&str] = &[
        "authorization",
        "access_token",
        "accesstoken",
        "api_key",
        "apikey",
        "secret",
        "password",
        "email",
    ];
    let lower = value.to_ascii_lowercase();
    let bytes = value.as_bytes();
    let mut ranges = Vec::new();

    for marker in FIELD_MARKERS {
        let mut search_from = 0;
        while let Some(relative) = lower[search_from..].find(marker) {
            let marker_start = search_from + relative;
            let mut start = marker_start + marker.len();
            while start < bytes.len()
                && matches!(bytes[start], b' ' | b'\t' | b'"' | b'\'' | b':' | b'=')
            {
                start += 1;
            }
            let mut end = start;
            if *marker == "authorization" {
                while end < bytes.len() && !matches!(bytes[end], b'\r' | b'\n') {
                    end += 1;
                }
            } else {
                while end < bytes.len()
                    && !matches!(
                        bytes[end],
                        b' ' | b'\t' | b'\r' | b'\n' | b'"' | b'\'' | b',' | b'}' | b']'
                    )
                {
                    end += 1;
                }
            }
            if end > start {
                ranges.push((start, end));
            }
            search_from = marker_start + marker.len();
        }
    }

    let mut bearer_search = 0;
    while let Some(relative) = lower[bearer_search..].find("bearer") {
        let marker_start = bearer_search + relative;
        let mut start = marker_start + "bearer".len();
        while start < bytes.len()
            && matches!(bytes[start], b' ' | b'\t' | b'"' | b'\'' | b':' | b'=')
        {
            start += 1;
        }
        let mut end = start;
        while end < bytes.len()
            && !matches!(
                bytes[end],
                b' ' | b'\t' | b'\r' | b'\n' | b'"' | b'\'' | b',' | b'}' | b']'
            )
        {
            end += 1;
        }
        if end > start {
            ranges.push((start, end));
        }
        bearer_search = marker_start + "bearer".len();
    }

    ranges.sort_unstable_by_key(|range| range.0);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && range.0 <= last.1
        {
            last.1 = last.1.max(range.1);
        } else {
            merged.push(range);
        }
    }
    for (start, end) in merged.into_iter().rev() {
        value.replace_range(start..end, "[redacted]");
    }
}

fn redact_known_token_prefixes(value: &mut String) {
    const PREFIXES: &[&str] = &["sk-ant-", "sk-", "xai-", "aiza", "ghp_", "github_pat_"];
    loop {
        let lower = value.to_ascii_lowercase();
        let match_start = PREFIXES
            .iter()
            .filter_map(|prefix| lower.find(prefix).map(|index| (index, prefix.len())))
            .min_by_key(|(index, _)| *index);
        let Some((start, prefix_len)) = match_start else {
            break;
        };
        let bytes = value.as_bytes();
        let mut end = start + prefix_len;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric()
                || matches!(bytes[end], b'-' | b'_' | b'.' | b'/' | b'+' | b'='))
        {
            end += 1;
        }
        value.replace_range(start..end, "[redacted]");
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        truncated.push_str("\n… output truncated");
    }
    truncated
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        fs,
        sync::Mutex,
    };

    use super::*;

    #[derive(Default)]
    struct FakeLookup(HashMap<String, PathBuf>);

    impl FakeLookup {
        fn installed(executable: &str) -> Self {
            Self(HashMap::from([(
                executable.to_owned(),
                PathBuf::from(format!("/mock/bin/{executable}")),
            )]))
        }
    }

    impl ExecutableLookup for FakeLookup {
        fn find(&self, executable: &str) -> Option<PathBuf> {
            self.0.get(executable).cloned()
        }
    }

    #[derive(Default)]
    struct FakeRunner {
        results: Mutex<VecDeque<(String, Result<CommandOutput, CommandFailure>)>>,
    }

    impl FakeRunner {
        fn with(
            results: impl IntoIterator<Item = (&'static str, Result<CommandOutput, CommandFailure>)>,
        ) -> Self {
            Self {
                results: Mutex::new(
                    results
                        .into_iter()
                        .map(|(command, result)| (command.to_owned(), result))
                        .collect(),
                ),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, request: &CommandRequest) -> Result<CommandOutput, CommandFailure> {
            let command = command_label(request);
            let mut results = self.results.lock().expect("fake runner lock");
            let index = results
                .iter()
                .position(|(expected, _)| expected == &command)
                .unwrap_or_else(|| panic!("unexpected command: {command}"));
            results.remove(index).expect("queued result").1
        }
    }

    fn output(code: i32, stdout: &str, stderr: &str) -> Result<CommandOutput, CommandFailure> {
        Ok(CommandOutput {
            exit_code: Some(code),
            stdout: stdout.into(),
            stderr: stderr.into(),
            duration_ms: 7,
        })
    }

    fn diagnostic_for<'a>(report: &'a DiagnosticReport, id: &str) -> &'a ConnectorDiagnostic {
        report
            .connectors
            .iter()
            .find(|diagnostic| diagnostic.descriptor.id == id)
            .expect("connector diagnostic")
    }

    #[test]
    fn registry_is_provider_neutral_and_explicit() {
        let descriptors = known_connector_descriptors();
        assert_eq!(descriptors.len(), 8);
        assert_eq!(
            descriptors
                .iter()
                .map(|descriptor| descriptor.id)
                .collect::<Vec<_>>(),
            vec![
                "codex",
                "claude",
                "grok",
                "cursor",
                "antigravity",
                "gemini",
                "aider",
                "opencode"
            ]
        );
        let codex = &descriptors[0];
        assert!(codex.current_capabilities.auth_probe);
        assert!(!codex.current_capabilities.sessions);
        assert!(!codex.current_capabilities.chat);
        assert!(!codex.current_capabilities.files);
        assert!(!codex.current_capabilities.event_stream);
        assert!(!codex.current_capabilities.costs);
        assert!(codex.transports.iter().any(|transport| {
            transport.kind == TransportKind::JsonRpcStdio
                && transport.implementation == AdapterImplementation::Available
                && transport.capabilities.sessions
                && transport.capabilities.chat
                && transport.capabilities.files
                && !transport.capabilities.costs
        }));
        let gemini = descriptors
            .iter()
            .find(|descriptor| descriptor.id == "gemini")
            .unwrap();
        assert!(!gemini.current_capabilities.auth_probe);
    }

    #[test]
    fn path_discovery_never_claims_authentication() {
        let probes = probe_known_local_clis(&FakeLookup::installed("codex"));
        assert!(
            probes
                .iter()
                .all(|probe| probe.auth_state == AuthState::Unknown)
        );
        assert_eq!(probes[0].install_evidence, EvidenceKind::Observed);
        assert_eq!(probes[1].install_evidence, EvidenceKind::Inferred);
    }

    #[cfg(unix)]
    #[test]
    fn deterministic_path_lookup_uses_first_executable_and_deduplicates_directories() {
        use std::os::unix::fs::PermissionsExt;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!("utu-path-test-{}-{nonce}", std::process::id()));
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        for directory in [&first, &second] {
            let executable = directory.join("agent-cli");
            fs::write(&executable, b"#!/bin/sh\n").unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let lookup = DeterministicPathLookup::new(vec![first.clone(), second, first.clone()]);
        assert_eq!(lookup.find("agent-cli"), Some(first.join("agent-cli")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn absent_connector_skips_commands_and_stays_unavailable() {
        let report =
            diagnose_known_connectors_with(&FakeLookup::default(), &FakeRunner::default(), 42);
        let codex = diagnostic_for(&report, "codex");
        assert_eq!(codex.installation.status, ProbeStatus::Absent);
        assert_eq!(codex.version.status, ProbeStatus::Skipped);
        assert_eq!(codex.auth.state, AuthState::Unknown);
        assert_eq!(codex.readiness, Readiness::Unavailable);
        assert!(codex.command_evidence.is_empty());
    }

    #[test]
    fn direct_codex_auth_evidence_can_mark_ready() {
        let runner = FakeRunner::with([
            ("codex --version", output(0, "codex-cli 1.2.3\n", "")),
            (
                "codex login status",
                output(0, "Logged in using ChatGPT\n", ""),
            ),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("codex"), &runner, 42);
        let codex = diagnostic_for(&report, "codex");
        assert_eq!(codex.version.value.as_deref(), Some("codex-cli 1.2.3"));
        assert_eq!(codex.auth.state, AuthState::Confirmed);
        assert_eq!(codex.readiness, Readiness::Ready);
        assert_eq!(codex.health, Severity::Healthy);
    }

    #[test]
    fn version_failure_is_classified_without_erasing_auth_evidence() {
        let runner = FakeRunner::with([
            ("codex --version", output(2, "", "broken install")),
            (
                "codex login status",
                output(0, "Logged in using API key", ""),
            ),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("codex"), &runner, 42);
        let codex = diagnostic_for(&report, "codex");
        assert_eq!(codex.version.status, ProbeStatus::Failed);
        assert_eq!(codex.auth.state, AuthState::Confirmed);
        assert_eq!(codex.readiness, Readiness::NeedsAttention);
        assert!(
            codex
                .problems
                .iter()
                .any(|problem| problem.code == ProblemCode::VersionProbeFailed)
        );
    }

    #[test]
    fn claude_json_confirms_and_denies_auth_without_guessing() {
        let confirmed = FakeRunner::with([
            ("claude --version", output(0, "2.0.0", "")),
            (
                "claude auth status --json",
                output(0, r#"{"loggedIn":true,"authMethod":"oauth"}"#, ""),
            ),
        ]);
        let report =
            diagnose_known_connectors_with(&FakeLookup::installed("claude"), &confirmed, 42);
        assert_eq!(
            diagnostic_for(&report, "claude").auth.state,
            AuthState::Confirmed
        );

        let missing = FakeRunner::with([
            ("claude --version", output(0, "2.0.0", "")),
            (
                "claude auth status --json",
                output(0, r#"{"loggedIn":false,"authMethod":"none"}"#, ""),
            ),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("claude"), &missing, 42);
        let claude = diagnostic_for(&report, "claude");
        assert_eq!(claude.auth.state, AuthState::Missing);
        assert!(
            claude
                .problems
                .iter()
                .any(|problem| problem.code == ProblemCode::AuthMissing)
        );
    }

    #[test]
    fn authentication_payload_metadata_never_crosses_diagnostic_boundary() {
        let private_payload = r#"{"loggedIn":true,"email":"owner@example.com","orgName":"owner@example.com private workspace","orgId":"org_private_7f91","subscriptionType":"private-ultra","accessToken":"eyJhbGciOi.private.jwt"}"#;
        let runner = FakeRunner::with([
            ("claude --version", output(0, "2.0.0", "")),
            (
                "claude auth status --json",
                output(
                    0,
                    private_payload,
                    "secondary identity owner@example.com; tenant_private_44",
                ),
            ),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("claude"), &runner, 42);
        let claude = diagnostic_for(&report, "claude");
        assert_eq!(claude.auth.state, AuthState::Confirmed);
        let auth_evidence = &claude.command_evidence[1];
        assert!(auth_evidence.stdout.is_empty());
        assert!(auth_evidence.stderr.is_empty());

        let serialized = serde_json::to_string(&report).expect("serialize report");
        for private_fragment in [
            "owner@example.com",
            "org_private_7f91",
            "private-ultra",
            "eyJhbGciOi.private.jwt",
            "tenant_private_44",
            "orgName",
            "subscriptionType",
        ] {
            assert!(
                !serialized.contains(private_fragment),
                "serialized auth diagnostic leaked {private_fragment}"
            );
        }
    }

    #[test]
    fn failed_claude_auth_command_cannot_confirm_stale_json() {
        let runner = FakeRunner::with([
            ("claude --version", output(0, "2.0.0", "")),
            (
                "claude auth status --json",
                output(
                    1,
                    r#"{"loggedIn":true,"orgName":"stale-private-org"}"#,
                    "command failed",
                ),
            ),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("claude"), &runner, 42);
        let claude = diagnostic_for(&report, "claude");
        assert_eq!(claude.auth.state, AuthState::Unknown);
        assert_eq!(claude.auth.status, ProbeStatus::Failed);
        assert_eq!(claude.readiness, Readiness::InstalledUnverified);
        assert!(claude.command_evidence[1].stdout.is_empty());
        assert!(
            !serde_json::to_string(claude)
                .unwrap()
                .contains("stale-private-org")
        );
    }

    #[test]
    fn expired_and_unknown_auth_are_distinct() {
        let expired = FakeRunner::with([
            ("codex --version", output(0, "codex 1", "")),
            ("codex login status", output(1, "", "token expired")),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("codex"), &expired, 42);
        assert_eq!(
            diagnostic_for(&report, "codex").auth.state,
            AuthState::Expired
        );

        let malformed = FakeRunner::with([
            ("claude --version", output(0, "claude 1", "")),
            (
                "claude auth status --json",
                output(0, r#"{"unexpected":true}"#, ""),
            ),
        ]);
        let report =
            diagnose_known_connectors_with(&FakeLookup::installed("claude"), &malformed, 42);
        let claude = diagnostic_for(&report, "claude");
        assert_eq!(claude.auth.state, AuthState::Unknown);
        assert_eq!(claude.auth.status, ProbeStatus::Malformed);
        assert_ne!(claude.health, Severity::Healthy);
    }

    #[test]
    fn timeouts_are_bounded_and_classified() {
        let runner = FakeRunner::with([
            (
                "codex --version",
                Err(CommandFailure::TimedOut {
                    stdout: String::new(),
                    stderr: "still waiting".into(),
                    duration_ms: 2_001,
                }),
            ),
            (
                "codex login status",
                output(0, "Logged in using ChatGPT", ""),
            ),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("codex"), &runner, 42);
        let codex = diagnostic_for(&report, "codex");
        assert_eq!(codex.version.status, ProbeStatus::TimedOut);
        assert!(
            codex
                .problems
                .iter()
                .any(|problem| problem.code == ProblemCode::VersionProbeTimedOut)
        );
        assert_eq!(codex.command_evidence[0].outcome, CommandOutcome::TimedOut);
    }

    #[cfg(unix)]
    #[test]
    fn system_timeout_kills_descendants_that_hold_capture_pipes() {
        let request = CommandRequest {
            program: PathBuf::from("/bin/sh"),
            display_program: "sh".into(),
            args: vec!["-c".into(), "printf parent-ready; sleep 30 & wait".into()],
            timeout: Duration::from_millis(80),
        };
        let started = Instant::now();
        let failure = SystemCommandRunner::default().run(&request).unwrap_err();

        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timeout path blocked on a descendant-held pipe for {:?}",
            started.elapsed()
        );
        match failure {
            CommandFailure::TimedOut { stdout, .. } => {
                assert_eq!(stdout, "parent-ready");
            }
            other => panic!("expected timeout, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn system_success_cleans_descendants_that_hold_capture_pipes() {
        let request = CommandRequest {
            program: PathBuf::from("/bin/sh"),
            display_program: "sh".into(),
            args: vec!["-c".into(), "printf parent-ready; sleep 30 & exit 0".into()],
            timeout: Duration::from_secs(2),
        };
        let started = Instant::now();
        let output = SystemCommandRunner::default()
            .run(&request)
            .expect("parent exits successfully");

        assert!(
            started.elapsed() < Duration::from_secs(1),
            "success path blocked on a descendant-held pipe for {:?}",
            started.elapsed()
        );
        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.stdout, "parent-ready");
    }

    #[test]
    fn authentication_timeout_stays_unknown_and_redacts_partial_output() {
        let runner = FakeRunner::with([
            ("codex --version", output(0, "codex 1.0.0", "")),
            (
                "codex login status",
                Err(CommandFailure::TimedOut {
                    stdout: "Authorization: Bearer secret.session.token".into(),
                    stderr: String::new(),
                    duration_ms: 2_001,
                }),
            ),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("codex"), &runner, 42);
        let codex = diagnostic_for(&report, "codex");
        assert_eq!(codex.auth.state, AuthState::Unknown);
        assert_eq!(codex.auth.status, ProbeStatus::TimedOut);
        assert_eq!(codex.readiness, Readiness::InstalledUnverified);
        assert!(
            codex
                .problems
                .iter()
                .any(|problem| problem.code == ProblemCode::AuthProbeTimedOut)
        );
        assert!(
            !codex.command_evidence[1]
                .stdout
                .contains("secret.session.token")
        );
        assert!(codex.command_evidence[1].stdout.is_empty());
        assert!(codex.command_evidence[1].stderr.is_empty());
    }

    #[test]
    fn malformed_version_output_never_becomes_observed_version() {
        let runner = FakeRunner::with([
            ("codex --version", output(0, "\u{1b}[32m\u{1b}[0m", "")),
            (
                "codex login status",
                output(0, "Logged in using ChatGPT", ""),
            ),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("codex"), &runner, 42);
        let codex = diagnostic_for(&report, "codex");
        assert_eq!(codex.version.status, ProbeStatus::Malformed);
        assert_eq!(codex.version.value, None);
        assert_ne!(codex.health, Severity::Healthy);
    }

    #[test]
    fn version_parser_ignores_successful_startup_warnings() {
        let runner = FakeRunner::with([
            (
                "codex --version",
                output(
                    0,
                    "codex-cli 9.8.7\n",
                    "WARNING: could not create PATH aliases: Operation not permitted (os error 1)",
                ),
            ),
            (
                "codex login status",
                output(0, "Logged in using ChatGPT", ""),
            ),
        ]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("codex"), &runner, 42);
        let codex = diagnostic_for(&report, "codex");
        assert_eq!(codex.version.value.as_deref(), Some("codex-cli 9.8.7"));
        assert_eq!(codex.readiness, Readiness::Ready);
    }

    #[test]
    fn unsupported_auth_is_explicit_and_never_healthy() {
        let runner = FakeRunner::with([("gemini --version", output(0, "0.9.0", ""))]);
        let report = diagnose_known_connectors_with(&FakeLookup::installed("gemini"), &runner, 42);
        let gemini = diagnostic_for(&report, "gemini");
        assert_eq!(gemini.auth.state, AuthState::Unsupported);
        assert_eq!(gemini.auth.kind, EvidenceKind::Unsupported);
        assert_eq!(gemini.readiness, Readiness::InstalledUnverified);
        assert_eq!(gemini.health, Severity::Unknown);
    }

    #[test]
    fn command_evidence_strips_terminal_controls_redacts_secrets_and_truncates() {
        let raw = concat!(
            "\u{1b}[31mAuthorization: Bearer abc.def.secret\u{1b}[0m\n",
            "api_key=sk-ant-super-secret\n",
            "accessToken: xai-another-secret\n",
            "email=owner@example.com"
        );
        let sanitized = sanitize_command_output(raw, 1_000);
        assert!(!sanitized.contains('\u{1b}'));
        assert!(!sanitized.contains("abc.def.secret"));
        assert!(!sanitized.contains("sk-ant-super-secret"));
        assert!(!sanitized.contains("xai-another-secret"));
        assert!(!sanitized.contains("owner@example.com"));
        assert!(sanitized.contains("[redacted]"));

        let truncated = sanitize_command_output(&"x".repeat(100), 8);
        assert!(truncated.starts_with("xxxxxxxx"));
        assert!(truncated.contains("truncated"));
    }
}
