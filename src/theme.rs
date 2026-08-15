use leptos::prelude::*;
use wasm_bindgen::{JsCast, closure::Closure};

const STORAGE_KEY: &str = "utu.theme";
const LIGHT_THEME_COLOR: &str = "#e2e7ec";
const DARK_THEME_COLOR: &str = "#171e26";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedTheme {
    Light,
    Dark,
}

impl ThemePreference {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub fn resolve(self, system_dark: bool) -> ResolvedTheme {
        match self {
            Self::Light => ResolvedTheme::Light,
            Self::Dark => ResolvedTheme::Dark,
            Self::System => {
                if system_dark {
                    ResolvedTheme::Dark
                } else {
                    ResolvedTheme::Light
                }
            }
        }
    }
}

impl ResolvedTheme {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub const fn theme_color(self) -> &'static str {
        match self {
            Self::Light => LIGHT_THEME_COLOR,
            Self::Dark => DARK_THEME_COLOR,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ThemeController {
    pub preference: RwSignal<ThemePreference>,
}

impl ThemeController {
    pub fn install() -> Self {
        let preference = RwSignal::new(load_preference());
        apply_theme(preference.get_untracked());
        listen_system_changes(preference);
        Effect::new(move || {
            let pref = preference.get();
            save_preference(pref);
            apply_theme(pref);
        });
        let controller = Self { preference };
        provide_context(controller);
        controller
    }

    pub fn set(self, preference: ThemePreference) {
        self.preference.set(preference);
    }
}

pub fn hydrate() {
    apply_theme(load_preference());
}

fn load_preference() -> ThemePreference {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten())
        .map(|value| ThemePreference::parse(&value))
        .unwrap_or_default()
}

fn save_preference(preference: ThemePreference) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(STORAGE_KEY, preference.as_str());
    }
}

fn system_prefers_dark() -> bool {
    web_sys::window()
        .and_then(|window| {
            window
                .match_media("(prefers-color-scheme: dark)")
                .ok()
                .flatten()
        })
        .is_some_and(|media| media.matches())
}

fn apply_theme(preference: ThemePreference) {
    let resolved = preference.resolve(system_prefers_dark());
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    if let Some(root) = document.document_element() {
        let _ = root.set_attribute("data-theme", resolved.as_str());
    }
    if let Some(meta) = document
        .query_selector("meta[name='theme-color']")
        .ok()
        .flatten()
    {
        let _ = meta.set_attribute("content", resolved.theme_color());
    }
}

fn listen_system_changes(preference: RwSignal<ThemePreference>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(media)) = window.match_media("(prefers-color-scheme: dark)") else {
        return;
    };
    let closure = Closure::<dyn FnMut()>::new(move || {
        let pref = preference.get_untracked();
        if pref == ThemePreference::System {
            apply_theme(pref);
        }
    });
    let _ = media.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
    closure.forget();
}

#[cfg(test)]
mod tests {
    use super::{ResolvedTheme, ThemePreference};

    #[test]
    fn unknown_preference_defaults_to_system() {
        assert_eq!(ThemePreference::parse(""), ThemePreference::System);
        assert_eq!(ThemePreference::parse("nope"), ThemePreference::System);
        assert_eq!(ThemePreference::parse("system"), ThemePreference::System);
    }

    #[test]
    fn system_follows_prefers_color_scheme() {
        assert_eq!(ThemePreference::System.resolve(true), ResolvedTheme::Dark);
        assert_eq!(ThemePreference::System.resolve(false), ResolvedTheme::Light);
    }

    #[test]
    fn explicit_preference_ignores_system() {
        assert_eq!(ThemePreference::Light.resolve(true), ResolvedTheme::Light);
        assert_eq!(ThemePreference::Dark.resolve(false), ResolvedTheme::Dark);
    }

    #[test]
    fn preference_round_trips_through_as_str() {
        for preference in [
            ThemePreference::System,
            ThemePreference::Light,
            ThemePreference::Dark,
        ] {
            assert_eq!(ThemePreference::parse(preference.as_str()), preference);
        }
    }
}
