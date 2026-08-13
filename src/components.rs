use leptos::prelude::*;

pub const ICON_HOME: &str =
    "M3 10.5 12 3l9 7.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 19.5zm6 10v-6h6v6";
pub const ICON_ATTENTION: &str = "M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4";
pub const ICON_FOLDER: &str = "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z";
pub const ICON_BRANCH: &str = "M6 3v12a4 4 0 0 0 4 4h5m0 0-3-3m3 3-3 3M18 3v4a4 4 0 0 1-4 4H6";
pub const ICON_TERMINAL: &str = "m4 6 5 5-5 5m8 0h8";
pub const ICON_LOCK: &str = "M6 10h12v11H6zm3 0V7a3 3 0 0 1 6 0v3";
pub const ICON_CHEVRON_RIGHT: &str = "m9 18 6-6-6-6";
pub const ICON_REFRESH: &str = "M20 7v5h-5M4 17v-5h5m9.6-3A8 8 0 0 0 5 7m.4 8A8 8 0 0 0 19 17";
pub const ICON_CLOUD: &str = "M7 18h11a4 4 0 0 0 .7-7.9A7 7 0 0 0 5.4 8.4 4.8 4.8 0 0 0 7 18z";
pub const ICON_SEARCH: &str = "m21 21-4.35-4.35m2.35-5.65a8 8 0 1 1-16 0 8 8 0 0 1 16 0";
pub const ICON_NODES: &str =
    "M8 6h8M8 18h8M6 8v8m12-8v8M6 3a3 3 0 1 0 0 6 3 3 0 0 0 0-6m12 12a3 3 0 1 0 0 6 3 3 0 0 0 0-6";
pub const ICON_PLUG: &str = "M8 3v4m8-4v4M6 7h12v3a6 6 0 0 1-6 6v5m-3 0h6";
pub const ICON_COST: &str = "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20m3 7c-.5-1-1.5-1.5-3-1.5-1.7 0-3 .9-3 2.2 0 3.3 6 1.5 6 4.6 0 1.3-1.3 2.2-3 2.2-1.5 0-2.7-.5-3.4-1.5M12 5v14";
pub const ICON_SETTINGS: &str = "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7m7.4 3.5 1.3-2.2-2-3.4h-2.6L14.8 4h-4L9.5 6.4H6.9l-2 3.4L6.2 12l-1.3 2.2 2 3.4h2.6l1.3 2.4h4l1.3-2.4h2.6l2-3.4z";
pub const ICON_FILTER: &str = "M4 5h16l-6.5 7.2V19l-3 1v-7.8z";
pub const ICON_PLUS: &str = "M12 5v14M5 12h14";
pub const ICON_SEND: &str = "m4 4 17 8-17 8 3-8zm3 8h14";
pub const ICON_CLOSE: &str = "m6 6 12 12M18 6 6 18";
pub const ICON_PAUSE: &str = "M8 5v14m8-14v14";
pub const ICON_STOP: &str = "M7 7h10v10H7z";
pub const ICON_MORE: &str = "M5 12h.01M12 12h.01M19 12h.01";
pub const ICON_ARROW: &str = "m9 18 6-6-6-6";
pub const ICON_CHECK: &str = "m5 12 4 4L19 6";
pub const ICON_FILE: &str = "M6 2h8l4 4v16H6zm8 0v6h6M9 13h6m-6 4h6";
pub const ICON_SHIELD: &str = "M12 3 20 6v6c0 5-3.5 8-8 10-4.5-2-8-5-8-10V6z";
pub const ICON_COMMAND: &str =
    "M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z";
/// Radial/orbit icon for the Overview view.
pub const ICON_ORBIT: &str =
    "M12 12m-2 0a2 2 0 1 0 4 0 2 2 0 1 0-4 0M5.2 5.2l2.1 2.1M16.7 16.7l2.1 2.1M5.2 18.8l2.1-2.1M16.7 7.3l2.1-2.1M12 3v2M12 19v2M3 12h2M19 12h2";

#[component]
pub fn AppMarkGlyph() -> impl IntoView {
    view! {
        <svg viewBox="0 0 64 64" aria-hidden="true" focusable="false">
            <rect width="64" height="64" rx="16" fill="#183a31" />
            <path d="M18 19h28v8H18zm0 14h18v12H18zm24 0h4v12h-4z" fill="#eff6f2" />
        </svg>
    }
}

#[component]
pub fn Icon(#[prop(into)] path: String) -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
            <path d=path />
        </svg>
    }
}

#[component]
pub fn AgentAvatar(
    initials: &'static str,
    tone: &'static str,
    #[prop(default = "md")] size: &'static str,
) -> impl IntoView {
    view! {
        <span class=format!("agent-avatar avatar-{tone} avatar-{size}") aria-hidden="true">
            {initials}
        </span>
    }
}

#[component]
pub fn StatusDot(#[prop(default = "healthy")] tone: &'static str) -> impl IntoView {
    view! { <span class=format!("status-dot status-{tone}") aria-hidden="true"></span> }
}

