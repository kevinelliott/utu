mod about;
mod app;
mod components;
#[cfg(test)]
mod demo;
mod ipc;
mod views;
mod workspace_data;

fn main() {
    if crate::ipc::is_about_window() {
        leptos::mount::mount_to_body(about::AboutWindow);
    } else {
        leptos::mount::mount_to_body(app::App);
    }
}
