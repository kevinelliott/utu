# Roadmap

This sequence keeps the desktop app primary, establishes deep local-agent truth first, and begins cloud coverage immediately afterward.

## Foundation — implemented in this repository

- Tauri 2 + Leptos application shell.
- Attention, Projects, and Fleet views with a coherent desktop interaction model.
- Provider-neutral Rust types for projects, tasks, agents, sessions, events, evidence, controls, costs, isolation, and handoffs.
- Conservative local CLI discovery boundary and tests.
- Secondary read-only browser surface.

The UI uses demonstration data; foundation completion is not connector completion.

## Milestone 1 — local CLI vertical slice

- SQLite event store, migrations, projections, retention, and search.
- OS keychain service and redaction pipeline.
- Production Codex adapter: discovery, compatibility, login probe, sessions, structured logs, controls where supported, and cost evidence.
- Supervisor lifecycle, backpressure, cancellation, and recovery.
- Real Attention findings and project/session import.
- Host execution plus one optional sandbox or local-VM path with visible boundary health.

Exit condition: one local CLI can be discovered, truthfully health-checked, observed, directed, and recovered end to end without demo state.

## Milestone 2 — local fleet

- Claude Code, Cursor Agent, Antigravity, and other available CLI adapters.
- Multi-agent task assignment and auditable handoffs.
- Cross-provider log search, file-change correlation, and normalized cost analysis.
- Connector conformance harness and fault-injection suite.
- macOS, Windows, and Linux packaging gates.

## Milestone 3 — cloud agents, immediately next

Cloud coverage is the priority directly after the local foundation, not a distant optional phase.

- Claude Work and ChatGPT Work discovery/spikes, using supported APIs first.
- Cursor and other cloud-agent integrations.
- Explicit browser-mediated fallback only where permitted and necessary.
- Reconciled cloud/local projects, sessions, identity, cost, and health.
- Opt-in remote read-only status with strong authentication and revocation.

## Milestone 4 — isolation and automation depth

- Container, local VM, and remote VM execution profiles.
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
