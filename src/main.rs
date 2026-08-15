mod about;
mod app;
mod components;
#[cfg(test)]
mod demo;
mod ipc;
mod markdown;
mod theme;
mod views;
mod workspace_data;

fn main() {
    crate::theme::hydrate();
    if crate::ipc::is_about_window() {
        leptos::mount::mount_to_body(about::AboutWindow);
    } else {
        leptos::mount::mount_to_body(app::App);
    }
}
