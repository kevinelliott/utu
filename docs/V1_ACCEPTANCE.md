# Utu v1 target acceptance contract

Utu v1 is a local-first, single-owner desktop workspace. The desktop process is
the control and persistence authority; the target browser build is a read-only
status projection. The current browser build is only a labeled demonstration
shell. Cloud-agent coverage follows the local vertical slice; optional
sandbox/VM execution and small-team operation are later milestones and must not
be implied by the first release.

This is the target contract for declaring the local CLI vertical slice v1. It
is not a claim that every item is implemented in the current foundation; the
current boundary and remaining milestones are tracked in `README.md` and
`ROADMAP.md`. A connector can be useful while supporting only part of this
contract, but the UI must never present an unsupported capability as live.

## Owner workflow

Before v1 is declared complete, the owner must be able to:

1. create a project with an optional canonical local root;
2. create tasks and assign each task to one or more known agents;
3. open a project workspace shell with conversation, plan/activity, files, and
   integration context without leaving the active selection;
4. record owner direction for a real stored session and see whether it was queued locally, acknowledged
   by a provider, rejected, timed out, or is unsupported, without treating an
   acknowledgement as turn or task completion;
5. review normalized agent messages, tool activity, file changes, logs, costs,
   failures, and handoffs with their evidence source;
6. find stored work by project/session scope and literal full-text search;
7. inspect connector installation, version, reachability, authentication,
   compatibility, and supported controls as separate facts;
8. re-probe missing binaries, expired login, failed probes, and stale
   observations without restarting the whole application.

## Product surfaces

- **Attention** is the exception inbox: approvals, auth failures, stalled work,
  connector problems, abnormal cost evidence, and owner decisions.
- **Projects** is the primary workbench: chat, tasks, sessions, files, plan,
  changes, tests, logs, integrations, assignments, and handoffs.
- **Fleet** is the live inventory: agents and connectors grouped by observed
  state, with capability-gated direction and lifecycle controls.
- **Search, coordination, integrations, costs, and settings** are compact
  workspace modes or transient inspectors, not SaaS dashboard pages.

Every live view must have intentional loading, empty, partial, unsupported,
stale, error, and read-only states. Any sample-only view must instead have a
persistent `Demo` boundary and no path into live native controls. Keyboard
focus remains visible and no status relies on color alone.

## Durable local data

SQLite stores projects, tasks, assignments, agents, providers, integrations,
sessions, messages, normalized events, file-change evidence, cost records,
attention records, and handoffs. Migrations are idempotent, foreign keys are
enabled, related records presented as one action are transactional, and unknown
costs remain `NULL` rather than becoming zero.

Provider credentials, browser session material, and raw secrets are never
stored in SQLite. A later keychain service can grant connectors short-lived
access without changing the domain model.

## Connector levels

Each adapter advertises one of these truthful levels by granular capability,
not by marketing label:

- **Detected** — executable or supported configuration was found.
- **Diagnosed** — version/reachability and a documented non-destructive auth
  probe were observed with a bounded timeout.
- **Observed** — projects, sessions, messages, events, or changes can be
  imported or watched with stable identifiers.
- **Controllable** — one or more explicit controls return provider evidence and
  a durable receipt.

Initial adapters cover known local CLIs through detection and diagnostics.
Deeper session/control transports land independently as providers expose safe,
documented interfaces. Cloud adapters follow immediately after the local
vertical slice; unavailable transports remain visible as roadmap profiles with
`unsupported` evidence, not as connected accounts.

The current experimental Codex slice is below this v1 exit condition. It permits
explicit one-project metadata-only sync and separately armed text direction for
the lifetime of the attached runtime. It imports no transcript, response, event,
file-change, cost, or approval payload, and provider acknowledgement is not
completion. Installed-provider verification covers initialize and thread list
only; mutating behavior is fake-process-tested. All other CLI profiles remain
diagnostics-only.

## Filesystem and execution boundaries

File browsing is limited to the canonical root of the selected project. Paths
are normalized, traversal and symlink escapes are rejected, binary/oversized
previews are bounded, and reading a file never grants execution authority.

The domain supports `host`, `process sandbox`, `container`, `local VM`, or
`remote VM`. The sample selector may demonstrate those profiles, but only an
implemented and observed boundary may be shown as active for a real session.
Isolation execution remains roadmap scope.

## Performance gates

- startup and first projection do not depend on network access;
- database and connector work never runs on the webview/UI thread;
- connector probes have per-process timeouts and bounded output;
- high-volume logs are paged and filtered rather than copied into one render;
- release builds use the workspace's size-conscious LTO profile;
- refresh and control failures remain isolated to their connector.

## Release gates

The candidate is acceptable only when all of the following pass from a clean
checkout:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
env -u NO_COLOR trunk build --release --locked
env -u NO_COLOR cargo tauri build --no-bundle -- --locked
```

The desktop UI must also receive a fresh native-window walkthrough at narrow
and default sizes, including keyboard navigation, reduced motion, read-only web
mode, connector failure, empty data, and persisted restart behavior. Passing
these source gates or a host-only `--no-bundle` build does not claim signed
installers, update delivery, or macOS/Windows/Linux release verification.
