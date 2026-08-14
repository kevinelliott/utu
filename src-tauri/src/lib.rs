mod about;
mod agent_sessions;
mod clock;
mod codex_commands;
mod codex_runtime;
mod commands;
mod ids;
mod project_files;
mod session_sync;
mod state;
mod supervisor;
mod transcript_import;

use std::env;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};
use utu_connectors::{EnvironmentPath, LocalCliProbe, probe_known_local_clis};

use crate::state::AppState;

#[tauri::command]
async fn detect_local_clis() -> Result<Vec<LocalCliProbe>, String> {
    tauri::async_runtime::spawn_blocking(|| probe_known_local_clis(&EnvironmentPath))
        .await
        .map_err(|error| format!("local CLI discovery worker failed: {error}"))
}

#[tauri::command]
fn host_summary() -> serde_json::Value {
    serde_json::json!({
        "os": env::consts::OS,
        "arch": env::consts::ARCH,
        "surface": "desktop",
        "controlMode": "local-owner"
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&[about::ABOUT_WINDOW_LABEL])
                .build(),
        )
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .build(),
        )
        .menu(about::build_menu)
        .on_menu_event(about::handle_menu_event)
        .setup(|app| {
            let data_directory = app
                .path()
                .app_local_data_dir()
                .map_err(|error| format!("could not resolve the Utu data directory: {error}"))?;
            let state =
                tauri::async_runtime::block_on(tauri::async_runtime::spawn_blocking(move || {
                    AppState::open(data_directory)
                }))
                .map_err(|error| format!("local store initialization worker failed: {error}"))??;
            app.manage(state.clone());
            state.supervisor.attach_and_start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_local_clis,
            host_summary,
            about::close_about_window,
            commands::pick_folder,
            session_sync::sync_project_sessions,
            commands::latest_connector_report,
            commands::connector_catalog,
            commands::refresh_connectors,
            commands::workspace_snapshot,
            commands::session_stream,
            commands::create_project,
            commands::save_project,
            commands::delete_project,
            commands::create_task,
            commands::save_task,
            commands::delete_task,
            commands::assign_task_agents,
            commands::delete_agent,
            commands::create_session,
            commands::delete_session,
            commands::send_direction,
            commands::request_control,
            commands::create_handoff,
            commands::resolve_attention,
            commands::search_workspace,
            commands::project_directory,
            commands::project_file_preview,
        ])
        .run(tauri::generate_context!())
        .expect("Utu failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_binary_is_not_reported_as_authenticated() {
        let probes = probe_known_local_clis(&EnvironmentPath);
        assert!(
            probes
                .iter()
                .all(|probe| probe.auth_state == utu_core::AuthState::Unknown)
        );
    }
}
