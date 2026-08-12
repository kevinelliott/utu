# Security model

Utu can launch tools that edit files and execute commands, so its security boundary must be visible and conservative.

## Defaults

- Local data and local control authority.
- No hosted relay, remote listener, cloud synchronization, or team access enabled by default.
- Credentials in the operating system keychain; credential values excluded from application databases and logs.
- Connector capabilities denied until explicitly declared and observed.
- Web status mode read-only.
- Provider payload and command logging redacted before persistence.

## Threats in scope

- a connector or agent emitting hostile or malformed output;
- prompt or log content attempting to trigger unintended controls;
- leaked API keys, browser session material, repository secrets, or personal data;
- stale evidence causing an unsafe decision;
- a compromised provider account or local CLI;
- symlink/path confusion across projects;
- unauthorized remote dashboard access;
- an agent escaping its intended filesystem, process, container, or VM boundary.

UI content is never control authority. Connector output is data, even when it resembles instructions. Every mutating action passes through a typed command path with capability, target, project scope, and owner-policy validation.

## Isolation

Sessions record one explicit execution mode: host, process sandbox, container, local VM, or remote VM. Optional sandbox and VM support is a first-class requirement, not a claim that the current foundation provides containment. Future adapters must verify the boundary is active and surface degraded or failed isolation immediately.

Recommended controls include canonical project roots, allowlisted mounts, deny-by-default network policy where supported, resource ceilings, per-session temporary directories, immutable base images, and disposable VM snapshots. Boundary changes require owner confirmation and an audit event.

## Data and retention

The local event store should encrypt sensitive fields at rest using a key protected by the OS keychain. Retention is configurable by event category. Secret-like material is redacted before storage, and raw logs have stricter limits than normalized events. Export is explicit, reviewable, and excludes credentials.

## Remote and team expansion

Remote status, cloud sync, and small-team support arrive only after local authority is stable. They require per-user identity, project roles, scoped grants, approval policy, device/session revocation, end-to-end transport security, an immutable audit history, and clear separation between viewer, operator, and administrator actions.

## Reporting issues

Do not include live credentials, session cookies, or private agent logs in an issue. Until a private reporting channel exists, disclose only sanitized reproduction details to the repository owner.