#[component]
pub fn DemoBadge(#[prop(default = false)] web: bool) -> impl IntoView {
    view! {
        <span class="demo-badge">
            <StatusDot tone="attention" />
            {if web { "Web status · Demo" } else { "Demo workspace" }}
        </span>
    }
}
#[component]
pub fn Composer(
    placeholder: &'static str,
    context: &'static str,
    agent: &'static str,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    let submit = move |_| {
        let message = if read_only {
            "The web status surface is read-only. Open Utu on the owner device to direct an agent."
        } else {
            "Direction staged in this prototype. A live connector will deliver it after explicit confirmation."
        };
        notice.set(Some(message.into()));
    };

    view! {
        <div class="composer" class:is-read-only=read_only>
            <label class="sr-only" for="agent-direction">{placeholder}</label>
            <textarea id="agent-direction" rows="1" placeholder=placeholder readonly=read_only></textarea>
            <div class="composer-toolbar">
                <div class="composer-context">
                    <button type="button" class="context-chip" disabled=read_only>{context}</button>
                    <button type="button" class="context-chip" disabled=read_only>{agent}</button>
                    <button type="button" class="icon-button mini" disabled=read_only aria-label="Add context"><Icon path=ICON_PLUS /></button>
                </div>
                <button type="button" class="send-button" disabled=read_only on:click=submit aria-label="Send direction">
                    <Icon path=ICON_SEND />
                </button>
            </div>
        </div>
    }
}

#[component]
pub fn EvidenceTag(kind: &'static str) -> impl IntoView {
    let tone = match kind {
        "Observed" => "observed",
        "Inferred" => "inferred",
        "Stale" => "stale",
        _ => "unknown",
    };
    view! { <span class=format!("evidence-tag evidence-{tone}")>{kind}</span> }
}

#[component]
pub fn EmptyInspectorButton(inspector_open: RwSignal<bool>) -> impl IntoView {
    view! {
        <button class="icon-button" type="button" aria-label="Open details" on:click=move |_| inspector_open.set(true)>
            <Icon path=ICON_ARROW />
        </button>
    }
}

/// Returns the background CSS color for a known connector.
pub fn connector_bg_color(connector_id: &str) -> &'static str {
    match connector_id {
        "claude" | "claude-code" | "claude-sessions" | "claude-agent" => "#d07040",
        "codex" | "codex-cli" => "#5b84c4",
        "cursor" | "cursor-agent" | "cursor-sessions" => "#2d2d2d",
        "gemini" => "#4285f4",
        "aider" => "#e74c3c",
        "opencode" => "#7c5cbf",
        "grok" => "#5c6370",
        "antigravity" => "#3a7d5c",
        _ => "#65716c",
    }
}

pub fn connector_initials(connector_id: &str) -> &'static str {
    match connector_id {
        "claude" | "claude-code" | "claude-sessions" | "claude-agent" => "CL",
        "codex" | "codex-cli" => "CO",
        "cursor" | "cursor-agent" | "cursor-sessions" => "CU",
        "gemini" => "Ge",
        "aider" => "Ai",
        "opencode" => "OC",
        "grok" => "GR",
        "antigravity" => "AG",
        _ => "?",
    }
}

pub fn connector_display_name(connector_id: &str) -> &'static str {
    match connector_id {
        "claude" | "claude-code" | "claude-sessions" | "claude-agent" => "Claude Code",
        "codex" | "codex-cli" => "Codex",
        "cursor" | "cursor-agent" | "cursor-sessions" => "Cursor Agent",
        "gemini" => "Gemini CLI",
        "aider" => "Aider",
        "opencode" => "OpenCode",
        "grok" => "Grok Build",
        "antigravity" => "Antigravity",
        _ => "Agent",
    }
}

/// Rows above/below the visible viewport that are still mounted as a buffer.
const VIRTUAL_OVERSCAN: usize = 6;

/// A fixed-height virtual scroll container.
///
/// Only rows within the visible viewport plus `VIRTUAL_OVERSCAN` rows on each
/// side are mounted in the DOM. Top and bottom spacer divs maintain the correct
/// scroll track height so the scroll thumb is always accurate.
///
/// `row_height` must match the rendered height of every item (px).  Group
/// headers rendered as list items should be given the same height; the extra
/// blank space is intentional and keeps the math simple.
#[component]
pub fn VirtualList<T, F, V>(
    /// Full item list, re-evaluated on every scroll.
    items: impl Fn() -> Vec<T> + Send + Sync + 'static,
    /// Height of every row in pixels (uniform).
    #[prop(default = 52.0_f64)] row_height: f64,
    /// Renders one item into a view.
    render_item: F,
) -> impl IntoView
where
    T: Clone + Send + Sync + 'static,
    F: Fn(T) -> V + Clone + Send + Sync + 'static,
    V: IntoView + 'static,
{
    let scroll_top = RwSignal::new(0.0f64);

    view! {
        <div
            class="virtual-list-scroll"
            on:scroll=move |ev| {
                use wasm_bindgen::JsCast;
                if let Some(t) = ev.current_target() {
                    scroll_top.set(t.unchecked_into::<web_sys::Element>().scroll_top() as f64);
                }
            }
        >
            {move || {
                let all_items = items();
                let total = all_items.len();
                let top = scroll_top.get();
                // Estimate viewport height from the browser window; fall back to 700 px
                // (generous enough to cover any normal Utu desktop window).
                let vp = web_sys::window()
                    .and_then(|w| w.inner_height().ok())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(700.0);
                let start = ((top / row_height) as usize).saturating_sub(VIRTUAL_OVERSCAN);
                let end = (((top + vp) / row_height).ceil() as usize + VIRTUAL_OVERSCAN).min(total);
                let top_px  = start as f64 * row_height;
                let bot_px  = total.saturating_sub(end) as f64 * row_height;
                let render  = render_item.clone();
                view! {
                    <div style=format!("height:{top_px}px;flex-shrink:0")></div>
                    {all_items.into_iter().skip(start).take(end - start).map(move |item| render(item)).collect_view()}
                    <div style=format!("height:{bot_px}px;flex-shrink:0")></div>
                }.into_any()
            }}
        </div>
    }
}

