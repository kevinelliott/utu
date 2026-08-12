---
version: 1
slug: "src-main-rs"
primary_target: "src/main.rs"
related_targets: ["styles.css"]
---

# Utu native shell surface brief

## Purpose

A fast, provider-neutral desktop workspace for one owner directing many AI agents across projects and sessions. The default surface is the active project conversation: it must make direction, plans, tool activity, permissions, files, verification, cost, and evidence understandable without leaving the work. Attention, Projects, Fleet, and Integrations provide nearby operational depth.

## Visual contract

Keep the approved native shell vocabulary in `.impeccable/mocks/app-attention.webp`, `app-projects.webp`, and `app-fleet.webp`, but make Workspace the default destination. Integrate platform window controls into compact native chrome and compose the utility rail, project/session sidebar, conversation surface, and contextual Files/Evidence inspector edge to edge with structural dividers. Workspace, Attention, Projects, Fleet, and Integrations switch in place without a rounded website frame, SaaS page headers, KPI strips, analytics cards, or management-dashboard composition.

The labeled demonstration conversation should combine owner and agent messages
with compact, inspectable plan, tool-call, permission, diff, test, handoff, and
streaming states. A live composer must appear only when an explicit displayed
session has a verified control capability; otherwise it stays disabled. The
composer keeps multi-agent assignment, provider-neutral model routing, context
scope, and future isolation close to the prompt. Files and connector details
are contextual panes, not competing pages or decorative card grids.

Project and task creation use focused native-style sheets over this same window.
The project sheet requires a short name and an absolute local folder path,
explains that the native layer canonicalizes and verifies the folder, preserves
input on errors, and never implies that files are uploaded. It opens from both
the Workspace project rail and the first-run empty state. The Projects work
surface groups persisted tasks under their projects and offers task creation at
the selected-project and per-project levels. The task sheet supports an
optional brief plus multi-select assignment from stored agents only; an empty
agent catalog produces an honest unassigned draft instead of sample choices.
Both sheets disable duplicate submission, expose inline native-command errors,
close only after a record is created, refresh the local snapshot, and keep the
new project selected.

The tone is light, calm, compact, and craft-forward, inspired by the interaction density of Buzz/Xirp/Grok Bot without copying provider branding. Green is reserved for primary action and healthy/live state; amber and red identify evidence-backed attention or failure.

Chrome-light refinements shipped: the workspace back/forward nav renders without a border box; the session context bar and inline tool/plan/diff cards use transparent backgrounds (border provides all needed separation); conversation turns breathe at 22px vertical padding; section-label weight and tracking are reduced (600 / 0.045em); all structural dividers use `color-mix` softening (55–70% of `--line`) to reduce visual noise without losing spatial clarity. These reinforce the Grok/Buzz-like feel already in the surface brief.

## Asset decisions

The canonical provider-neutral app mark is `static/favicon.svg`; its inline UI version must match that geometry. Tauri raster derivatives are generated from the same master. All functional icons are inline SVG paths. Agent avatars use CSS initials. Status meaning uses code-native shape plus text. Do not ship provider logos, raster mockup slices, illustration, photography, texture, or decorative assets.

## Responsive behavior

Desktop is primary. At narrow widths, preserve the main work surface, convert
the utility rail to a bottom bar, make the contextual rail transient, hide
secondary density, and keep touch targets usable. The current browser build is
a read-only, demonstration-only shell and is visually identical where
presentation states overlap.

## Truth and accessibility

Demo data is labeled. Observed, inferred, stale, unsupported, and failed states remain distinct. All status meaning has text or shape in addition to color. Controls use native buttons, keyboard focus remains visible, motion respects reduced-motion preferences, and the target is WCAG 2.2 AA.
