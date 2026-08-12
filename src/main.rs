mod app;
mod components;
#[cfg(test)]
mod demo;
mod ipc;
mod views;
mod workspace_data;

fn main() {
    leptos::mount::mount_to_body(app::App);
}
