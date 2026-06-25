use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::{AccentColor, App, UiTheme};

const SETTINGS_FILE: &str = ".slidecli-settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiSettings {
    pub font_name: String,
    pub font_size: u8,
    pub ui_theme: UiTheme,
    pub accent_color: AccentColor,
}

impl UiSettings {
    pub fn from_app(app: &App) -> Self {
        Self {
            font_name: app.presentation.font_name.clone(),
            font_size: app.presentation.font_size,
            ui_theme: app.ui_theme,
            accent_color: app.accent_color,
        }
    }

    pub fn apply_to_app(&self, app: &mut App) {
        app.presentation.font_name = self.font_name.clone();
        app.presentation.font_size = self.font_size.clamp(8, 72);
        app.ui_theme = self.ui_theme;
        app.accent_color = self.accent_color;
    }
}

pub fn settings_path() -> PathBuf {
    PathBuf::from(SETTINGS_FILE)
}

pub fn load_settings(path: &Path) -> anyhow::Result<Option<UiSettings>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)?;
    let settings = serde_json::from_str(&raw)?;
    Ok(Some(settings))
}

pub fn save_settings(path: &Path, settings: &UiSettings) -> anyhow::Result<()> {
    let raw = serde_json::to_string_pretty(settings)?;
    fs::write(path, raw)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_settings_json() {
        let settings = UiSettings {
            font_name: "JetBrains Mono".to_string(),
            font_size: 20,
            ui_theme: UiTheme::HighContrast,
            accent_color: AccentColor::Pink,
        };

        let raw = serde_json::to_string(&settings).unwrap();
        let decoded: UiSettings = serde_json::from_str(&raw).unwrap();
        assert_eq!(decoded, settings);
    }
}
