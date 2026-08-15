use leptos::prelude::*;
use utu_core::{CostAmount as CoreAmount, CostConfidence};

use crate::{
    components::{DemoBadge, EvidenceTag},
    ipc::{CostAmount, CostRecord, ProjectCostSummary, WorkspaceSnapshot},
    workspace_data::{LiveStatus, LoadPhase, relative_unix_ms},
};

#[component]
pub fn CostsView() -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let selected_project_id = RwSignal::new(None::<String>);

    view! {
        <div class="workspace-layout costs-layout">
            <header class="workspace-toolbar">
                <div class="toolbar-leading">
                    <div>
                        <h1>"Costs"</h1>
                        <p>"What Utu has recorded, and what it cannot observe"</p>
                    </div>
                </div>
            </header>

            <div class="costs-content">
                <Show when=move || live.phase.get() == LoadPhase::Loading>
                    <div class="costs-empty">
                        <span class="spinner"></span>
                        <strong>"Loading cost records"</strong>
                        <small>"Waiting for the local store snapshot."</small>
                    </div>
                </Show>

                <Show when=move || live.phase.get() == LoadPhase::Error>
                    <div class="costs-empty costs-empty-problem">
                        <strong>"Cost records unavailable"</strong>
                        <small>{move || live.error.get().unwrap_or_else(|| "Store integrity check failed.".into())}</small>
                    </div>
                </Show>

                <Show when=move || !live.is_desktop() && live.phase.get() == LoadPhase::Demo>
                    <div class="costs-empty">
                        <strong>"Costs live on the owner device"</strong>
                        <small>"This web surface cannot read local cost records. Open Utu on the owner device. CLI agents do not report provider invoices."</small>
                    </div>
                </Show>

                <Show when=move || live.is_desktop() && matches!(live.phase.get(), LoadPhase::Empty | LoadPhase::Ready)>
                    {move || {
                        let snapshot = live.snapshot.get();
                        let Some(snapshot) = snapshot else {
                            return view! {
                                <div class="costs-empty">
                                    <strong>"No workspace snapshot"</strong>
                                    <small>"Cost records appear after the local store is readable."</small>
                                </div>
                            }.into_any();
                        };

                        if snapshot.projects.is_empty() {
                            return view! {
                                <div class="costs-empty">
                                    <strong>"No projects yet"</strong>
                                    <small>"Cost records attach to projects. Add a project first. Most CLI agents still cannot report spend."</small>
                                </div>
                            }.into_any();
                        }

                        let board = CostBoard::from_snapshot(&snapshot);
                        let selected = selected_project_id
                            .get()
                            .filter(|id| board.projects.iter().any(|project| project.id == *id))
                            .or_else(|| board.projects.first().map(|project| project.id.clone()));
                        let selected_project = board
                            .projects
                            .iter()
                            .find(|project| Some(project.id.as_str()) == selected.as_deref())
                            .cloned();
                        let coverage = board.coverage.clone();
                        let has_demo = coverage.has_demo;
                        let projects = board.projects.clone();

                        view! {
                            <div class="costs-coverage" aria-label="Cost coverage">
                                <div class="costs-coverage-copy">
                                    <strong>{coverage.headline}</strong>
                                    <p>{coverage.detail}</p>
                                </div>
                                <Show when=move || has_demo>
                                    <DemoBadge />
                                </Show>
                            </div>

                            <div class="costs-board">
                                <nav class="costs-master" aria-label="Projects">
                                    <div class="section-label"><span>"Projects"</span></div>
                                    {projects.into_iter().map(|project| {
                                        let project_id = project.id.clone();
                                        let is_selected = selected.as_deref() == Some(project.id.as_str());
                                        view! {
                                            <button
                                                class=move || if is_selected { "cost-project-row is-selected" } else { "cost-project-row" }
                                                type="button"
                                                on:click=move |_| selected_project_id.set(Some(project_id.clone()))
                                            >
                                                <span class="row-copy">
                                                    <strong>{project.name.clone()}</strong>
                                                    <small>{project.coverage_line.clone()}</small>
                                                </span>
                                                <span class="cost-project-amount">
                                                    <span>{project.amount_display.clone()}</span>
                                                    <span class=confidence_class(&project.confidence)>{confidence_label(&project.confidence)}</span>
                                                </span>
                                            </button>
                                        }
                                    }).collect_view()}
                                </nav>

                                <section class="costs-detail" aria-label="Selected project cost records">
                                    {match selected_project {
                                        Some(project) => project_detail(project, snapshot.generated_at_unix_ms, &snapshot).into_any(),
                                        None => view! {
                                            <div class="costs-empty">
                                                <strong>"Select a project"</strong>
                                                <small>"Records and evidence open here."</small>
                                            </div>
                                        }.into_any(),
                                    }}
                                </section>
                            </div>
                        }.into_any()
                    }}
                </Show>
            </div>
        </div>
    }
}

