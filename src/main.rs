mod app;
mod components;
#[cfg(test)]
mod demo;
mod views;

fn main() {
    leptos::mount::mount_to_body(app::App);
}
