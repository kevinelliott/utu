# Architecture

## Shape of the system

Utu uses one provider-neutral Rust domain behind two interface builds. The
Tauri app is the authority for control, local discovery, and persistence. The
current browser build is a read-only demonstration shell; a restricted live
projection transport is planned but not implemented.

```text
Tauri desktop (primary)        Browser status (secondary)
          |                                |
          +------ Leptos shared UI --------+
                          |
             application/query layer
                          |
        Rust domain + evidence policies + event log
                          |
        connector supervisors and control receipts
             /            |             \
        local CLI       cloud API     browser mediation
             |
    host / sandbox / container / VM
```

The current code implements the shared UI, core domain, evidence policy, native
query/command layer, durable SQLite store, secure project-file reads, and bounded
local CLI diagnostics. It also contains an experimental local-agent observation slice: explicit
metadata sync for ready agents, filesystem watches for Claude Code and Codex
sessions, and one-shot, owner-armed Codex text direction. Transcript and
response ingestion, event/file/cost projection, approval handling,
long-running multi-provider supervision, cloud adapters, credentials, and
isolation runtimes remain planned boundaries rather than current claims.

## Workspace responsibilities

### `utu-core` (`crates/dashboard-core`)

Pure Rust and provider-neutral. Owns identity and serialization for providers, agents, projects, tasks, sessions, events, handoffs, evidence, capabilities, isolation mode, authentication state, and cost confidence. It must not know about Tauri, DOM APIs, subprocesses, or provider SDKs.

### `utu-connectors` (`crates/dashboard-connectors`)

Rust diagnostic boundary. It turns provider-specific observations into core
types and declares only the capabilities an adapter can prove. Current scope is
deterministic PATH discovery plus bounded version and supported authentication
probes for eight local CLI families. These registry profiles remain
diagnostics-only; merely discovering potential App Server or ACP support never
activates a control capability. Session transports, event normalization, live
controls, and persistent supervisors belong in provider-specific child crates
and the native composition layer.

### `utu-codex` (`crates/utu-codex`)

Bounded JSON-RPC-over-stdio client for the experimental Codex App Server. The
crate implements initialize, thread list/read/resume/start, text turn start,
typed notifications, resource limits, and fail-closed server-request rejection.
The installed provider has been exercised only for initialize and `thread/list`;
mutating calls and notification families are fake-process conformance-tested.
The native application deliberately exposes a smaller surface than the crate.

### `utu-store` (`crates/utu-store`)

Thread-safe local SQLite authority. Owns migrations and repositories for normalized operational records, append-ordered message/event streams, cost confidence, attention, handoffs, control request/receipts, and literal-wildcard search. It never stores provider credentials. Unknown cost remains `NULL`, not zero.

### `utu` (`src-tauri`)

Tauri composition root. Owns process lifetime, typed IPC, application-data
location, connector diagnostic workers, canonical-root filesystem reads, window
state, and local logging. It owns the experimental agent observation runtime: a fresh
diagnostic binds Codex's exact executable to authorized project roots,
metadata-only sync or startup hydration activates those projects, and
filesystem watches keep Claude Code and Codex session records current.
Blocking SQLite, process, and filesystem work is moved off the
UI thread. Credential/keychain, durable output projection,
multi-provider control supervision, and sandbox/VM coordination are future
services.

### `utu-ui` (workspace root)

Leptos CSR application compiled to WebAssembly. Owns the installed-app shell, view state, keyboard/pointer interactions, accessible rendering, and projections such as Attention, Projects, and Fleet. It consumes typed query results and control receipts; it does not infer provider health from presentation data.

## Operational model

All connector facts enter as evidence with a source, observation time, and evidence kind:

- **Observed** — directly confirmed by a connector at a known time.
- **Inferred** — derived from incomplete but relevant evidence.
- **Stale** — once observed but now outside the connector freshness budget.
- **Unsupported** — the adapter cannot produce this fact.

Unknown, stale, and unsupported are real states. They must not collapse into healthy, authenticated, zero cost, or controllable.

The durable store is append-oriented for messages and events, with mutable materialized records for projects, tasks, agents, integrations, attention, and receipts. Provider event IDs are retained where available; otherwise connectors create stable correlation and deduplication keys. Destructive record deletes require an exact identifier confirmation. Control intent and provider receipt are separate records, so a locally recorded request never becomes a false acknowledgement.

## Process model

Current diagnostic commands run off the UI thread with bounded output and
per-process timeouts. A session supervisor hydrates ready agents on startup,
watches local session files, and reconnects Codex App Server after restart.
Provider notification payloads are still discarded in metadata-only mode.
The full control supervisor—bounded concurrency, cancellation, streaming
backpressure, and health budgets—remains roadmap work.
A misbehaving adapter must not block the UI or other adapters.

A Codex `turn/start` response is stored as provider acknowledgement, not turn or
task completion. Timeouts remain unknown until reconciled. Utu currently imports
no provider responses, transcripts, events, costs, file changes, or approval
requests, so it cannot present a complete live execution trace.

Local execution targets are explicit:

- host process;
- process sandbox;
- container;
- local VM;
- remote VM.

The execution-mode enum reserves these provider-neutral targets, but the current
durable session schema does not yet assert a boundary. The isolation milestone
adds selected mode plus observed boundary health to every session. Until then,
Utu must not label host activity as sandboxed or VM-isolated.

## Web boundary

The web dashboard is not an alternate control authority. The current build
contains labeled sample states and no desktop-database transport. Its first live
delivery will be a local read-only projection. Later remote exposure requires
explicit enablement, authentication, scoped capabilities, encryption, origin
restrictions, revocation, and an audit trail. Hosted relay and team
synchronization are future extensions.