fn project_detail(
    project: CostProject,
    generated_at_unix_ms: u64,
    snapshot: &WorkspaceSnapshot,
) -> impl IntoView {
    let sessions = snapshot.sessions.clone();
    let agents = snapshot.agents.clone();
    let tasks = snapshot.tasks.clone();
    let records = project.records.clone();
    let empty = records.is_empty();
    let truncated = project.truncated;
    let currency_note = if project.other_currencies.is_empty() {
        None
    } else {
        Some(format!(
            "Also recorded in {} — currencies are not converted or summed together.",
            project.other_currencies.join(", ")
        ))
    };
    let record_items = records
        .iter()
        .map(|record| {
            let amount = format_cost_amount(&record.amount);
            let confidence = record.amount.confidence.clone();
            let evidence = record.evidence.clone();
            let when = relative_unix_ms(generated_at_unix_ms, Some(record.occurred_at_unix_ms));
            let scope = record_scope(record, &sessions, &agents, &tasks);
            let note = record.note.clone();
            let source = source_label(&record.source);
            let demo = is_demo_source(&record.source);
            view! {
                <article class="cost-record">
                    <div class="cost-record-main">
                        <strong class="cost-record-amount">{amount}</strong>
                        <span class=confidence_class(&confidence)>{confidence_label(&confidence)}</span>
                        <EvidenceTag kind=evidence />
                        {if demo {
                            view! { <span class="cost-tag cost-tag-demo">"Demo"</span> }.into_any()
                        } else {
                            view! {}.into_any()
                        }}
                    </div>
                    <p class="cost-record-scope">{scope}</p>
                    <p class="cost-record-meta">{format!("{source} · {when}")}</p>
                    {note.map(|note| view! { <p class="cost-record-note">{note}</p> })}
                </article>
            }
        })
        .collect_view();

    view! {
        <header class="costs-detail-header">
            <div>
                <h2>{project.name.clone()}</h2>
                <p>{project.detail_line.clone()}</p>
            </div>
            <div class="costs-detail-amount">
                <strong>{project.amount_display.clone()}</strong>
                <span class=confidence_class(&project.confidence)>{confidence_label(&project.confidence)}</span>
            </div>
        </header>

        {currency_note.map(|note| view! { <p class="costs-currency-note">{note}</p> })}

        <Show when=move || truncated>
            <p class="costs-currency-note">"Showing the 200 most recent records."</p>
        </Show>

        {if empty {
            view! {
                <div class="costs-detail-empty">
                    <strong>"No cost records"</strong>
                    <small>"Utu has not observed spend for this project. Connected CLI agents do not report provider invoices, so this is unsupported — not $0."</small>
                </div>
            }.into_any()
        } else {
            view! {
                <div class="costs-record-list" aria-label="Cost records">
                    {record_items}
                </div>
            }.into_any()
        }}
    }
}

#[derive(Clone)]
struct CostBoard {
    projects: Vec<CostProject>,
    coverage: CostCoverage,
}

#[derive(Clone)]
struct CostProject {
    id: String,
    name: String,
    amount_display: String,
    confidence: String,
    coverage_line: String,
    detail_line: String,
    records: Vec<CostRecord>,
    other_currencies: Vec<String>,
    truncated: bool,
}

#[derive(Clone)]
struct CostCoverage {
    headline: String,
    detail: String,
    has_demo: bool,
}

