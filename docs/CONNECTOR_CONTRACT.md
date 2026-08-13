# Connector contract

Every integration must satisfy this contract before it can appear as live or controllable in Utu.

## Descriptor and capabilities

An adapter declares its provider, version, transport, supported operating systems, discovery method, and granular capabilities. Capabilities are independent: observing a process does not imply authentication probing; reading a transcript does not imply pause or stop; token counts do not imply exact price.

The core capability set is:

- observe status and sessions;
- probe authentication;
- send owner direction;
- pause, resume, and stop;
- import logs and file changes;
- report cost evidence;
- exchange scoped agent-to-agent messages.

Unsupported capabilities are disabled and labeled, never simulated.

## Lifecycle

1. **Discover** — locate the executable, API configuration, or browser integration without reading secrets.
2. **Probe** — report reachability, compatibility, and authentication as separate evidence.
3. **Import** — normalize projects, agents, sessions, tasks, and events while retaining provider identifiers.
4. **Watch** — emit ordered, deduplicated observations with heartbeat and freshness information.
5. **Control** — validate capability and scope, request the action, then return an acknowledged, rejected, timed-out, or unknown receipt.
6. **Recover** — expose actionable failure context and a safe re-probe path.

## Evidence rules

- PATH discovery can report installation evidence only. It cannot report login state.
- A running PID can report process presence only. It cannot prove useful work or provider reachability.
- Authentication is confirmed only through a provider-supported non-destructive probe with a bounded freshness window.
- Cost includes currency, integer micros, source, period, and confidence: exact, estimated, partial, or unknown.
- Missing telemetry is unknown, not zero.
- Browser-mediated evidence is labeled as such and becomes stale when page structure or session state cannot be confirmed.

## Events

Normalized events carry a session ID, monotonically increasing local sequence, event time, ingestion time, kind, summary, evidence kind, source, and optional provider/correlation IDs. Raw provider payloads are retained only when useful, redacted, and permitted by the retention policy.

Adapters must tolerate replay. Deduplication must not discard distinct repeated commands, tool calls, or messages merely because their text matches.

## Controls and handoffs

Controls are capability-gated and scoped to one provider session. Stop, credential changes, and boundary changes require explicit owner intent. A timeout yields an unknown result until reconciled; it must not be shown as success.

Agent-to-agent communication is an Utu handoff, not an invisible side channel. Each handoff records project, task, sender, recipient, instruction, owner policy, timestamps, delivery evidence, and resulting session/event links. Cross-project handoffs are denied by default.

## Authentication and secrets

The current diagnostic adapters inherit an existing provider CLI login and run
only documented, non-destructive status commands. Utu does not copy or persist
their credential material, and raw authentication command output must not cross
the connector boundary.

A future production credential service must grant adapters short-lived access
and store secrets in the OS keychain, never in the event database, command-line
arguments, telemetry, raw logs, or the browser surface. A connector should
prefer an existing provider login and documented API over copying session
material.

Browser mediation is opt-in, visibly identified, and isolated from unrelated browser state. DOM automation is never described as an official provider API.

## Acceptance gates

A connector is not production-ready until it has tests for absent binaries/configuration, malformed output, incompatible versions, expired login, network loss, process exit, hung probes, duplicate/reordered events, partial cost data, control timeout, redaction, and restart recovery. Provider terms and permitted automation must also be reviewed.

## Implemented diagnostic profiles

The current registry covers Codex, Claude Code, Grok Build, Cursor Agent,
Antigravity, Gemini CLI, Aider, and OpenCode. All eight support deterministic
discovery where an executable contract is known; version and authentication
probes vary by verified provider surface. Codex, Claude Code, and Cursor have
direct status probes. The others keep authentication `unsupported` rather than
inspecting or guessing from credential files.

The eight registry profiles above remain diagnostics-only. A descriptor's
potential App Server or ACP support does not make it active or controllable.

The Codex App Server crate implements and hostile-tests initialization, thread
list/read/resume/start, text turn submission, typed notifications, resource
bounds, and fail-closed server-request rejection. The installed Codex CLI has
been exercised only for initialize and the read-only `thread/list` path.
Mutating requests and notification families are fake-process conformance-tested,
not exercised against owner sessions.

The native application exposes a deliberately smaller, experimental slice:

- metadata sync requires explicit confirmation for a selected local project or
  Sync for All. Sync for All discovers Claude Code and Codex session roots on
  disk, creates missing Utu projects for those directories, and imports
  metadata for every ready authenticated agent;
- after that first import, Utu watches local Claude Code and Codex session
  files and refreshes connector diagnostics on a heartbeat so the workspace
  stays current without another manual sync;
- the Codex runtime binds the exact observed executable, accepts only threads
  whose canonical cwd exactly equals that project's canonical root, and
  authorizes that project for the current attached process without revoking
  other projects;
- restart still drops the volatile Codex App Server process; the supervisor
  reconnects and re-imports metadata automatically;
- sync imports session/thread metadata only. Transcript bodies, agent
  responses, notification payloads, provider events, file changes, costs, and
  approval requests are discarded or not requested;
- every provider-bound text direction needs a separate one-shot owner
  confirmation and requests provider read-only, no-network, and `Never`
  approval policies. These are requested provider settings, not verified host
  sandbox or VM containment;
- a `turn/start` response is an acknowledgement of acceptance, not evidence of
  turn or task completion. Timeout or ambiguous failure remains unconfirmed.

Approvals, response/event projection, costs, lifecycle controls, agent-to-agent
delivery, cloud transports, and host isolation remain unsupported. Structured
streams and ACP outside this bounded Codex client remain roadmap metadata. See
`crates/dashboard-connectors/README.md` and
`crates/utu-codex/README.md` for exact boundaries.
