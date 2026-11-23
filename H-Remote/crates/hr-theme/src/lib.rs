//! Theme model shared by the Rust shell and the Svelte UI.
//!
//! Colours are stored in the same `#AARRGGBB` notation used by the reference
//! application and exported to CSS as `#RRGGBBAA` so the webview renders them
//! correctly.

use serde::{Deserialize, Serialize};

pub const MIN_FONT_SIZE: u8 = 10;
pub const MAX_FONT_SIZE: u8 = 20;
pub const CUSTOM_THEME_NAME: &str = "Custom";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ThemeError {
    #[error("invalid colour '{value}': {reason}")]
    InvalidColor { value: String, reason: String },
    #[error("unknown theme preset '{0}'")]
    UnknownPreset(String),
}

/// The ten colour slots every preset defines. Field names are stable because
/// they are persisted in `hremote.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThemeColors {
    pub primary_mid: String,
    pub primary_light: String,
    pub primary_dark: String,
    pub primary_text: String,
    pub accent_mid: String,
    pub accent_light: String,
    pub accent_dark: String,
    pub accent_text: String,
    pub background: String,
    pub background_text: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            primary_mid: "#323233".to_string(),
            primary_light: "#474748".to_string(),
            primary_dark: "#2d2d2d".to_string(),
            primary_text: "#cccccc".to_string(),
            accent_mid: "#FF007ACC".to_string(),
            accent_light: "#FF32A7F4".to_string(),
            accent_dark: "#FF0061A3".to_string(),
            accent_text: "#FFFFFFFF".to_string(),
            background: "#1e1e1e".to_string(),
            background_text: "#cccccc".to_string(),
        }
    }
}

impl ThemeColors {
    fn all(&self) -> [(&'static str, &str); 10] {
        [
            ("primaryMid", &self.primary_mid),
            ("primaryLight", &self.primary_light),
            ("primaryDark", &self.primary_dark),
            ("primaryText", &self.primary_text),
            ("accentMid", &self.accent_mid),
            ("accentLight", &self.accent_light),
            ("accentDark", &self.accent_dark),
            ("accentText", &self.accent_text),
            ("background", &self.background),
            ("backgroundText", &self.background_text),
        ]
    }
}

/// Persisted theme selection. `theme_name` selects a preset; when it is
/// `Custom` (or `is_custom` is true) the colour fields are authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThemeConfig {
    pub theme_name: String,
    pub is_custom: bool,
    pub colors: ThemeColors,
    pub font_family: String,
    pub font_size: u8,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            theme_name: "Dark".to_string(),
            is_custom: false,
            colors: ThemeColors::default(),
            font_family: "system-ui".to_string(),
            font_size: 12,
        }
    }
}

impl ThemeConfig {
    /// Builds a config that selects an existing preset.
    pub fn from_preset(name: &str) -> Result<Self, ThemeError> {
        let preset =
            find_preset(name).ok_or_else(|| ThemeError::UnknownPreset(name.to_string()))?;
        Ok(Self {
            theme_name: preset.name.to_string(),
            is_custom: false,
            colors: preset.colors.clone(),
            font_family: "system-ui".to_string(),
            font_size: 12,
        })
    }

    /// Clamps persisted values into their supported range. Called whenever a
    /// config file is loaded or written.
    pub fn sanitize(&mut self) {
        if self.theme_name.trim().is_empty() {
            self.theme_name = "Dark".to_string();
        }
        self.font_size = self.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        if self.font_family.trim().is_empty() {
            self.font_family = "system-ui".to_string();
        }
    }

