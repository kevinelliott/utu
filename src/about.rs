use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::ipc;

const PRODUCT_NAME: &str = "Utu";
const PRODUCT_TAGLINE: &str = "A local-first operations workspace for AI agents.";

#[component]
pub fn AboutWindow() -> impl IntoView {
    let version = app_version();
    let handle = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" {
            spawn_local(async {
                let _ = ipc::close_about_window().await;
            });
        }
    });
    on_cleanup(move || drop(handle));

    view! {
        <main class="about-shell" aria-labelledby="about-name" data-tauri-drag-region="">
            <img class="about-icon" src="app-icon.svg" width="128" height="128" alt="" draggable="false" />
            <h1 id="about-name" class="about-name">{PRODUCT_NAME}</h1>
            <p class="about-version">"Version "{version}</p>
            <p class="about-tagline">{PRODUCT_TAGLINE}</p>
        </main>
    }
}

fn app_version() -> String {
    ipc::query_param("version").unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned())
}
