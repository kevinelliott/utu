# Utu connector diagnostics

This crate is Utu's provider-neutral local CLI registry and bounded diagnostic
engine. It deliberately keeps three claims separate:

- **Installation**: an executable was found on the current process `PATH`.
- **Version**: a bounded `--version` probe returned recognizable output.
- **Authentication**: a documented, non-interactive status command directly
  reported a state.

Finding a binary or a config file never implies authentication. A connector
cannot be reported `ready` / `healthy` unless direct auth evidence and observed
version evidence are both present. Unsupported auth checks remain explicit.

## Current probe contracts

The command surface was verified on 2026-08-12 using official documentation
where available and installed CLI `--help` output otherwise.

| Connector | Version | Authentication | Current boundary |
| --- | --- | --- | --- |
| Codex | `codex --version` | `codex login status` | Auth command is documented in the [official Codex command reference](https://learn.chatgpt.com/docs/developer-commands?surface=cli). The bounded App Server initialize + `thread/list` path is live-verified against CLI 0.147.0; read/resume/start, text turns, events, and file changes pass fake-process conformance tests. Server requests are rejected and costs are not inferred. |
| Claude Code | `claude --version` | `claude auth status --json` | Verified from installed `claude --help` / `claude auth status --help`. |
| Cursor Agent | `cursor-agent --version` | `cursor-agent status` | Verified from installed `cursor-agent --help` / `status --help`. |
| Grok Build | `grok --version` | Unsupported | Installed help exposes login/logout but no safe non-interactive status command. |
| Antigravity | Unsupported | Unsupported | No stable local CLI diagnostic contract has been verified. |
| Gemini CLI | `gemini --version` | Unsupported | Installed help exposes no global non-interactive auth-status command; ACP is only planned metadata. |
| Aider | `aider --version` | Unsupported | Credentials are provider-specific; no global auth state is inferred. |
| OpenCode | `opencode --version` | Unsupported | Credential listings are provider-specific; ACP is only planned metadata. |

## Embedding API

Tauri should invoke the blocking entry point on a blocking worker:

```rust
let report: utu_connectors::DiagnosticReport =
    utu_connectors::diagnose_known_connectors();
```

Tests and alternate hosts can inject all nondeterminism:

```rust
let report = utu_connectors::diagnose_known_connectors_with(
    &lookup,
    &runner,
    checked_at_unix_ms,
);
```

`DiagnosticReport.connectors` includes the provider descriptor, installation,
version, auth, readiness, severity, typed problems, and bounded sanitized
command evidence. Authentication evidence deliberately omits raw stdout and
stderr after deriving its safe state; status output can contain identities,
organization metadata, or opaque credentials. `known_connector_descriptors()`
is available when only the registry/capability catalog is needed.

The real runner closes stdin, prevents browser-based login from opening during
status probes, continuously drains stdout/stderr while retaining at most 64 KiB
per stream, and kills the isolated Unix process group at timeout or parent exit.
Windows currently terminates the direct child while capture collection remains
bounded. Non-auth evidence is stripped of terminal control sequences and
credential-shaped values are redacted before serialization.
