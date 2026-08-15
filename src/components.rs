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
pub const ICON_ORBIT: &str = "M12 12m-2 0a2 2 0 1 0 4 0 2 2 0 1 0-4 0M5.2 5.2l2.1 2.1M16.7 16.7l2.1 2.1M5.2 18.8l2.1-2.1M16.7 7.3l2.1-2.1M12 3v2M12 19v2M3 12h2M19 12h2";

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
pub fn EvidenceTag(#[prop(into)] kind: String) -> impl IntoView {
    let normalized = kind.to_ascii_lowercase();
    let (tone, label) = match normalized.as_str() {
        "observed" => ("observed", "Observed"),
        "inferred" => ("inferred", "Inferred"),
        "stale" => ("stale", "Stale"),
        "unsupported" => ("unknown", "Unsupported"),
        _ => ("unknown", "Unknown"),
    };
    view! { <span class=format!("evidence-tag evidence-{tone}")>{label}</span> }
}

/// Branded icon for a known agent CLI. Hover shows the official name.
/// Paths are inlined; no remote fetches.
#[component]
pub fn AgentCliIcon(
    #[prop(into)] connector_id: String,
    #[prop(default = "sm")] size: &'static str,
) -> impl IntoView {
    let name = connector_display_name(&connector_id);
    let (path_html, viewbox, extra_class) = agent_svg(&connector_id);
    let slug = agent_id_slug(&connector_id);
    // Inject the full SVG as innerHTML of the span. Using inner_html on the
    // outer span (an HTML element) is reliable in Leptos 0.8 WASM — the same
    // technique used for markdown bodies. Attempting inner_html on an
    // SVG-namespace element created by the view! macro does not reliably call
    // set_inner_html in the current runtime.
    let svg_markup = format!(
        r#"<svg viewBox="{viewbox}" xmlns="http://www.w3.org/2000/svg" class="agent-logo {extra_class}" focusable="false" aria-hidden="true">{path_html}</svg>"#
    );
    view! {
        <span
            class=format!("agent-cli-icon agent-cli-icon-{size} agent-icon-{slug}")
            title=name
            aria-label=name
            inner_html=svg_markup
        ></span>
    }
}

fn agent_id_slug(agent_id: &str) -> &str {
    match agent_id {
        id if id.contains("claude") => "claude",
        id if id.contains("codex") || id.contains("openai") => "openai",
        id if id.contains("cursor") => "cursor",
        id if id.contains("gemini") || id.contains("google") => "gemini",
        id if id.contains("grok") || id.contains("xai") => "grok",
        _ => "agent",
    }
}

/// Returns `(inner_svg_html, viewBox, extra_css_class)` for a given agent id.
fn agent_svg(agent_id: &str) -> (&'static str, &'static str, &'static str) {
    match agent_id_slug(agent_id) {
        // Anthropic / Claude — official brand mark from simple-icons (Anthropic asterisk/starburst).
        "claude" => (
            r#"<path fill="currentColor" d="m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z"/>"#,
            "0 0 24 24",
            "logo-claude",
        ),
        // OpenAI / Codex — official blossom mark from simple-icons.
        "openai" => (
            r#"<path fill="currentColor" d="M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z"/>"#,
            "0 0 24 24",
            "logo-openai",
        ),
        // Cursor — official hexagonal mark from simple-icons (cursor.com brand kit).
        "cursor" => (
            r#"<path fill="currentColor" d="M11.503.131 1.891 5.678a.84.84 0 0 0-.42.726v11.188c0 .3.162.575.42.724l9.609 5.55a1 1 0 0 0 .998 0l9.61-5.55a.84.84 0 0 0 .42-.724V6.404a.84.84 0 0 0-.42-.726L12.497.131a1.01 1.01 0 0 0-.996 0M2.657 6.338h18.55c.263 0 .43.287.297.515L12.23 22.918c-.062.107-.229.064-.229-.06V12.335a.59.59 0 0 0-.295-.51l-9.11-5.257c-.109-.063-.064-.23.061-.23"/>"#,
            "0 0 24 24",
            "logo-cursor",
        ),
        // Google Gemini — official 4-pointed star mark from simple-icons.
        "gemini" => (
            r#"<path fill="currentColor" d="M11.04 19.32Q12 21.51 12 24q0-2.49.93-4.68.96-2.19 2.58-3.81t3.81-2.55Q21.51 12 24 12q-2.49 0-4.68-.93a12.3 12.3 0 0 1-3.81-2.58 12.3 12.3 0 0 1-2.58-3.81Q12 2.49 12 0q0 2.49-.96 4.68-.93 2.19-2.55 3.81a12.3 12.3 0 0 1-3.81 2.58Q2.49 12 0 12q2.49 0 4.68.96 2.19.93 3.81 2.55t2.55 3.81"/>"#,
            "0 0 24 24",
            "logo-gemini",
        ),
        // xAI / Grok — official X mark from simple-icons.
        "grok" => (
            r#"<path fill="currentColor" d="M14.234 10.162 22.977 0h-2.072l-7.591 8.824L7.251 0H.258l9.168 13.343L.258 24H2.33l8.016-9.318L16.749 24h6.993zm-2.837 3.299-.929-1.329L3.076 1.56h3.182l5.965 8.532.929 1.329 7.754 11.09h-3.182z"/>"#,
            "0 0 24 24",
            "logo-grok",
        ),
        // Generic fallback — terminal prompt chevron.
        _ => (
            r#"<path d="m4 6 5 5-5 5m8 0h8" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
            "0 0 24 24",
            "logo-agent",
        ),
    }
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
        _ => "#3f4a52",
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
    #[prop(default = 52.0_f64)]
    row_height: f64,
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