impl CostBoard {
    fn from_snapshot(snapshot: &WorkspaceSnapshot) -> Self {
        let projects = snapshot
            .projects
            .iter()
            .map(|project| {
                let summary = snapshot
                    .costs
                    .iter()
                    .find(|cost| cost.project_id == project.id);
                CostProject::from_summary(project.id.clone(), project.name.clone(), summary)
            })
            .collect::<Vec<_>>();
        let coverage = CostCoverage::from_snapshot(snapshot, &projects);
        Self { projects, coverage }
    }
}

impl CostProject {
    fn from_summary(id: String, name: String, summary: Option<&ProjectCostSummary>) -> Self {
        let records = summary.map(|item| item.records.clone()).unwrap_or_default();
        let known = summary.map(|item| item.known_records).unwrap_or(0);
        let unknown = summary.map(|item| item.unknown_records).unwrap_or(0);
        let mut other_currencies = records
            .iter()
            .map(|record| record.amount.currency.clone())
            .filter(|currency| !currency.eq_ignore_ascii_case("USD"))
            .collect::<Vec<_>>();
        other_currencies.sort();
        other_currencies.dedup();
        let empty = records.is_empty() && known == 0 && unknown == 0;
        let usd_empty_with_other = !empty && known == 0 && unknown == 0;
        let confidence = if empty {
            "empty".into()
        } else if usd_empty_with_other {
            "unknown".into()
        } else {
            summary
                .map(|item| item.amount.confidence.clone())
                .unwrap_or_else(|| "unknown".into())
        };
        let amount_display = if empty {
            "No records".into()
        } else if usd_empty_with_other {
            "No USD records".into()
        } else {
            summary
                .map(|item| format_cost_amount(&item.amount))
                .unwrap_or_else(|| "Unknown".into())
        };
        let coverage_line = if empty {
            "No records · unsupported by connected CLIs".into()
        } else if usd_empty_with_other {
            format!(
                "Recorded in {} · not converted",
                other_currencies.join(", ")
            )
        } else {
            format!(
                "{} · {} known · {} unknown",
                confidence_label(&confidence),
                known,
                unknown
            )
        };
        let detail_line = if empty {
            "No spend has been recorded. Unknown is not $0.".into()
        } else if usd_empty_with_other {
            "USD summary is empty. Other currencies are listed and not converted.".into()
        } else if summary.is_some_and(|item| item.complete) {
            format!(
                "{known} exact observed record{}",
                if known == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Known amounts are {} — not a complete invoice. {} unknown record{}.",
                confidence_label(&confidence).to_ascii_lowercase(),
                unknown,
                if unknown == 1 { "" } else { "s" }
            )
        };
        Self {
            id,
            name,
            amount_display,
            confidence,
            coverage_line,
            detail_line,
            truncated: records.len() >= 200,
            records,
            other_currencies,
        }
    }
}

impl CostCoverage {
    fn from_snapshot(snapshot: &WorkspaceSnapshot, projects: &[CostProject]) -> Self {
        let empty = projects
            .iter()
            .filter(|project| project.confidence == "empty")
            .count();
        let exact = projects
            .iter()
            .filter(|project| project.confidence == "exact")
            .count();
        let estimated = projects
            .iter()
            .filter(|project| project.confidence == "estimated")
            .count();
        let partial = projects
            .iter()
            .filter(|project| project.confidence == "partial")
            .count();
        let unknown = projects
            .iter()
            .filter(|project| project.confidence == "unknown")
            .count();
        let cost_capable = snapshot
            .agents
            .iter()
            .filter(|agent| agent.capabilities.costs)
            .count()
            + snapshot
                .integrations
                .iter()
                .filter(|integration| integration.capabilities.costs)
                .count();
        let has_demo = projects.iter().any(|project| {
            project
                .records
                .iter()
                .any(|record| is_demo_source(&record.source))
        });

        let mut parts = Vec::new();
        if exact > 0 {
            parts.push(format!("{exact} exact"));
        }
        if estimated > 0 {
            parts.push(format!("{estimated} estimated"));
        }
        if partial > 0 {
            parts.push(format!("{partial} partial"));
        }
        if unknown > 0 {
            parts.push(format!("{unknown} unknown"));
        }
        if empty > 0 {
            parts.push(format!("{empty} with no records",));
        }
        let headline = if parts.is_empty() {
            format!("{} projects", projects.len())
        } else {
            format!("{} projects · {}", projects.len(), parts.join(" · "))
        };

        let capability = if cost_capable == 0 {
            "No connected agent or connector reports cost. CLI usage is unsupported, not $0."
        } else {
            "Only connectors that advertise cost reporting can produce observed amounts. Others stay unsupported."
        };
        let demo = if has_demo {
            " Demonstration records are synthetic and labeled."
        } else {
            ""
        };
        Self {
            headline,
            detail: format!(
                "{capability}{demo} Totals are not summed across projects or currencies."
            ),
            has_demo,
        }
    }
}

