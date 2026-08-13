## Learned User Preferences

- Manage language toolchains with mise, not rustup or brew. Use `mise exec` and mise-provided cargo for Rust and wasm builds.
- Native macOS chrome should show the product name "Utu", not lowercase "utu" (app switcher, About menu, window titles).

## Learned Workspace Facts

- Utu is a Leptos + Tauri 2 desktop app. Toolchains are pinned in `mise.toml`; wasm/UI builds use trunk and `wasm32-unknown-unknown` via mise-managed cargo.
- Workspace is a left-to-right master-detail layout: projects, then that project's sessions, then session details — do not stack sessions under the project list.
- Monitor all agent types (Codex, Claude Code, and others) in realtime; Fleet should list every running session. Use Sync only for initial setup, adding a project, or a manual request on Integrations/Project.
