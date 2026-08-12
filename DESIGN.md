---
name: Utu
description: A calm, compact, provider-neutral workbench for operating AI work.
colors:
  chrome-top: "#edf2ef"
  chrome-bottom: "#e7edeb"
  surface: "#fcfdfc"
  surface-soft: "#f4f7f6"
  surface-raised: "#ffffff"
  ink: "#18221f"
  ink-soft: "#394640"
  muted: "#65716c"
  quiet: "#5f6b66"
  line: "#dce3e0"
  line-strong: "#cbd5d1"
  green: "#1f7a5a"
  green-strong: "#126348"
  green-soft: "#e7f2ed"
  amber: "#875515"
  amber-soft: "#f7edda"
  coral: "#c34e42"
  coral-soft: "#fae9e6"
  blue-soft: "#e8f0f6"
  selection: "#e5f1ec"
  focus: "#167657"
  native-control: "#f7f9f8"
  native-control-hover: "#ffffff"
  native-control-pressed: "#e9efec"
  segmented-field: "#e8eeeb"
  rail-field: "#edf2f0"
  context-field: "#f5f8f7"
  toolbar-field: "#fbfcfb"
  evidence-inferred: "#315e7d"
  evidence-stale: "#7a510f"
  attention-count: "#86560e"
  read-only-text: "#46514d"
  toast-ink: "#1e2a26"
  blue: "#376c91"
  violet: "#68548f"
  agent-ink: "#25312d"
  avatar-coral: "#f0b4a8"
  avatar-blue: "#b8d3e6"
  avatar-lime: "#d7df9a"
  avatar-violet: "#cec1e4"
  avatar-rose: "#e9bdc9"
  avatar-amber: "#edcc94"
  avatar-teal: "#abd8cc"
  avatar-aqua: "#aad9dc"
  avatar-sand: "#ded2b3"
  avatar-navy: "#b8c4dd"
  avatar-ink: "#cad1ce"
  avatar-purple: "#d2b8dc"
  mark-deep: "#183a31"
  mark-light: "#eff6f2"
typography:
  headline:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "19px"
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: "-0.02em"
  title:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "15px"
    fontWeight: 700
    lineHeight: 1.35
    letterSpacing: "-0.012em"
  body:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "11.5px"
    fontWeight: 400
    lineHeight: 1.52
    letterSpacing: "normal"
  label:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "10.5px"
    fontWeight: 700
    lineHeight: 1.3
    letterSpacing: "0.075em"
  control:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "11px"
    fontWeight: 690
    letterSpacing: "normal"
  micro-avatar:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "6.5px"
    fontWeight: 760
    letterSpacing: "-0.03em"
  micro:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "8px"
    fontWeight: 600
  caption:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "8.5px"
    fontWeight: 600
  product:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "13px"
    fontWeight: 760
  compact-title:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "14px"
    fontWeight: 700
  responsive-title:
    fontFamily: "ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI Variable, Segoe UI, sans-serif"
    fontSize: "16px"
    fontWeight: 700
  mono:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "9.5px"
    fontWeight: 400
    lineHeight: 1.45
    letterSpacing: "normal"
rounded:
  segmented: "5px"
  badge: "6px"
  control: "7px"
  sm: "8px"
  action: "9px"
  row: "10px"
  tray: "11px"
  md: "12px"
  composer: "13px"
  overlay: "14px"
  lg: "18px"
  pill: "999px"
components:
  button-primary:
    backgroundColor: "{colors.green}"
    textColor: "{colors.surface-raised}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "32px"
  button-primary-hover:
    backgroundColor: "{colors.green-strong}"
    textColor: "{colors.surface-raised}"
  button-secondary:
    backgroundColor: "{colors.native-control}"
    textColor: "{colors.ink-soft}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "32px"
  rail-button:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.sm}"
    width: "38px"
    height: "38px"
  rail-button-active:
    backgroundColor: "color-mix(in srgb, #e5f1ec 88%, white)"
    textColor: "{colors.green-strong}"
    rounded: "{rounded.sm}"
    width: "38px"
    height: "38px"
  evidence-observed:
    backgroundColor: "{colors.green-soft}"
    textColor: "{colors.green-strong}"
    rounded: "{rounded.badge}"
    padding: "2px 6px"
    height: "19px"
  evidence-stale:
    backgroundColor: "{colors.amber-soft}"
    textColor: "{colors.evidence-stale}"
    rounded: "{rounded.badge}"
    padding: "2px 6px"
    height: "19px"
  context-row-selected:
    backgroundColor: "{colors.selection}"
    textColor: "{colors.ink-soft}"
    rounded: "{rounded.control}"
    padding: "7px 8px"
  composer:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.composer}"
    padding: "9px 10px 8px"