    pub fn select_preset(&mut self, name: &str) -> Result<(), ThemeError> {
        let mut preset = Self::from_preset(name)?;
        preset.font_family = self.font_family.clone();
        preset.font_size = self.font_size;
        *self = preset;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePreset {
    pub name: &'static str,
    pub display_name: &'static str,
    pub colors: ThemeColors,
}

/// The ten presets shipped with H-Remote (nine reference palettes plus a
/// high-contrast accessibility preset), with their original palette values.
pub fn presets() -> Vec<ThemePreset> {
    vec![
        preset(
            "Light",
            "Light",
            "#FFF2F3F5",
            "#FFFFFFFF",
            "#FFE4E7EB",
            "#FF232323",
            "#FFE83D61",
            "#FFED6884",
            "#FFB5304C",
            "#FFFFFFFF",
            "#FFFFFFFF",
            "#000000",
        ),
        preset(
            "Dark",
            "Dark",
            "#323233",
            "#474748",
            "#2d2d2d",
            "#cccccc",
            "#FF007ACC",
            "#FF32A7F4",
            "#FF0061A3",
            "#FFFFFFFF",
            "#1e1e1e",
            "#cccccc",
        ),
        preset(
            "PRemoteM",
            "PRemoteM",
            "#102b3e",
            "#445a68",
            "#0c2230",
            "#FFFFFFFF",
            "#FFE83D61",
            "#FFED6884",
            "#FFB5304C",
            "#FFFFFFFF",
            "#ced8e1",
            "#000000",
        ),
        preset(
            "SecretKey",
            "SecretKey",
            "#FF473368",
            "#796090",
            "#382853",
            "#FFFFFFFF",
            "#FFEF6D3B",
            "#FF9A63",
            "#BF572F",
            "#FFFFFFFF",
            "#FFF2F1EC",
            "#000000",
        ),
        preset(
            "Greystone",
            "Greystone",
            "#FFC7D0D5",
            "#F9FDFD",
            "#9FA6AA",
            "#FF1B2C3F",
            "#FFFF7247",
            "#FFED583A",
            "#CC5B38",
            "#FFFFFFFF",
            "#FFF5F5F5",
            "#000000",
        ),
        preset(
            "Asphalt",
            "Asphalt",
            "#FF393939",
            "#6B6661",
            "#2D2D2D",
            "#FFFFFFFF",
            "#FFFF7247",
            "#FFED583A",
            "#CC5B38",
            "#FFFFFFFF",
            "#FFF5F5F5",
            "#000000",
        ),
        preset(
            "Wine",
            "Wine",
            "#FF57112D",
            "#893E55",
            "#450D24",
            "#FFFFFFFF",
            "#FFA82159",
            "#DA4E81",
            "#861A47",
            "#FFFFFFFF",
            "#FFFDEAD9",
            "#FF450D24",
        ),
        preset(
            "Forest",
            "Forest",
            "#FF253938",
            "#576660",
            "#1D2D2C",
            "#FFFFFFFF",
            "#FF5FA291",
            "#91CFB9",
            "#4C8174",
            "#FFFFFFFF",
            "#FFF5F5F5",
            "#FF303030",
        ),
        preset(
            "Soil",
            "Soil",
            "#FF776245",
            "#A98F6D",
            "#FF735E41",
            "#FFFFFFFF",
            "#FF0193B8",
            "#33C0E0",
            "#007593",
            "#FFFFFFFF",
            "#FFCFC3B5",
            "#FF080000",
        ),
        preset(
            "HighContrast",
            "High Contrast",
            "#FF000000",
            "#FF1A1A1A",
            "#FF000000",
            "#FFFFFFFF",
            "#FFFFD400",
            "#FFFFE766",
            "#FFB89A00",
            "#FF000000",
            "#FF000000",
            "#FFFFFFFF",
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn preset(
    name: &'static str,
    display_name: &'static str,
    primary_mid: &str,
    primary_light: &str,
    primary_dark: &str,
    primary_text: &str,
    accent_mid: &str,
    accent_light: &str,
    accent_dark: &str,
    accent_text: &str,
    background: &str,
    background_text: &str,
) -> ThemePreset {
    ThemePreset {
        name,
        display_name,
        colors: ThemeColors {
            primary_mid: primary_mid.to_string(),
            primary_light: primary_light.to_string(),
            primary_dark: primary_dark.to_string(),
            primary_text: primary_text.to_string(),
            accent_mid: accent_mid.to_string(),
            accent_light: accent_light.to_string(),
            accent_dark: accent_dark.to_string(),
            accent_text: accent_text.to_string(),
            background: background.to_string(),
            background_text: background_text.to_string(),
        },
    }
}

pub fn find_preset(name: &str) -> Option<ThemePreset> {
    presets()
        .into_iter()
        .find(|preset| preset.name.eq_ignore_ascii_case(name))
}

/// A fully validated theme ready to be applied by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTheme {
    pub id: String,
    pub display_name: String,
    pub colors: ThemeColors,
    pub font_family: String,
    pub font_size: u8,
}

/// Resolves a persisted config into a usable theme.
///
/// Invalid colour values are reported as errors when the theme was explicitly
/// customised; for a preset the palette is authoritative. Unknown preset names
/// fall back to `Dark` so a corrupted config can never prevent startup.
pub fn resolve(config: &ThemeConfig) -> Result<ResolvedTheme, ThemeError> {
    let mut sanitized = config.clone();
    sanitized.sanitize();

    if sanitized.is_custom || sanitized.theme_name.eq_ignore_ascii_case(CUSTOM_THEME_NAME) {
        for (slot, value) in sanitized.colors.all() {
            validate_color(value).map_err(|error| ThemeError::InvalidColor {
                value: format!("{slot}={value}"),
                reason: error.to_string(),
            })?;
        }
        return Ok(ResolvedTheme {
            id: CUSTOM_THEME_NAME.to_string(),
            display_name: CUSTOM_THEME_NAME.to_string(),
            colors: sanitized.colors,
            font_family: sanitized.font_family,
            font_size: sanitized.font_size,
        });
    }

    let preset = find_preset(&sanitized.theme_name).unwrap_or_else(|| find_preset("Dark").unwrap());
    Ok(ResolvedTheme {
        id: preset.name.to_string(),
        display_name: preset.display_name.to_string(),
        colors: preset.colors,
        font_family: sanitized.font_family,
        font_size: sanitized.font_size,
    })
}

/// Validates and normalises a colour to CSS `#RRGGBB` or `#RRGGBBAA`.
pub fn to_css_color(value: &str) -> Result<String, ThemeError> {
    let trimmed = value.trim();
    let hex = trimmed
        .strip_prefix('#')
        .ok_or_else(|| ThemeError::InvalidColor {
            value: trimmed.to_string(),
            reason: "must start with '#'".to_string(),
        })?;
    if !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(ThemeError::InvalidColor {
            value: trimmed.to_string(),
            reason: "contains non-hex characters".to_string(),
        });
    }
    match hex.len() {
        6 => Ok(format!("#{}", hex.to_ascii_uppercase())),
        8 => {
            // #AARRGGBB -> #RRGGBBAA
            let (alpha, rgb) = hex.split_at(2);
            if alpha.eq_ignore_ascii_case("FF") {
                Ok(format!("#{}", rgb.to_ascii_uppercase()))
            } else {
                Ok(format!(
                    "#{}{}",
                    rgb.to_ascii_uppercase(),
                    alpha.to_ascii_uppercase()
                ))
            }
        }
        _ => Err(ThemeError::InvalidColor {
            value: trimmed.to_string(),
            reason: "expected #RRGGBB or #AARRGGBB".to_string(),
        }),
    }
}

pub fn validate_color(value: &str) -> Result<(), ThemeError> {
    to_css_color(value).map(|_| ())
}

impl ResolvedTheme {
    /// CSS custom properties consumed by the Svelte design tokens.
    pub fn css_variables(&self) -> Vec<(String, String)> {
        let color = |value: &str| to_css_color(value).unwrap_or_else(|_| "#000000".to_string());
        let body = self.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE) as i32;
        vec![
            ("--hr-primary-mid".into(), color(&self.colors.primary_mid)),
            (
                "--hr-primary-light".into(),
                color(&self.colors.primary_light),
            ),
            ("--hr-primary-dark".into(), color(&self.colors.primary_dark)),
            ("--hr-primary-text".into(), color(&self.colors.primary_text)),
            ("--hr-accent-mid".into(), color(&self.colors.accent_mid)),
            ("--hr-accent-light".into(), color(&self.colors.accent_light)),
            ("--hr-accent-dark".into(), color(&self.colors.accent_dark)),
            ("--hr-accent-text".into(), color(&self.colors.accent_text)),
            ("--hr-background".into(), color(&self.colors.background)),
            (
                "--hr-background-text".into(),
                color(&self.colors.background_text),
            ),
            ("--hr-font-family".into(), self.font_family.clone()),
            ("--hr-font-size-body".into(), format!("{body}px")),
            (
                "--hr-font-size-small".into(),
                format!("{}px", (body - 2).max(10)),
            ),
            (
                "--hr-font-size-subtitle".into(),
                format!("{}px", (body + 2).min(20)),
            ),
            (
                "--hr-font-size-title".into(),
                format!("{}px", (body + 6).min(20)),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ships_exactly_ten_unique_presets() {
        let presets = presets();
        assert_eq!(presets.len(), 10);
        let mut names: Vec<&str> = presets.iter().map(|preset| preset.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 10);
        assert!(presets.iter().any(|preset| preset.name == "SecretKey"));
        assert!(presets.iter().any(|preset| preset.name == "Soil"));
        assert!(presets.iter().any(|preset| preset.name == "HighContrast"));
    }

    #[test]
    fn every_preset_colour_is_valid() {
        for preset in presets() {
            for (slot, value) in preset.colors.all() {
                validate_color(value)
                    .unwrap_or_else(|error| panic!("{}.{}: {error}", preset.name, slot));
            }
        }
    }

    #[test]
    fn argb_colours_are_converted_to_rgba() {
        assert_eq!(to_css_color("#FF007ACC").unwrap(), "#007ACC");
        assert_eq!(to_css_color("#80112233").unwrap(), "#11223380");
        assert_eq!(to_css_color("  #aabbcc ").unwrap(), "#AABBCC");
        assert!(to_css_color("007ACC").is_err());
        assert!(to_css_color("#GGGGGG").is_err());
        assert!(to_css_color("#12345").is_err());
    }

    #[test]
    fn unknown_preset_falls_back_to_dark() {
        let config = ThemeConfig {
            theme_name: "missing".to_string(),
            ..ThemeConfig::default()
        };
        let resolved = resolve(&config).unwrap();
        assert_eq!(resolved.id, "Dark");
    }

    #[test]
    fn custom_theme_is_validated() {
        let mut config = ThemeConfig {
            theme_name: CUSTOM_THEME_NAME.to_string(),
            is_custom: true,
            ..ThemeConfig::default()
        };
        assert!(resolve(&config).is_ok());
        config.colors.accent_mid = "not-a-colour".to_string();
        assert!(resolve(&config).is_err());
    }

    #[test]
    fn font_size_is_clamped_and_scaled_slots_never_exceed_bounds() {
        let mut config = ThemeConfig {
            font_size: 99,
            ..ThemeConfig::default()
        };
        config.sanitize();
        assert_eq!(config.font_size, MAX_FONT_SIZE);
        let resolved = resolve(&config).unwrap();
        let vars: std::collections::HashMap<_, _> = resolved.css_variables().into_iter().collect();
        assert_eq!(vars["--hr-font-size-body"], "20px");
        assert_eq!(vars["--hr-font-size-title"], "20px");
        assert_eq!(vars["--hr-font-size-small"], "18px");

        let mut tiny = ThemeConfig {
            font_size: 1,
            ..ThemeConfig::default()
        };
        tiny.sanitize();
        assert_eq!(tiny.font_size, MIN_FONT_SIZE);
        let vars: std::collections::HashMap<_, _> = resolve(&tiny)
            .unwrap()
            .css_variables()
            .into_iter()
            .collect();
        assert_eq!(vars["--hr-font-size-small"], "10px");
        assert_eq!(vars["--hr-font-size-title"], "16px");
    }

    #[test]
    fn selecting_a_preset_preserves_font_settings() {
        let mut config = ThemeConfig {
            font_family: "Inter".to_string(),
            font_size: 16,
            ..ThemeConfig::default()
        };
        config.select_preset("Forest").unwrap();
        assert_eq!(config.theme_name, "Forest");
        assert_eq!(config.font_family, "Inter");
        assert_eq!(config.font_size, 16);
        assert!(config.select_preset("Nope").is_err());
    }

    #[test]
    fn theme_config_round_trips_through_json() {
        let config = ThemeConfig::from_preset("Wine").unwrap();
        let encoded = serde_json::to_string(&config).unwrap();
        let decoded: ThemeConfig = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, config);
    }
}
