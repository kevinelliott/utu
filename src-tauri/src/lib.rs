use std::env;
use tauri_plugin_log::{Target, TargetKind};
use utu_connectors::{EnvironmentPath, LocalCliProbe, probe_known_local_clis};

#[tauri::command]
fn detect_local_clis() -> Vec<LocalCliProbe> {
    probe_known_local_clis(&EnvironmentPath)
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
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![detect_local_clis, host_summary])
        .run(tauri::generate_context!())
        .expect("Utu failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_binary_is_not_reported_as_authenticated() {
        let probes = detect_local_clis();
        assert!(
            probes
                .iter()
                .all(|probe| probe.auth_state == utu_core::AuthState::Unknown)
        );
    }
}
