# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

## Stack

Delegated Rust-first implementation: Tauri 2 for the primary desktop application, Leptos compiled to WebAssembly for the shared interface, and Rust crates for the domain model, persistence, connector supervision, telemetry, and automation. The same interface should support a browser-based status dashboard without making the web surface the product authority.

## Users

The first release serves a solo technical owner operating many AI agents across multiple projects and sessions. They need to understand current activity, intervene quickly, delegate work deliberately, and catch authentication or runtime problems without checking every tool separately.

Small-team support is a confirmed follow-on requirement. The product model must leave room for shared projects, roles, ownership, approvals, and collaborative audit history without forcing team complexity into the initial solo workflow.

## Product Purpose

Utu is a local-first operations center for AI work. It organizes local AI-agent CLIs and cloud agents into one coherent model of providers, agents, projects, sessions, tasks, costs, logs, and health. Success means the owner can answer what is running, what it is doing, what it costs, what is blocked, and where attention is needed from a single primary desktop application.

## Positioning

Utu is provider-neutral and project-centered. Unlike a provider-specific agent console, it reconciles activity from heterogeneous local and cloud tools while keeping control, credentials, and operational history under the owner's authority.

## Operating Context

- The desktop application is the primary control surface and should remain useful without a remote service.
- Local connectors supervise tools such as Codex, Claude Code, Antigravity, and other agent CLIs through supported command, process, filesystem, session, and structured-log interfaces.
- Cloud connectors will follow quickly after the local-agent foundation, including services such as Claude Work, ChatGPT Work, Cursor, and other available provider APIs or safely mediated browser sessions.
- The browser dashboard is a secondary status and oversight surface. Remote access or synchronization is opt-in.
- Owners work across many repositories and non-repository projects, with concurrent sessions and potentially several agents assigned to one task.

## Capabilities and Constraints

- Inventory providers, agents, projects, tasks, sessions, and their relationships.
- Show live or best-available agent status, current activity, project context, elapsed time, resource use, cost, health, and recent events.
- Create and manage projects and tasks; assign a task to one or several explicit agents.
- Let the owner direct, pause, resume, or stop agents when the connector and provider safely support those actions.
- Preserve agent logs and provide useful search, filtering, correlation, and failure context.
- Validate authentication and connector readiness; detect expired sessions, missing CLIs, incompatible versions, unreachable providers, malformed output, stalled work, and other actionable faults when evidence permits.
- Support agent-to-agent communication through an auditable coordination layer with explicit project/task scope.
- Keep data local by default and store credentials in the operating system keychain rather than application databases or logs.
- Offer optional execution isolation through configurable agent sandboxes and local or remote virtual machines. Isolation state and boundary failures must be visible to the owner.
- Prefer structured integrations and documented APIs. Browser mediation must be clearly labeled, permissioned, and resilient to unsupported page changes.
- Never imply that an agent is healthy, authenticated, controllable, or incurring a precise cost when the connector only has partial evidence.
- Initial collaboration is single-owner. Shared teams, roles, approvals, and hosted synchronization are planned extensions, not initial-release claims.
- Cloud-agent coverage is the highest-priority expansion immediately after the local CLI foundation; connector interfaces must support it without redesigning the core domain.

## Brand Commitments

Product name: Utu.

The interface should be clean, light, fast, and operationally calm, with the directness of Buzz or Grok Bot while remaining visibly provider-neutral rather than borrowing one vendor's identity.

The chosen visual direction is a focused native desktop workspace, executed at the craft level of Buzz, Xirp, and especially Grok Bot. It must feel like a fast installed application rather than a browser administration console: compact window chrome, a small stable app rail, one dominant working surface, content-first sessions and work, transient contextual panes, and controls that appear where the owner is acting. Avoid SaaS dashboard composition, persistent KPI strips, page-like headers, analytics-card grids, and wide management tables as the home experience.

Attention, Projects, and Fleet are three first-class views inside the same app shell, not competing product directions. The owner can switch among them without losing the active project, agent, session, selection, or command context. Attention remains the default view.

Do not introduce a metaphorical control-room, terminal, retro-computer, or clinical-triage skin. Distinctiveness must come from exceptional hierarchy, interaction, operational truth, and provider-neutral product details.

## Evidence on Hand

No production connectors, customer evidence, usage benchmarks, pricing claims, logos, or final product assets exist yet. Early interface data must be labeled as demonstration data and must not be presented as live provider evidence.

## Product Principles

1. Truth before confidence: distinguish observed, inferred, stale, unsupported, and failed states.
2. Local authority first: the desktop owner retains control of data, credentials, execution, and remote exposure.
3. Projects over providers: organize work around outcomes while preserving provider-specific evidence and controls.
4. Attention is the scarce resource: surface exceptions, blockers, and decisions without turning normal activity into noise.
5. Extensible boundaries: local CLIs, cloud services, sandboxes, VMs, and future teams join through explicit capabilities rather than special cases.

## Accessibility & Inclusion

Target WCAG 2.2 AA for the shared interface. All status distinctions must have text or shape in addition to color, dense operational views must support keyboard navigation, and motion must respect reduced-motion preferences.
