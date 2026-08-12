# Roadmap

This sequence keeps the desktop app primary, establishes deep local-agent truth first, and begins cloud coverage immediately afterward.

## Local-first foundation — implemented in this repository

- Tauri 2 + Leptos native workspace with conversation, files, Integrations, Attention, Projects, and Fleet.
- Provider-neutral Rust types for projects, tasks, agents, sessions, messages, events, evidence, controls, costs, isolation, and handoffs.
- SQLite migrations and repositories for operational state, cost confidence, search, assignments, and auditable request/receipt separation.
- Deterministic local CLI discovery plus bounded version and supported authentication diagnostics for eight connector families.
- Native IPC for CRUD, assignment, snapshots, diagnostics, chat intent, handoffs, attention, search, and project-root file previews.
- Real diagnostic evidence and local-store state in the desktop surface; separately labeled sample data demonstrates future live session states.
- Experimental Codex App Server slice: explicit one-project, metadata-only
  session sync and separately armed text direction. Authorization is ephemeral,
  provider policies are requested rather than containment, and acknowledgement
  is not completion.
- Secondary read-only browser demonstration shell; no live projection
  transport yet.

Foundation completion is not live agent orchestration. Ordinary CLI integrations
remain diagnostics-only. The Codex exception does not ingest transcripts,
responses, events, file changes, costs, or approvals, and its mutating paths are
fake-process-tested rather than exercised against owner sessions.

## Milestone 1 — local CLI vertical slice

- Retention policies and export over the implemented SQLite store.
- OS keychain service and redaction pipeline.
- Finish native supervision and durable projection over the bounded Codex App
  Server transport; add transcript/response/event ingestion, reviewed approval
  handling, file-change projection, usage/cost evidence, reconciliation, and
  restart recovery. The transport crate's broader APIs are not all exposed by
  the current native slice.
- Supervisor lifecycle, backpressure, cancellation, and recovery.
- Real Attention findings and project/session import.

Exit condition: one local CLI can be discovered, truthfully health-checked, observed, directed, and recovered end to end without demo state.

## Milestone 2 — cloud agents, immediately next

Cloud coverage is the priority directly after the local vertical slice, not a
distant optional phase.

- Claude Work and ChatGPT Work discovery/spikes, using supported APIs first.
- Cursor and other cloud-agent integrations.
- Explicit browser-mediated fallback only where permitted and necessary.
- Reconciled cloud/local projects, sessions, identity, cost, and health.
- Opt-in remote read-only status with strong authentication and revocation.

## Milestone 3 — local fleet expansion

- Claude Code, Cursor Agent, Grok Build, Gemini CLI/ACP, OpenCode/ACP, Antigravity, and other available session transports. Their current discovery/diagnostic profiles remain useful while orchestration matures.
- Multi-agent task assignment and auditable handoffs.
- Cross-provider log search, file-change correlation, and normalized cost analysis.
- Connector conformance harness and fault-injection suite.
- macOS, Windows, and Linux packaging gates.

No current CI or `--no-bundle` build is a signed, installable, or verified
multi-platform release.

## Milestone 4 — isolation and automation depth

- Optional process-sandbox, container, local-VM, and remote-VM execution
  profiles; host execution remains available and visibly labeled.
- Mount/network/resource policies and boundary health checks.
- Rules for stalled work, auth expiry, abnormal cost, retries, and owner escalation.
- Scheduled health checks and local notifications.

## Milestone 5 — small teams

Initial releases are single-owner. Small-team support is confirmed follow-on scope and remains explicit in the domain design.

- shared projects and task ownership;
- viewer/operator/admin roles;
- approval policies for controls, credentials, and handoffs;
- collaborative audit history, comments, and presence;
- optional encrypted synchronization without weakening local-first operation.