/// Branded icon for a known agent CLI connector.
///
/// Renders an official or official-like inline SVG logo for each known
/// connector.  All paths are bundled — no remote fetches.  Falls back to a
/// muted initials badge for unknown connectors so unknown agents never
/// silently render blank.
#[component]
pub fn AgentCliIcon(
    #[prop(into)] connector_id: String,
    #[prop(default = "sm")] size: &'static str,
) -> impl IntoView {
    let name = connector_display_name(&connector_id);
    let logo = connector_logo_svg(&connector_id);
    if let Some(svg) = logo {
        view! {
            <span
                class=format!("agent-cli-icon agent-cli-icon-logo agent-cli-icon-{size}")
                title=name
                aria-label=name
                aria-hidden="true"
                inner_html=svg
            />
        }
        .into_any()
    } else {
        let bg = connector_bg_color(&connector_id);
        let initials = connector_initials(&connector_id);
        view! {
            <span
                class=format!("agent-cli-icon agent-cli-icon-{size}")
                style=format!("background:{bg}")
                title=name
                aria-label=name
                aria-hidden="true"
            >
                {initials}
            </span>
        }
        .into_any()
    }
}

/// Returns an inline SVG string for known connectors, `None` for unknowns.
///
/// All SVGs are 16×16 and designed to be legible at 16–20 px.  Paths are
/// derived from publicly documented official brand assets; no hotlinks or
/// external assets are needed at runtime.
pub fn connector_logo_svg(connector_id: &str) -> Option<&'static str> {
    match connector_id {
        // Cursor — stylised "C" cursor-arrow composite mark
        "cursor" | "cursor-agent" | "cursor-sessions" => Some(
            r#"<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
              <rect width="16" height="16" rx="3.5" fill="#1a1a1a"/>
              <path d="M4 3h8v1.5H4V3zm0 4h5.5V8.5H4V7zm0 4h8v1.5H4V11z" fill="#ffffff"/>
              <path d="M10 7l4 4-1.5 0.5-1-2.5L10 11V7z" fill="#c792ea"/>
            </svg>"#,
        ),
        // Claude / Anthropic — asterisk / star mark
        "claude" | "claude-code" | "claude-sessions" | "claude-agent" => Some(
            r#"<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
              <rect width="16" height="16" rx="3.5" fill="#d4774a"/>
              <path d="M8 3v10M3 8h10M4.5 4.5l7 7M11.5 4.5l-7 7" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/>
            </svg>"#,
        ),
        // Codex / OpenAI — simplified blossom / swirl
        "codex" | "codex-cli" => Some(
            r#"<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
              <rect width="16" height="16" rx="3.5" fill="#4a6fa5"/>
              <path d="M8 2.5A5.5 5.5 0 1 1 8 13.5A5.5 5.5 0 0 1 8 2.5z" stroke="#fff" stroke-width="1.4" fill="none"/>
              <path d="M8 5a3 3 0 1 1 0 6 3 3 0 0 1 0-6z" fill="#fff" opacity="0.6"/>
              <circle cx="8" cy="8" r="1.2" fill="#fff"/>
            </svg>"#,
        ),
        // Gemini — Google-style diamond mark
        "gemini" => Some(
            r#"<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
              <rect width="16" height="16" rx="3.5" fill="#3367d6"/>
              <path d="M8 2.5C8 2.5 5 6 5 8s3 5.5 3 5.5 3-3.5 3-5.5-3-5.5-3-5.5z" fill="#fff"/>
              <path d="M2.5 8c0 0 3.5-3 5.5-3s5.5 3 5.5 3-3.5 3-5.5 3-5.5-3-5.5-3z" fill="#fff" opacity="0.55"/>
            </svg>"#,
        ),
        // Grok / xAI — stylised X
        "grok" => Some(
            r#"<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
              <rect width="16" height="16" rx="3.5" fill="#2d2d2d"/>
              <path d="M4 4l8 8M12 4l-8 8" stroke="#fff" stroke-width="2" stroke-linecap="round"/>
            </svg>"#,
        ),
        _ => None,
    }
}
