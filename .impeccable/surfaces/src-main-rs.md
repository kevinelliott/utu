---
version: 1
slug: "src-main-rs"
primary_target: "src/main.rs"
related_targets: ["styles.css"]
---

# Utu native shell surface brief

## Purpose

A fast, provider-neutral desktop workspace for one owner supervising many AI agents and projects. The app must immediately answer what needs attention, what work is active, what every agent is doing, and what evidence supports each state.

## Visual contract

Use the approved three-view native shell in `.impeccable/mocks/app-attention.webp`, `app-projects.webp`, and `app-fleet.webp`. Integrate platform window controls into compact native chrome and compose the utility rail, contextual sidebar, work surface, and standing inspector edge to edge with structural dividers. Attention, Projects, and Fleet switch in place without a rounded website frame, SaaS page headers, KPI strips, analytics cards, or management-dashboard composition.

The tone is light, calm, compact, and craft-forward, inspired by the interaction density of Buzz/Xirp/Grok Bot without copying provider branding. Green is reserved for primary action and healthy/live state; amber and red identify evidence-backed attention or failure.

## Asset decisions

The canonical provider-neutral app mark is `static/favicon.svg`; its inline UI version must match that geometry. Tauri raster derivatives are generated from the same master. All functional icons are inline SVG paths. Agent avatars use CSS initials. Status meaning uses code-native shape plus text. Do not ship provider logos, raster mockup slices, illustration, photography, texture, or decorative assets.

## Responsive behavior

Desktop is primary. At narrow widths, preserve the main work surface, convert the utility rail to a bottom bar, make the contextual rail transient, hide secondary density, and keep touch targets usable. The browser status surface is read-only and visually identical where capabilities overlap.

## Truth and accessibility

Demo data is labeled. Observed, inferred, stale, unsupported, and failed states remain distinct. All status meaning has text or shape in addition to color. Controls use native buttons, keyboard focus remains visible, motion respects reduced-motion preferences, and the target is WCAG 2.2 AA.
