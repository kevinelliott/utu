# Utu

Utu is a local-first desktop operations app for AI agents. It brings local agent CLIs and, next, cloud agents into one project-centered workspace with four primary views:

- **Workspace** — project conversation, plans, tools, permissions, files, tests, and evidence.
- **Attention** — decisions, failures, authentication problems, and work waiting on the owner.
- **Projects** — tasks, sessions, multi-agent assignments, handoffs, and project history.
- **Fleet** — every agent, its current session, trace, files, controls, health, and cost evidence.

Integrations is a nearby operational view for installed CLIs, versions, login evidence, and adapter capabilities.

The interface is intentionally an installed-app workspace rather than a SaaS
admin dashboard. Tauri is the primary and currently live surface; the browser
build is a read-only demonstration shell for the planned status projection.

## Current status

This repository is an executable local-first vertical slice, not a production-connected agent release. It currently includes:

- a responsive, native-feeling Leptos workspace with chat, files, activity, evidence, Integrations, Attention, Projects, and Fleet;
- a Tauri 2 host with typed IPC, window-state restoration, local logs, and a private application-data directory;
- a migration-backed SQLite store for providers, integrations, projects, tasks, assignments, agents, sessions, messages, events, file changes, costs, attention, handoffs, and control receipts;
- a shared provider-neutral Rust domain with explicit observed, inferred, stale, and unsupported states;
- bounded executable/version/login diagnostics for Codex, Claude Code, Grok Build, Cursor Agent, Antigravity, Gemini CLI, Aider, and OpenCode;
- an experimental Codex App Server slice for an explicitly selected local project: metadata-only session discovery plus one-shot, owner-armed text direction;
- canonical-root project file listing and bounded text previews with traversal and symlink escape rejection;
- a polished sample workspace for interaction states, always labeled separately from live local evidence.

Ordinary CLI integrations remain diagnostics-only. Utu validates authentication
only where a CLI exposes a verified, non-destructive status command; finding an
executable still proves only installation, never login.

Codex is the sole experimental transport exception. After explicit confirmation
for exactly one selected project, Utu reruns fresh Codex diagnostics, binds to
that exact observed executable, and imports only thread metadata whose canonical
working directory exactly matches the project's canonical root. It does not
import transcripts, agent responses, event payloads, file changes, costs, or
approval requests. The project authorization is process-local: application
restart, runtime loss, or any explicit connector refresh disables delivery
until the owner explicitly synchronizes that project again. Utu cannot yet
attest a stable provider-account identity across diagnostic and App Server
processes, so even a successful refresh revokes the prior runtime lease.

Each provider-bound Codex text direction requires a separate, one-shot owner
confirmation. Utu requests provider read-only, no-network, and `Never` approval
policies. A returned acknowledgement means only that Codex accepted the turn;
it does not prove completion. Those requested provider policies are not host
sandbox or VM containment. The installed Codex CLI has been exercised only for
the read-only initialize and thread-list path; mutating paths are covered by a
fake-process conformance harness, not by owner-session execution.

Utu does **not** yet store credentials, connect to cloud agents, enforce a
sandbox/VM, expose a live web dashboard, ingest agent output, normalize provider
costs, or handle provider approvals. Direction to every transport other than an
explicitly active Codex project is recorded locally with an `unsupported`
receipt.

## Run it

Prerequisites: Rust 1.88+, the `wasm32-unknown-unknown` target, Trunk, and the Tauri CLI/platform prerequisites.

```sh
rustup target add wasm32-unknown-unknown
trunk serve --open
```

Run the primary desktop app:

```sh
cargo tauri dev
```

Open the secondary demonstration mode at
`http://127.0.0.1:1421/?surface=web`. It does not read the desktop database, and
mutating controls are disabled. A real local/remote status transport remains a
roadmap item.

## Verify it

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
env -u NO_COLOR trunk build --release --locked
env -u NO_COLOR cargo tauri build --no-bundle -- --locked
```

These are source and host-build gates. They do not produce signed installers or
claim verified macOS, Windows, and Linux distribution.

## Repository map

- `crates/dashboard-core` (`utu-core`) — provider-neutral domain types and attention policy.
- `crates/dashboard-connectors` (`utu-connectors`) — connector registry and bounded local CLI diagnostics; no UI or Tauri dependency.
- `crates/utu-store` — local SQLite migrations, repositories, projections, and search.
- `src-tauri` — native lifecycle, commands, permissions, and OS integrations.
- `src` — shared Leptos application shell and views.
- `styles.css` — visual tokens and responsive app layout.
- `docs` — architecture, connector, security, and delivery contracts.

See [PRODUCT.md](PRODUCT.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), and [docs/ROADMAP.md](docs/ROADMAP.md) for the product and implementation boundaries.

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attribution.
