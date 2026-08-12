# Security model

Utu can launch tools that edit files and execute commands, so its security boundary must be visible and conservative.

## Defaults

- Local data and local control authority.
- No hosted relay, remote listener, cloud synchronization, or team access enabled by default.
- Credential persistence is not implemented. Existing provider logins are queried only through supported status commands; credential values are excluded from application databases and diagnostic evidence.
- Connector capabilities denied until explicitly declared and observed.
- Browser mode is a labeled, read-only demonstration with no desktop-database
  transport.
- Persisted diagnostic and control evidence is bounded and redacted; Codex
  transcript, response, and event payloads are not persisted by this slice.
- Codex live delivery denied unless the owner has explicitly synchronized one
  canonical project in the current process and separately arms that direction.

## Threats in scope

- a connector or agent emitting hostile or malformed output;
- prompt or log content attempting to trigger unintended controls;
- leaked API keys, browser session material, repository secrets, or personal data;
- stale evidence causing an unsafe decision;
- a compromised provider account or local CLI;
- symlink/path confusion across projects;
- unauthorized remote dashboard access;
- an agent escaping its intended filesystem, process, container, or VM boundary.

UI content is never control authority. Connector output is data, even when it
resembles instructions. Current local mutations pass through typed native
commands and store-level relation validation. Ordinary CLI profiles are
diagnostics-only.

The experimental Codex exception first requires a fresh ready/authenticated
diagnostic and explicit metadata sync for exactly one selected canonical project
root. Its effective direct capability is process-local and revoked on restart,
runtime loss, or any explicit connector refresh. A successful refresh also
revokes the lease because Utu cannot yet attest a stable provider-account
identity across processes. Each direction then requires a separate one-shot
owner confirmation before the local intent is
recorded and submitted. The request includes provider read-only, no-network,
and `Never` approval policies. A provider acknowledgement proves only turn
acceptance; timeout or ambiguous failure is never displayed as success.

Those provider-requested policies are not a verified host isolation boundary.
No transcript, provider response, event payload, file change, cost, or approval
request is imported by this slice. The real installed provider gate covers only
initialize and read-only thread listing; mutating paths use a fake-process
conformance harness.

## Isolation

The domain defines explicit execution modes—host, process sandbox, container,
local VM, and remote VM—but the current durable `Session` record does not yet
claim or enforce one. Adding observed boundary state to sessions and the UI is
a prerequisite for optional sandbox/VM execution. Future adapters must verify
the selected boundary is active and surface degraded or failed isolation
immediately.

Recommended controls include canonical project roots, allowlisted mounts, deny-by-default network policy where supported, resource ceilings, per-session temporary directories, immutable base images, and disposable VM snapshots. Boundary changes require owner confirmation and an audit event.

## Data and retention

The local database is placed in the platform application-data directory; on Unix its directory and primary database file are owner-only. It is not yet field-encrypted. Connector command evidence is control-character stripped, credential-shaped values are redacted, and output is bounded before serialization. Keychain-backed field encryption, configurable retention, reviewable export, and category-specific raw-log limits remain required before sensitive transcript import is enabled.

## Remote and team expansion

Remote status, cloud sync, and small-team support arrive only after local authority is stable. They require per-user identity, project roles, scoped grants, approval policy, device/session revocation, end-to-end transport security, an immutable audit history, and clear separation between viewer, operator, and administrator actions.

## Reporting issues

Do not include live credentials, session cookies, or private agent logs in an issue. Until a private reporting channel exists, disclose only sanitized reproduction details to the repository owner.
