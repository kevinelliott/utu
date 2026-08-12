# Utu

Utu is a local-first desktop operations app for AI agents. It brings local agent CLIs and, next, cloud agents into one project-centered workspace with three primary views:

- **Attention** — decisions, failures, authentication problems, and work waiting on the owner.
- **Projects** — tasks, sessions, multi-agent assignments, handoffs, and project history.
- **Fleet** — every agent, its current session, trace, files, controls, health, and cost evidence.

The interface is intentionally an installed-app workspace rather than a SaaS admin dashboard. Tauri is the primary surface; the browser build is a secondary read-only status surface.

## Current status

This repository is an executable product foundation, not a connected release. It currently includes:

- a responsive Leptos interface for the approved Attention, Projects, and Fleet shell;
- a Tauri 2 host with window-state restoration and local logs;
- a shared Rust domain for providers, agents, projects, tasks, sessions, events, costs, evidence, controls, and handoffs;
- a tested Rust connector boundary plus conservative local CLI path discovery;
- demonstration interactions and data, explicitly labeled in the UI.

It does **not** yet control live agents, validate their authentication, persist operational state, store credentials, or connect to cloud providers. Finding an executable proves only that the executable is present; it never proves login state.

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

Open the secondary status mode at `http://127.0.0.1:1421/?surface=web`. Mutating controls are disabled in that mode.

## Verify it

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
trunk build --release
```

## Repository map

- `crates/dashboard-core` (`utu-core`) — provider-neutral domain types and attention policy.
- `crates/dashboard-connectors` (`utu-connectors`) — connector-side discovery primitives; no UI or Tauri dependency.
- `src-tauri` — native lifecycle, commands, permissions, and OS integrations.
- `src` — shared Leptos application shell and views.
- `styles.css` — visual tokens and responsive app layout.
- `docs` — architecture, connector, security, and delivery contracts.

See [PRODUCT.md](PRODUCT.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), and [docs/ROADMAP.md](docs/ROADMAP.md) for the product and implementation boundaries.

## License

Apache-2.0.