---

# Design System: Utu

## Overview

**Creative North Star: "The Owner's Workbench"**

The Owner's Workbench is a focused native workspace: light, calm, compact, direct, and operationally truthful. It feels app-native without borrowing a provider's identity, and it lets the owner move quickly among attention, projects, agents, sessions, and evidence without turning the work into a management report.

The system is restrained, precise, and locally tactile. One edge-to-edge work surface holds the task at hand; a stable utility rail and contextual rail establish place, while inspectors and toasts appear only when the owner needs more depth. The titlebar, rails, toolbar, and content read as one continuous desktop window rather than a web page mounted inside a decorative frame. Hierarchy, evidence language, and small responsive state changes create character without a control-room skin or decorative noise.

**Key Characteristics:**

- Compact native-workspace density with a clear 9–19px type hierarchy.
- Provider-neutral identity anchored by the canonical geometric app mark.
- Pale mineral surfaces, deep spruce ink, and scarce semantic color.
- Edge-to-edge structural panes with transient contextual depth.
- Evidence-aware states that pair color with text or shape.
- Restrained motion, visible keyboard focus, and reduced-motion parity.

## Colors

The palette is a pale mineral field with spruce-green authority, warm evidence colors, and quiet blue only for inference.

### Primary

- **Owner Green** (`green`): Primary actions and evidence-backed healthy or live state.
- **Deep Owner Green** (`green-strong`): Hover emphasis, active labels, and high-contrast semantic copy.
- **Pale Owner Green** (`green-soft`): Observed badges, healthy task glyphs, and low-intensity live surfaces.
- **Selection Wash** (`selection`): Selected context rows and active work without a card-like lift.
- **Focus Green** (`focus`): The source color for visible keyboard focus rings.

### Secondary

- **Evidence Amber** (`amber`): Waiting, stale, or external attention that has supporting evidence.
- **Pale Evidence Amber** (`amber-soft`): The paired low-intensity field for amber labels and counts.

### Tertiary

- **Failure Coral** (`coral`): Failed, blocked, destructive, or problem states.
- **Pale Failure Coral** (`coral-soft`): Diff removal and low-intensity failure fields.
- **Inference Blue Mist** (`blue-soft`): The background for explicitly inferred evidence.

### Neutral

- **Mist Chrome** (`chrome-top`) and **Blue-Gray Chrome** (`chrome-bottom`): The subtle native-window surround.
- **Work Surface** (`surface`): The dominant inset operating plane.
- **Soft Field** (`surface-soft`): Recessed controls, log fields, chips, and low-emphasis structure.
- **Raised White** (`surface-raised`): Controls, composer, and bounded content that needs local separation.
- **Deep Spruce Ink** (`ink`) and **Soft Spruce Ink** (`ink-soft`): Primary and supporting text.
- **Operational Gray** (`muted`) and **Quiet Gray** (`quiet`): Secondary labels, timestamps, and tertiary evidence; both remain readable on their relevant surfaces.
- **Fine Divider** (`line`) and **Strong Divider** (`line-strong`): Structural separation without card proliferation.
- **Mark Deep** (`mark-deep`) and **Mark Light** (`mark-light`): The fixed two-color identity of the canonical app mark.

### Named Rules

**The Scarce Green Rule.** Use green only for primary action, selected context, and evidence-backed healthy or live state; its rarity preserves operational signal.

**The Truth Is Redundant Rule.** Every status pairs color with text or shape, and observed, inferred, stale, unsupported, and failed states remain distinct.

## Typography

