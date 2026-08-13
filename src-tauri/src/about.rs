use tauri::menu::{
    Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, SubmenuBuilder, WINDOW_SUBMENU_ID,
};
use tauri::webview::Color;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const ABOUT_MENU_ID: &str = "about";
pub const ABOUT_WINDOW_LABEL: &str = "about";

const PRODUCT_NAME: &str = "Utu";
const ABOUT_WIDTH: f64 = 360.0;
const ABOUT_HEIGHT: f64 = 428.0;
const CHROME_TOP: Color = Color(0xed, 0xf2, 0xef, 255);

pub fn build_menu(handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let about = MenuItem::with_id(
        handle,
        ABOUT_MENU_ID,
        format!("About {PRODUCT_NAME}"),
        true,
        None::<&str>,
    )?;

    #[cfg(target_os = "macos")]
    let app_menu = Submenu::with_items(
        handle,
        PRODUCT_NAME,
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::services(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::hide(handle, Some(&format!("Hide {PRODUCT_NAME}")))?,
            &PredefinedMenuItem::hide_others(handle, None)?,
            &PredefinedMenuItem::show_all(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::quit(handle, Some(&format!("Quit {PRODUCT_NAME}")))?,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(handle, None)?,
            &PredefinedMenuItem::redo(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::cut(handle, None)?,
            &PredefinedMenuItem::copy(handle, None)?,
            &PredefinedMenuItem::paste(handle, None)?,
            &PredefinedMenuItem::select_all(handle, None)?,
        ],
    )?;

    #[cfg(target_os = "macos")]
    let view_menu = Submenu::with_items(
        handle,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(handle, None)?],
    )?;

    let window_menu = SubmenuBuilder::with_id(handle, WINDOW_SUBMENU_ID, "Window")
        .item(&PredefinedMenuItem::minimize(handle, None)?)
        .item(&PredefinedMenuItem::maximize(handle, None)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(handle, None)?)
        .build()?;

    #[cfg(not(target_os = "macos"))]
    let file_menu = Submenu::with_items(
        handle,
        "File",
        true,
        &[
            &PredefinedMenuItem::close_window(handle, None)?,
            &PredefinedMenuItem::quit(handle, Some(&format!("Quit {PRODUCT_NAME}")))?,
        ],
    )?;

    #[cfg(not(target_os = "macos"))]
    let help_menu = SubmenuBuilder::with_id(handle, tauri::menu::HELP_SUBMENU_ID, "Help")
        .item(&about)
        .build()?;

    #[cfg(target_os = "macos")]
    let menu = Menu::with_items(handle, &[&app_menu, &edit_menu, &view_menu, &window_menu])?;

    #[cfg(not(target_os = "macos"))]
    let menu = Menu::with_items(handle, &[&file_menu, &edit_menu, &window_menu, &help_menu])?;

    Ok(menu)
}

pub fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    if event.id() == ABOUT_MENU_ID
        && let Err(error) = show_about_window(app)
    {
        eprintln!("could not open the About Utu window: {error}");
    }
}

pub fn show_about_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(ABOUT_WINDOW_LABEL) {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }

    let version = app.package_info().version.to_string();
    let mut builder = WebviewWindowBuilder::new(
        app,
        ABOUT_WINDOW_LABEL,
        WebviewUrl::App(format!("index.html?window=about&version={version}").into()),
    )
    .title(format!("About {PRODUCT_NAME}"))
    .inner_size(ABOUT_WIDTH, ABOUT_HEIGHT)
    .min_inner_size(ABOUT_WIDTH, ABOUT_HEIGHT)
    .max_inner_size(ABOUT_WIDTH, ABOUT_HEIGHT)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(true)
    .center()
    .focused(true)
    .accept_first_mouse(true)
    .shadow(true)
    .skip_taskbar(true)
    .background_color(CHROME_TOP)
    .decorations(true);

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(14.0, 18.0));
        if let Some(main) = app.get_webview_window("main") {
            builder = builder.parent(&main)?;
        }
    }

    builder.build()?;
    Ok(())
}

#[tauri::command]
pub fn close_about_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(ABOUT_WINDOW_LABEL) {
        window.close().map_err(|error| error.to_string())?;
    }
    Ok(())
}
