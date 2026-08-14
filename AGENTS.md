## Learned User Preferences

- Manage language toolchains with mise, not rustup or brew. Use `mise exec` and mise-provided cargo for Rust and wasm builds.
- Native macOS chrome should show the product name "Utu", not lowercase "utu" (app switcher, About menu, window titles).
- Session titles should be the session name, not the agent CLI name. Show official agent logos instead of CLI names or made-up icons; hover reveals the name.
- Sessions running outside Utu are observe-only: see and process them, but do not write to, interrupt, or send directions into those sessions. Format transcript contents as markdown.
- Title bar should align with the window controls. Omit "Local owner" and "Live local" chrome text; the status icon should reflect whether agents are active, idle, or awaiting interaction.
- Keep the Files pane closed by default.

## Learned Workspace Facts

- Utu is a Leptos + Tauri 2 desktop app. Toolchains are pinned in `mise.toml`; wasm/UI builds use trunk and `wasm32-unknown-unknown` via mise-managed cargo.
- Workspace is a left-to-right master-detail layout: projects, then that project's sessions, then session details — do not stack sessions under the project list.
- Monitor all agent types (Codex, Claude Code, Cursor, and others) in realtime; Fleet should list every running session. Use Sync only for initial setup, adding a project, or a manual request on Integrations/Project.
- Sync stays metadata-only; hydrate a session's transcript lazily when that session is opened.
- Cursor live sessions are discovered from `~/.cursor/projects/*/agent-transcripts/`. Local `cargo tauri dev` / `trunk serve` binds TCP 1421.