**Display Font:** None; this product does not use marketing display type.
**Body Font:** Platform system UI sans (`ui-sans-serif` with Apple system and Segoe UI Variable/Segoe UI fallbacks)
**Label/Mono Font:** System UI sans for labels; system monospace (`ui-monospace`, SFMono-Regular, Menlo, Consolas) for logs, diffs, commands, and tool activity

**Character:** The typography is compact and plainspoken, using weight and small spacing changes instead of font-family theatrics. Negative tracking tightens work-surface headings; uppercase labels provide quiet structure at reduced weight (600) and tighter tracking (0.045em) so they guide the eye without competing with content; monospace appears only where the content is genuinely machine-shaped.

### Hierarchy

- **Headline** (700, 19px, 1.2): Workspace titles and the highest local emphasis.
- **Title** (700, 15px, 1.35): Event and task titles inside the work stream.
- **Body** (400, 11.5px, 1.52): Operational explanations and owner/agent messages, generally limited to about 74 characters per line in streams.
- **Label** (700, 10.5px, 0.075em, uppercase): Section labels and inspector taxonomy.
- **Control** (690, 11px): Compact buttons and local actions.
- **Mono** (400, 9.5px, 1.45): Logs, commands, diffs, and trace-like evidence.

### Named Rules

**The Density Without Squinting Rule.** Use small type to support fast scanning, never to erase hierarchy or contrast; operational body copy remains readable and secondary copy still meets WCAG AA.

## Layout

The desktop shell is a full-height installed workspace with a 52px integrated titlebar and a four-column operating frame: 56px utility rail, 252px contextual rail, flexible work surface, and an optional 326px inspector. On macOS the native traffic lights sit inside the overlay titlebar; drag regions occupy only non-interactive chrome. Other platforms retain their native window controls above the same compact application toolbar. The work surface is the visual center. Streams use dividers and aligned bands rather than grids of summary cards; work toolbars are 56px high, controls stay close to their target, and reading lines remain compact.

At 1180px and below, the inspector overlays the right edge and the standing rails tighten to 54px and 230px. At 820px and below, the utility rail becomes a 54px bottom bar, the contextual rail becomes a transient drawer, the work surface occupies the first row, and secondary toolbar density is removed. At 480px and below, labels and metadata reduce further while essential state shapes and primary actions remain. The native window targets a minimum useful size of 880×620px; the current web demonstration shell keeps the same visual language, labels all sample state, and disables owner-only actions. A live web status projection remains planned.

**The One Window Rule.** Compose Utu edge to edge as one installed window. Rails, work surface, and standing inspector meet on structural dividers; do not float a rounded website shell over a page-like background.

## Elevation & Depth

Depth is structural and flat at rest. The standing desktop grid uses borders, tonal fields, and adjacency without a shell shadow or raised central page. The composer may use a small local shadow because it is an active control. The inspector gains stronger elevation only when it becomes an overlay; the toast is the darkest and highest transient element.

### Shadow Vocabulary

- **Composer** (`0 6px 20px rgb(24 46 37 / 5%)`): Local separation for the active direction field.
- **Overlay Inspector** (`0 20px 50px rgb(24 46 37 / 18%)`): Used only when the inspector leaves the desktop grid below 1180px.
- **Toast** (`0 14px 34px rgb(14 29 23 / 24%)`): Strongest transient feedback layer.

### Named Rules

**The Flat-at-Rest Rule.** Standing panes have no elevation. Reserve shadows for the active composer, overlay inspector, and toast.

## Shapes

The form language uses gently curved local controls and circular identity/state elements inside square structural panes. Segmented controls use 5px corners, badges 6px, ordinary controls 7–9px, grouped task/composer surfaces 11–13px, and transient overlays 14px. The window grid itself uses square joins. One-pixel borders define structure; 999px pills are reserved for counts and compact evidence labels. The app mark is a 64×64 rounded square with a 16-unit corner and an exact bar-and-column glyph.

**The Local Radius Rule.** Round interactive objects, never the standing window composition. Use the documented purpose-specific radius rather than an arbitrary new value.

## Components

### Buttons

Buttons are compact, quiet, and immediate.