fn format_cost_amount(amount: &CostAmount) -> String {
    match core_amount(amount) {
        Some(amount) => amount.display(),
        None => "Unknown".into(),
    }
}

fn core_amount(amount: &CostAmount) -> Option<CoreAmount> {
    let confidence = match amount.confidence.as_str() {
        "exact" => CostConfidence::Exact,
        "estimated" => CostConfidence::Estimated,
        "partial" => CostConfidence::Partial,
        "unknown" => CostConfidence::Unknown,
        _ => CostConfidence::Unknown,
    };
    CoreAmount::new(amount.currency.clone(), amount.micros, confidence).ok()
}

fn confidence_label(confidence: &str) -> &'static str {
    match confidence {
        "exact" => CostConfidence::Exact.label(),
        "estimated" => CostConfidence::Estimated.label(),
        "partial" => CostConfidence::Partial.label(),
        "empty" => "No records",
        _ => CostConfidence::Unknown.label(),
    }
}

fn confidence_class(confidence: &str) -> &'static str {
    match confidence {
        "exact" => "cost-confidence cost-confidence-exact",
        "estimated" => "cost-confidence cost-confidence-estimated",
        "partial" => "cost-confidence cost-confidence-partial",
        "empty" => "cost-confidence cost-confidence-empty",
        _ => "cost-confidence cost-confidence-unknown",
    }
}

fn is_demo_source(source: &str) -> bool {
    source == "utu.demo" || source.starts_with("utu.demo.")
}

fn source_label(source: &str) -> String {
    if is_demo_source(source) {
        "utu.demo".into()
    } else {
        source.to_owned()
    }
}

fn record_scope(
    record: &CostRecord,
    sessions: &[crate::ipc::SessionRecord],
    agents: &[crate::ipc::AgentRecord],
    tasks: &[crate::ipc::TaskRecord],
) -> String {
    let mut parts = Vec::new();
    if let Some(agent_id) = record.agent_id.as_deref() {
        let name = agents
            .iter()
            .find(|agent| agent.id == agent_id)
            .map(|agent| agent.display_name.clone())
            .unwrap_or_else(|| agent_id.to_owned());
        parts.push(name);
    }
    if let Some(session_id) = record.session_id.as_deref() {
        let title = sessions
            .iter()
            .find(|session| session.id == session_id)
            .and_then(|session| session.title_hint.clone())
            .unwrap_or_else(|| "Session".into());
        parts.push(title);
    }
    if let Some(task_id) = record.task_id.as_deref() {
        let title = tasks
            .iter()
            .find(|task| task.id == task_id)
            .map(|task| task.title.clone())
            .unwrap_or_else(|| task_id.to_owned());
        parts.push(title);
    }
    if parts.is_empty() {
        "Project-level record".into()
    } else {
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::{confidence_label, format_cost_amount};
    use crate::ipc::CostAmount;

    #[test]
    fn unknown_amount_does_not_become_zero() {
        let amount = CostAmount {
            currency: "USD".into(),
            micros: None,
            confidence: "unknown".into(),
        };
        assert_eq!(format_cost_amount(&amount), "Unknown");
    }

    #[test]
    fn estimated_amount_is_not_formatted_as_exact() {
        let estimated = CostAmount {
            currency: "USD".into(),
            micros: Some(184_000),
            confidence: "estimated".into(),
        };
        let exact = CostAmount {
            currency: "USD".into(),
            micros: Some(184_000),
            confidence: "exact".into(),
        };
        assert_eq!(format_cost_amount(&estimated), "~$0.18");
        assert_eq!(format_cost_amount(&exact), "$0.18");
        assert_eq!(confidence_label("empty"), "No records");
    }
}
