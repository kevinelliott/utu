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

Adapters receive short-lived secret access from the desktop credential service. Secrets are stored in the OS keychain, never in the event database, command-line arguments, telemetry, raw logs, or the browser surface. A connector should prefer an existing provider login and documented API over copying session material.

Browser mediation is opt-in, visibly identified, and isolated from unrelated browser state. DOM automation is never described as an official provider API.

## Acceptance gates

A connector is not production-ready until it has tests for absent binaries/configuration, malformed output, incompatible versions, expired login, network loss, process exit, hung probes, duplicate/reordered events, partial cost data, control timeout, redaction, and restart recovery. Provider terms and permitted automation must also be reviewed.