- **Shape:** Locally rounded controls (typically 7px) with a 32px minimum height; icon-only rail controls use a 38px square and 8px corners.
- **Primary:** Owner Green with Raised White text and compact horizontal padding.
- **Hover / Focus:** Primary actions deepen to Deep Owner Green; all buttons expose a 2px visible focus outline with a 2px offset. Disabled actions retain their shape and become visibly unavailable at 0.46 opacity.
- **Secondary / Outline:** Native Control over a Fine Divider border; green outlines mark high-value but non-filled action, and coral outlines identify destructive controls. Hover brightens the field; press darkens it.

### Chips

Chips are evidence or context, never decoration.

- **Style:** Compact 6–7px corners, low-intensity semantic backgrounds, and short high-contrast labels.
- **State:** Evidence chips name the evidence class in text. Context chips truncate to one line and remain subordinate to the composer.

### Cards / Containers

Containers feel structural rather than collectible.

- **Corner Style:** 12px for selected or expanded task groups; square joins for the shell, rails, work surface, and standing inspector.
- **Background:** Work Surface for the main plane; inline content blocks (execution plan, tool call, diff) use a transparent background so the border alone provides separation. Raised White for the composer and controls that need local elevation. Soft Field for recessed log/chip evidence.
- **Shadow Strategy:** Borders and tonal layering at rest; use the documented shadows only at their named depth.
- **Border:** Fine Divider for ordinary separation; inline content cards and conversation-stream separators use `color-mix(in srgb, var(--line) 60–65%, transparent)` to keep density light; green-mixed borders only for the current selection.
- **Internal Padding:** Compact 8–16px increments observed across rows, streams, and inspectors; conversation turns use 22px vertical padding for breathing room.

### Inputs / Fields

The composer is a locally elevated work control, not a form card.

- **Style:** Raised White, a Strong Divider border, 13px corners, and a transparent textarea.
- **Focus:** The containing surface receives a green border and a 3px pale-green ring so focus reads around the complete control.
- **Error / Disabled:** Read-only uses a quiet neutral field and disables action controls; it never pretends that a command was sent.

### Navigation

The utility rail uses icon-and-shape navigation with text supplied accessibly, while the contextual three-way switch uses compact labeled tabs. Active state combines a pale selection field with Deep Owner Green. Session tabs use a 2px green underline rather than another filled control. At narrow widths, the utility rail moves intact to the bottom and the contextual rail becomes transient.

The workspace back/forward nav (`.workspace-nav`) is rendered without a border or background pill; the individual icon-buttons retain their hover state, keeping the region interactive without adding chrome. The three-way view switch (`.view-switch`) uses a semi-transparent Soft Field background so it reads as structure without adding visual weight against the work surface.

### App Mark

The canonical mark is `static/favicon.svg`: a Mark Deep rounded square containing the exact Mark Light geometry `M18 19h28v8H18zm0 14h18v12H18zm24 0h4v12h-4z`. Inline UI instances must use the same 64×64 view box, 16-unit corner radius, colors, and path.

### Evidence State

Evidence badges, state labels, status dots, and plain-language copy work together. Healthy, attention, problem, quiet, observed, inferred, stale, unknown, and read-only are not interchangeable states, and a color alone never carries their meaning.

### Named Rules

**The Local Tactility Rule.** Controls respond where the owner is acting through restrained tonal change, border emphasis, and visible focus rather than decorative motion.

## Do's and Don'ts

### Do:

- **Do** keep Attention, Projects, and Fleet inside the same stable shell and preserve selection and command context when switching.
- **Do** label demo, read-only, observed, inferred, stale, unsupported, failed, and partial-evidence states honestly.
- **Do** use the canonical app mark and code-native SVG icons; keep provider identity in text rather than brand logos.
- **Do** keep Quiet Gray and Evidence Amber above 4.5:1 on every relevant surface, preserve visible keyboard focus, and respect reduced motion.
- **Do** preserve text or shape alongside every semantic color.

### Don't:

- **Don't** compose the home experience as a SaaS admin dashboard, KPI strip, analytics-card grid, or wide management table.
- **Don't** turn the shell into a retro terminal, control room, clinical triage surface, or provider-branded console.
- **Don't** scatter shadows across ordinary rows or promote every bounded group into a card.
- **Don't** use green as general decoration or collapse different evidence qualities into one healthy-looking state.
- **Don't** imply that read-only or partially evidenced controls have live authority.
