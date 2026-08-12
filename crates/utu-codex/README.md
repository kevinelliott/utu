# Utu Codex App Server adapter

This crate implements a local JSON-RPC-over-stdio client for the v2 surface
exposed by the experimental Codex App Server command. It follows the
[official protocol documentation](https://learn.chatgpt.com/docs/app-server)
and was developed against schemas generated locally by Codex CLI 0.147.0. The
initialize + bounded `thread/list` path was verified read-only against that
installed version; mutating calls and event families are fake-process
conformance-tested, not exercised against owner sessions by this gate.

Implemented APIs:

- one `initialize` request followed by the required `initialized` notification;
- cursor-based `thread/list`, read-only `thread/read`, `thread/resume`, and
  `thread/start`;
- text-only `turn/start`;
- typed projections for thread, turn, item, message-delta, lifecycle, and
  file-change notifications;
- explicit rejection of every server-initiated request. No approval is granted;
- bounded request, initialization, shutdown, message, event, stderr, pending-
  request, and event-queue resources;
- dedicated Unix process groups and bounded teardown. Windows descendant-tree
  containment still requires a Job Object supervisor in the native layer.

Security and truth boundaries:

- The adapter inherits the existing Codex CLI login. It never reads a
  credential file and never retains stdout protocol payloads in logs or stderr
  content. Provider RPC error text is always replaced with a generic detail.
- `dangerFullAccess` is denied unless `ClientConfig` explicitly opts in.
  Workspace roots must exist, canonicalize under an existing absolute cwd, and
  cannot contain parent traversal. The native layer must still map cwd to a
  registered project root and send explicit approval/sandbox policy; omitted
  values inherit Codex configuration.
- The event channel is bounded. `take_dropped_event_count() > 0` means a
  consumer must re-read the thread and then require a clean interval before
  presenting current state. It must not guess or call a lossy projection
  complete. Metadata-only clients discard payload-bearing notifications before
  projection and use explicit bounded reads as authority.
- Raw item payloads can contain owner messages, command output, and paths.
  They are native-only data and must pass the store redaction/projection
  boundary before any log or web surface.
- This crate does not implement approvals, cost normalization, credential
  management, remote transports, or browser exposure.

The read-only local check does not start or resume a thread:

```sh
cargo run -p utu-codex --example list_threads
```
