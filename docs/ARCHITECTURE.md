# Architecture

## Shape of the system

Utu uses one provider-neutral Rust domain behind two surfaces. The Tauri app is the authority for control, credentials, local discovery, and persistence. The browser surface consumes a restricted projection and is read-only by default.

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

The current code implements the shared UI, core domain, initial attention policy, native host, and conservative executable discovery. The application/query layer, durable event store, connector supervisors, and live adapters are planned boundaries rather than current claims.

## Workspace responsibilities

### `utu-core` (`crates/dashboard-core`)

Pure Rust and provider-neutral. Owns identity and serialization for providers, agents, projects, tasks, sessions, events, handoffs, evidence, capabilities, isolation mode, authentication state, and cost confidence. It must not know about Tauri, DOM APIs, subprocesses, or provider SDKs.

### `utu-connectors` (`crates/dashboard-connectors`)

Rust integration boundary. It turns provider-specific observations into core types and declares only the capabilities an adapter can prove. Current scope is PATH-based executable discovery. Provider process supervision, structured output parsing, auth probes, session import, event normalization, and control receipts belong here or in provider-specific child crates.

### `utu` (`src-tauri`)

Tauri composition root. Owns process lifetime, native permissions, OS keychain calls, local database location, filesystem watches, connector subprocesses, sandbox/VM coordination, window state, and local logging. Commands should delegate to Rust services rather than contain integration logic.

### `utu-ui` (workspace root)

Leptos CSR application compiled to WebAssembly. Owns the installed-app shell, view state, keyboard/pointer interactions, accessible rendering, and projections such as Attention, Projects, and Fleet. It consumes typed query results and control receipts; it does not infer provider health from presentation data.

## Operational model

All connector facts enter as evidence with a source, observation time, and evidence kind:

- **Observed** — directly confirmed by a connector at a known time.
- **Inferred** — derived from incomplete but relevant evidence.
- **Stale** — once observed but now outside the connector freshness budget.
- **Unsupported** — the adapter cannot produce this fact.

Unknown, stale, and unsupported are real states. They must not collapse into healthy, authenticated, zero cost, or controllable.

The durable store will be an append-oriented local event log with materialized projections for fast UI reads. Provider event IDs are retained where available; otherwise connectors create stable correlation and deduplication keys. Destructive controls produce request and receipt events so the owner can see what was asked, what was acknowledged, and what remains unconfirmed.

## Process model

Each connector runs under a supervisor with bounded concurrency, timeouts, cancellation, restart policy, and a health budget. A misbehaving adapter must not block the UI or the other adapters. High-volume stdout/stderr is parsed and persisted off the UI thread, then delivered as compact projection updates.

Local execution targets are explicit:

- host process;
- process sandbox;
- container;
- local VM;
- remote VM.

The selected boundary and its observed health travel with every session. Isolation is opt-in in the first release; host execution remains available but is never mislabeled as sandboxed.

## Web boundary

The web dashboard is not an alternate control authority. Its first delivery is a local read-only projection. Later remote exposure requires explicit enablement, authentication, scoped capabilities, encryption, origin restrictions, revocation, and an audit trail. Hosted relay and team synchronization are future extensions.
