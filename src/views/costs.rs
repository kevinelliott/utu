use leptos::prelude::*;

use crate::{
    ipc::{CostAmount, ProjectCostSummary},
    workspace_data::{LiveStatus, LoadPhase},
};

#[component]
pub fn CostsView() -> impl IntoView {
    let live = expect_context::<LiveStatus>();

    view! {
        <div class="workspace-layout costs-layout">
            <header class="workspace-toolbar">
                <div class="toolbar-leading">
                    <div>
                        <h1>"Costs"</h1>
                        <p>"Token spend reported by connected agents"</p>
                    </div>
                </div>
            </header>

            <div class="costs-content">
                <Show when=move || live.phase.get() == LoadPhase::Loading>
                    <div class="costs-empty">
                        <span class="spinner"></span>
                        <strong>"Loading cost records"</strong>
                        <small>"Waiting for the native workspace snapshot."</small>
                    </div>
                </Show>

                <Show when=move || live.phase.get() == LoadPhase::Error>
                    <div class="costs-empty costs-empty-problem">
                        <strong>"Cost records unavailable"</strong>
                        <small>{move || live.error.get().unwrap_or_else(|| "Store integrity check failed.".into())}</small>
                    </div>
                </Show>

                <Show when=move || matches!(live.phase.get(), LoadPhase::Empty | LoadPhase::Ready)>
                    {move || {
                        let snapshot = live.snapshot.get();
                        let costs = snapshot.as_ref().map(|s| s.costs.clone()).unwrap_or_default();

                        if costs.is_empty() {
                            return view! {
                                <div class="costs-empty">
                                    <strong>"No cost records yet"</strong>
                                    <small>"Agents that report token usage will appear here once observed. Most CLI agents do not report costs natively — cost records require provider API integration."</small>
                                </div>
                            }.into_any();
                        }

                        let total_str = total_recorded_str(&costs);
                        let has_any_amount = costs.iter().any(|c| c.amount.micros.is_some());
                        let complete_count = costs.iter().filter(|c| c.complete).count();
                        let total_count = costs.len();
                        let projects = snapshot.as_ref().map(|s| s.projects.clone()).unwrap_or_default();

                        let cost_rows: Vec<_> = costs.into_iter().map(|summary| {
                            let project_name = projects.iter()
                                .find(|p| p.id == summary.project_id)
                                .map(|p| p.name.clone())
                                .unwrap_or_else(|| summary.project_id.clone());
                            let amount_str = format_cost_display(&summary.amount);
                            let complete = summary.complete;
                            (project_name, amount_str, complete)
                        }).collect();

                        view! {
                            <div class="costs-summary-row">
                                {if has_any_amount {
                                    view! {
                                        <div class="costs-total-card">
                                            <span class="costs-total-label">"Recorded spend"</span>
                                            <span class="costs-total-value">{total_str}</span>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {}.into_any()
                                }}
                                <div class="costs-coverage-card">
                                    <span class="costs-total-label">"Coverage"</span>
                                    <span class="costs-total-value">{format!("{complete_count}/{total_count} projects complete")}</span>
                                </div>
                            </div>

                            <div class="costs-project-list">
                                {cost_rows.into_iter().map(|(project_name, amount_str, complete)| {
                                    view! {
                                        <div class="cost-project-row">
                                            <div class="cost-project-name">
                                                <strong>{project_name}</strong>
                                                {if complete {
                                                    view! { <span class="cost-tag cost-tag-complete">"complete"</span> }.into_any()
                                                } else {
                                                    view! { <span class="cost-tag cost-tag-partial">"partial"</span> }.into_any()
                                                }}
                                            </div>
                                            <div class="cost-project-amount">{amount_str}</div>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>

                            <div class="costs-footnote">
                                <p>"Cost records are sourced from agent-reported usage data. Partial records indicate that some sessions did not report spend. No cost data is estimated or interpolated."</p>
                            </div>
                        }.into_any()
                    }}
                </Show>

                <Show when=move || !live.is_desktop()>
                    <div class="costs-empty">
                        <strong>"Web surface · read-only"</strong>
                        <small>"Cost records are available on the owner device."</small>
                    </div>
                </Show>
            </div>
        </div>
    }
}

fn format_cost_usd(amount: &CostAmount) -> Option<f64> {
    let micros = amount.micros?;
    Some(micros as f64 / 1_000_000.0)
}

fn format_cost_display(amount: &CostAmount) -> String {
    match (amount.micros, amount.confidence.as_str()) {
        (Some(micros), _) => format!("${:.4} {}", micros as f64 / 1_000_000.0, amount.currency),
        (None, "unknown") => "Not reported".into(),
        (None, conf) => format!("Unknown · {conf}"),
    }
}

fn total_recorded_str(costs: &[ProjectCostSummary]) -> String {
    let sum_micros: u64 = costs.iter().filter_map(|c| c.amount.micros).sum();
    if sum_micros == 0 {
        return "Not reported".into();
    }
    let dollars = sum_micros as f64 / 1_000_000.0;
    if dollars < 0.01 {
        "<$0.01".into()
    } else {
        format!("${dollars:.2}")
    }
}
