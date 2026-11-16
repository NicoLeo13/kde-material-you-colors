use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use iced::theme::Palette;
use iced::widget::{
    button, checkbox, column, container, horizontal_rule, pick_list, responsive, row, scrollable,
    slider, text, text_input, Row, Space,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Size, Task, Theme};

use crate::apply::ThemePaths;
use crate::color::hex_to_rgb;
use crate::config::{normalize_color, AppConfig};
use crate::dbus_client;
use crate::scheme::{MaterialYouScheme, SchemeRequest, SchemeVariant};
use crate::wallpaper;

/// Inner border radius for color role swatches inside preview cards (`better-ui` concentric radius rule).
pub const SWATCH_INNER_RADIUS: f32 = 8.0;
/// Padding around inner swatches inside preview cards (`better-ui` concentric radius rule).
pub const SWATCH_CARD_PADDING: f32 = 8.0;
/// Outer border radius for preview cards: `SWATCH_INNER_RADIUS + SWATCH_CARD_PADDING` (`16.0`).
pub const SWATCH_OUTER_RADIUS: f32 = SWATCH_INNER_RADIUS + SWATCH_CARD_PADDING;

/// Inner border radius for grouped control cards.
pub const SECTION_INNER_RADIUS: f32 = 10.0;
/// Padding inside grouped control cards.
pub const SECTION_CARD_PADDING: f32 = 14.0;
/// Outer border radius for grouped control cards: `SECTION_INNER_RADIUS + SECTION_CARD_PADDING` (`24.0`).
pub const SECTION_OUTER_RADIUS: f32 = SECTION_INNER_RADIUS + SECTION_CARD_PADDING;

/// Ordered list of Material You roles displayed in the Live Palette Preview section.
pub const PREVIEW_ROLES: [(&str, &str, &str); 9] = [
    ("background", "Background", "onBackground"),
    ("surface", "Surface", "onSurface"),
    ("surfaceContainerLow", "Surface Low", "onSurface"),
    ("surfaceContainer", "Surface Container", "onSurface"),
    ("surfaceContainerHigh", "Surface High", "onSurface"),
    ("primary", "Primary", "onPrimary"),
    ("secondary", "Secondary", "onSecondary"),
    ("tertiary", "Tertiary", "onTertiary"),
    ("error", "Error", "onError"),
];

/// All 9 Material You scheme variants in canonical `0..=8` order.
pub const ALL_SCHEME_VARIANTS: [SchemeVariant; 9] = [
    SchemeVariant::Content,
    SchemeVariant::Expressive,
    SchemeVariant::Fidelity,
    SchemeVariant::Monochrome,
    SchemeVariant::Neutral,
    SchemeVariant::TonalSpot,
    SchemeVariant::Vibrant,
    SchemeVariant::Rainbow,
    SchemeVariant::FruitSalad,
];

/// Supported Material 3 specification versions.
pub const ALL_SPEC_VERSIONS: [&str; 2] = ["2025", "2021"];

impl fmt::Display for SchemeVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Content => "Content",
            Self::Expressive => "Expressive",
            Self::Fidelity => "Fidelity",
            Self::Monochrome => "Monochrome",
            Self::Neutral => "Neutral",
            Self::TonalSpot => "Tonal Spot",
            Self::Vibrant => "Vibrant",
            Self::Rainbow => "Rainbow",
            Self::FruitSalad => "Fruit Salad",
        };
        f.write_str(label)
    }
}

/// Three-state light/dark theme mode selector for Plasma and Konsole/Pywal/KSyntax targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    FollowScheme,
    Dark,
    Light,
}

impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::FollowScheme, Self::Dark, Self::Light];

    /// Resolve the current Plasma theme mode from `AppConfig`.
    pub fn from_plasma_config(config: &AppConfig) -> Self {
        if config.plasma_follows_scheme {
            Self::FollowScheme
        } else {
            match config.light {
                Some(true) => Self::Light,
                _ => Self::Dark,
            }
        }
    }

    /// Apply this mode to the Plasma fields of `AppConfig`.
    pub fn apply_to_plasma_config(self, config: &mut AppConfig) {
        match self {
            Self::FollowScheme => {
                config.plasma_follows_scheme = true;
                config.light = None;
            }
            Self::Light => {
                config.plasma_follows_scheme = false;
                config.light = Some(true);
            }
            Self::Dark => {
                config.plasma_follows_scheme = false;
                config.light = Some(false);
            }
        }
    }

    /// Resolve the current Konsole/Pywal/KSyntax theme mode from `AppConfig`.
    pub fn from_pywal_config(config: &AppConfig) -> Self {
        if config.pywal_follows_scheme {
            Self::FollowScheme
        } else {
            match config.pywal_light {
                Some(true) => Self::Light,
                _ => Self::Dark,
            }
        }
    }

    /// Apply this mode to the Konsole/Pywal/KSyntax fields of `AppConfig`.
    pub fn apply_to_pywal_config(self, config: &mut AppConfig) {
        match self {
            Self::FollowScheme => {
                config.pywal_follows_scheme = true;
                config.pywal_light = None;
            }
            Self::Light => {
                config.pywal_follows_scheme = false;
                config.pywal_light = Some(true);
            }
            Self::Dark => {
                config.pywal_follows_scheme = false;
                config.pywal_light = Some(false);
            }
        }
    }
}

impl fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FollowScheme => f.write_str("Follow Color Scheme"),
            Self::Dark => f.write_str("Dark"),
            Self::Light => f.write_str("Light"),
        }
    }
}

/// Convert a hex color string (`#rgb`, `#rrggbb`, `#aarrggbb`, or without `#`) into an `iced::Color`.
pub fn hex_to_iced_color(hex: &str) -> Option<Color> {
    let cleaned = hex.trim().trim_start_matches('#');
    if cleaned.len() == 8 {
        let a = u8::from_str_radix(&cleaned[0..2], 16).ok()?;
        let r = u8::from_str_radix(&cleaned[2..4], 16).ok()?;
        let g = u8::from_str_radix(&cleaned[4..6], 16).ok()?;
        let b = u8::from_str_radix(&cleaned[6..8], 16).ok()?;
        return Some(Color::from_rgba8(r, g, b, (a as f32) / 255.0));
    }
    let (r, g, b) = hex_to_rgb(hex).ok()?;
    Some(Color::from_rgb8(r, g, b))
}

/// Navigation sections in the Studio Split-View settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    ColorSource,
    Appearance,
    Targets,
    Advanced,
}

impl SettingsSection {
    pub const ALL: [Self; 4] = [
        Self::ColorSource,
        Self::Appearance,
        Self::Targets,
        Self::Advanced,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ColorSource => "Color Source & Seeds",
            Self::Appearance => "Appearance & Multipliers",
            Self::Targets => "Targets & Decorations",
            Self::Advanced => "Advanced & Misc",
        }
    }
}

/// Theme mode for the Settings GUI window itself (independent of the Plasma target config).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiThemeMode {
    System,
    Dark,
    Light,
}

impl GuiThemeMode {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];

    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "Follow System",
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

/// User interaction messages handled by `SettingsGui`.
#[derive(Debug, Clone, PartialEq)]
pub enum GuiMessage {
    // Navigation & GUI Theme
    SelectSection(SettingsSection),
    GuiThemeModeChanged(GuiThemeMode),

    // Color Source
    UseWallpaperSource(bool),
    CustomColorChanged(String),
    MonitorChanged(u32),
    MonitorInputChanged(String),
    SeedIndexSelected(u32),
    FetchColorsNow,

    // Dark / Light Mode
    PlasmaModeChanged(ThemeMode),
    PywalModeChanged(ThemeMode),

    // Scheme & Multipliers
    SchemeVariantChanged(SchemeVariant),
    ChromaMultiplierChanged(f64),
    ChromaInputChanged(String),
    ResetChromaMultiplier,
    ToneMultiplierChanged(f64),
    ToneInputChanged(String),
    ResetToneMultiplier,
    ContrastLevelChanged(f64),
    ContrastInputChanged(String),
    ResetContrastLevel,
    FrameContrastChanged(f64),
    FrameContrastInputChanged(String),
    ResetFrameContrast,
    SpecVersionChanged(String),

    // Targets & Advanced Settings
    ToggleAdvanced(bool),
    ApplyKonsoleChanged(bool),
    KonsoleOpacityChanged(u32),
    KonsoleOpacityDarkChanged(u32),
    KonsoleBlurChanged(bool),
    ApplyPywalChanged(bool),
    ApplyKittyChanged(bool),
    TitlebarOpacityOverrideChanged(bool),
    TitlebarOpacityChanged(u32),
    TitlebarOpacityDarkChanged(u32),
    ToolbarOpacityOverrideChanged(bool),
    ToolbarOpacityChanged(u32),
    ToolbarOpacityDarkChanged(u32),
    SierraBreezeButtonsChanged(bool),
    KlassyWindecoOutlineChanged(bool),
    KdeRoundedCornersOutlineChanged(bool),
    DarkerWindowListChanged(String),
    IconsLightChanged(String),
    IconsDarkChanged(String),
    OnChangeHookChanged(String),
    PauseModeChanged(bool),
    ScreenshotOnlyModeChanged(bool),
    ManualFetchChanged(bool),

    // Persistence & IPC Reload
    SaveAndApply,
}

/// Pure-Rust Material You configuration and live palette preview window state.
#[derive(Debug, Clone)]
pub struct SettingsGui {
    pub config: AppConfig,
    pub config_path: PathBuf,
    pub runtime_json_path: PathBuf,
    pub current_scheme: Option<MaterialYouScheme>,
    pub active_section: SettingsSection,
    pub gui_theme_mode: GuiThemeMode,
    pub system_light: bool,
    pub chroma_input: String,
    pub tone_input: String,
    pub contrast_input: String,
    pub frame_contrast_input: String,
    pub show_advanced: bool,
    pub has_unsaved_changes: bool,
    pub status_message: String,
}

impl Default for SettingsGui {
    fn default() -> Self {
        let config_path = AppConfig::default_path()
            .unwrap_or_else(|| PathBuf::from("/tmp/kde-material-you-colors-config.conf"));
        let runtime_json_path = std::env::temp_dir().join(format!(
            "kde-material-you-colors-{}.json",
            dbus_client::current_username()
        ));
        Self::new_with_paths(config_path, runtime_json_path)
    }
}

impl SettingsGui {
    /// Initialize `SettingsGui` from explicit configuration and runtime JSON paths,
    /// loading any existing scheme preview or generating a live fallback scheme.
    pub fn new_with_paths(config_path: PathBuf, runtime_json_path: PathBuf) -> Self {
        let config = AppConfig::load_from_path(&config_path).unwrap_or_default();
        let loaded_scheme = Self::load_runtime_scheme(&runtime_json_path);

        let chroma_input = format!("{:.2}", config.chroma_multiplier);
        let tone_input = format!("{:.2}", config.tone_multiplier);
        let contrast_input = format!("{:.2}", config.contrast_level);
        let frame_contrast_input = format!("{:.2}", config.frame_contrast);
        let system_light = Self::detect_system_light_mode();

        let mut gui = Self {
            config,
            config_path,
            runtime_json_path,
            current_scheme: loaded_scheme,
            active_section: SettingsSection::ColorSource,
            gui_theme_mode: GuiThemeMode::System,
            system_light,
            chroma_input,
            tone_input,
            contrast_input,
            frame_contrast_input,
            show_advanced: false,
            has_unsaved_changes: false,
            status_message: "Ready".to_string(),
        };

        if gui.current_scheme.is_none() {
            gui.regenerate_preview_scheme();
        }
        gui
    }

    /// Detect whether KDE Plasma is currently using a Light color scheme via `~/.config/kdeglobals`.
    pub fn detect_system_light_mode() -> bool {
        let paths = ThemePaths::default();
        let light_scheme = paths.color_schemes_dir().join("MaterialYouLight.colors");
        let dark_scheme = paths.color_schemes_dir().join("MaterialYouDark.colors");
        wallpaper::read_kdeglobals_light_mode(&paths.kdeglobals_path(), &light_scheme, &dark_scheme)
            .unwrap_or(false)
    }

    /// Parse an existing runtime JSON file (if present) or reconstruct a `MaterialYouScheme` from it.
    fn load_runtime_scheme(runtime_json_path: &Path) -> Option<MaterialYouScheme> {
        if !runtime_json_path.exists() {
            return None;
        }
        let raw = std::fs::read_to_string(runtime_json_path).ok()?;
        if let Ok(scheme) = serde_json::from_str::<MaterialYouScheme>(&raw) {
            return Some(scheme);
        }
        let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
        let seed_hex = val
            .get("seed")
            .and_then(|s| s.get("color"))
            .and_then(|c| c.as_str())
            .map(String::from);
        let best_vec: Vec<String> = val
            .get("best")
            .and_then(|b| b.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let req = SchemeRequest {
            source_color_hex: seed_hex,
            ..Default::default()
        };
        let mut generated = MaterialYouScheme::generate(&req).ok()?;
        if !best_vec.is_empty() {
            let mut best_map = BTreeMap::new();
            for (i, c) in best_vec.iter().enumerate() {
                best_map.insert(i.to_string(), c.clone());
            }
            generated.best = best_map;
            generated.best_list = best_vec;
        }
        Some(generated)
    }

    /// Regenerate `self.current_scheme` live from the current `self.config` and any existing extracted seed colors.
    pub fn regenerate_preview_scheme(&mut self) {
        let existing_best: Vec<String> = self
            .current_scheme
            .as_ref()
            .map(|s| s.best_list.clone())
            .unwrap_or_default();

        let existing_wallpaper_image: Option<PathBuf> = self
            .current_scheme
            .as_ref()
            .filter(|s| s.wallpaper.source_type == "image")
            .map(|s| PathBuf::from(&s.wallpaper.data))
            .filter(|p| p.exists());

        let custom_hex = self
            .config
            .color
            .as_deref()
            .and_then(normalize_color);

        let image_path = if custom_hex.is_some() {
            None
        } else {
            self.config
                .file
                .as_deref()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .or(existing_wallpaper_image)
        };

        let source_color_hex = if let Some(hex) = custom_hex {
            Some(hex)
        } else if image_path.is_none() && !existing_best.is_empty() {
            let idx = (self.config.ncolor as usize).min(existing_best.len().saturating_sub(1));
            existing_best.get(idx).cloned()
        } else {
            None
        };

        let is_light = self.config.light.unwrap_or(self.system_light);
        let is_pywal_light = self.config.pywal_light.unwrap_or(is_light);
        let chroma_mult = if self.config.chroma_multiplier <= 0.0 {
            0.001
        } else {
            self.config.chroma_multiplier
        };

        let req = SchemeRequest {
            source_color_hex,
            image_path,
            ncolor: self.config.ncolor as usize,
            variant: SchemeVariant::from_u8(self.config.scheme_variant),
            chroma_multiplier: chroma_mult,
            tone_multiplier: self.config.tone_multiplier,
            contrast_level: self.config.contrast_level,
            frame_contrast: self.config.frame_contrast,
            light: is_light,
            pywal_light: is_pywal_light,
            light_saturation_multiplier: self.config.light_blend_multiplier,
            dark_saturation_multiplier: self.config.dark_blend_multiplier,
            light_brightness_multiplier: 1.0,
            dark_brightness_multiplier: 1.0,
            custom_colors_list: self.config.custom_colors_list.clone().unwrap_or_default(),
        };

        if let Ok(mut scheme) = MaterialYouScheme::generate(&req) {
            if self.config.color.is_none() && !existing_best.is_empty() && scheme.best_list.len() == 1 {
                let mut best_map = BTreeMap::new();
                for (i, c) in existing_best.iter().enumerate() {
                    best_map.insert(i.to_string(), c.clone());
                }
                let seed_idx = (self.config.ncolor as usize).min(existing_best.len() - 1);
                scheme.seed.index = seed_idx;
                if let Some(seed_c) = existing_best.get(seed_idx) {
                    scheme.seed.color = seed_c.clone();
                }
                scheme.best = best_map;
                scheme.best_list = existing_best;
            }
            self.current_scheme = Some(scheme);
        }
    }

    /// Resolve the companion `.reload` trigger path for the current runtime JSON path.
    pub fn reload_trigger_path(&self) -> PathBuf {
        if self.runtime_json_path.as_os_str().is_empty() {
            dbus_client::default_reload_trigger_path()
        } else {
            self.runtime_json_path.with_extension("reload")
        }
    }

    /// Write `self.config` to `self.config_path` and touch the `.reload` trigger file.
    pub fn save_and_touch_reload(&mut self) -> Result<PathBuf> {
        self.config.save_to_path(&self.config_path)?;
        let reload_path = self.reload_trigger_path();
        if let Some(parent) = reload_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create reload directory {}", parent.display())
                })?;
            }
        }
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let payload = format!("reload:{now_nanos}\n");
        std::fs::write(&reload_path, &payload).with_context(|| {
            format!(
                "Failed to write reload trigger file at {}",
                reload_path.display()
            )
        })?;

        // If runtime_json_path lives in /tmp, also ensure the standard $USER.reload path is touched
        let default_reload = dbus_client::default_reload_trigger_path();
        if reload_path != default_reload && self.runtime_json_path.starts_with(std::env::temp_dir()) {
            let _ = std::fs::write(&default_reload, &payload);
        }

        Ok(reload_path)
    }

    /// Handle GUI state transitions for all user controls.
    pub fn update(&mut self, message: GuiMessage) -> Task<GuiMessage> {
        let mut mutated = true;
        match message {
            GuiMessage::SelectSection(section) => {
                self.active_section = section;
                mutated = false;
            }
            GuiMessage::GuiThemeModeChanged(mode) => {
                self.gui_theme_mode = mode;
                mutated = false;
            }
            GuiMessage::UseWallpaperSource(use_wallpaper) => {
                if use_wallpaper {
                    self.config.color = None;
                } else if self.config.color.is_none() {
                    let seed = self
                        .current_scheme
                        .as_ref()
                        .map(|s| s.seed.color.clone())
                        .unwrap_or_else(|| "#4285f4".to_string());
                    self.config.color = Some(seed);
                }
                self.regenerate_preview_scheme();
            }
            GuiMessage::CustomColorChanged(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    self.config.color = None;
                } else if let Some(norm) = normalize_color(trimmed) {
                    self.config.color = Some(norm);
                } else {
                    self.config.color = Some(raw);
                }
                self.regenerate_preview_scheme();
            }
            GuiMessage::MonitorChanged(mon) => {
                self.config.monitor = mon;
            }
            GuiMessage::MonitorInputChanged(raw) => {
                if let Ok(parsed) = raw.trim().parse::<u32>() {
                    self.config.monitor = parsed;
                } else if raw.trim().is_empty() {
                    self.config.monitor = 0;
                }
            }
            GuiMessage::SeedIndexSelected(idx) => {
                self.config.ncolor = idx.min(6);
                self.config.color = None;
                self.regenerate_preview_scheme();
            }
            GuiMessage::FetchColorsNow => {
                mutated = false;
                self.config.fetch_colors = true;
                match self.save_and_touch_reload() {
                    Ok(_) => {
                        self.has_unsaved_changes = false;
                        self.status_message = "Requested immediate wallpaper color fetch".to_string();
                    }
                    Err(err) => {
                        self.status_message = format!("Failed to trigger fetch: {err}");
                    }
                }
            }
            GuiMessage::PlasmaModeChanged(mode) => {
                mode.apply_to_plasma_config(&mut self.config);
                self.regenerate_preview_scheme();
            }
            GuiMessage::PywalModeChanged(mode) => {
                mode.apply_to_pywal_config(&mut self.config);
                self.regenerate_preview_scheme();
            }
            GuiMessage::SchemeVariantChanged(variant) => {
                self.config.scheme_variant = variant.as_u8();
                self.regenerate_preview_scheme();
            }
            GuiMessage::ChromaMultiplierChanged(val) => {
                self.config.chroma_multiplier = ((val * 100.0).round() / 100.0).clamp(0.0, 4.0);
                self.chroma_input = format!("{:.2}", self.config.chroma_multiplier);
                self.regenerate_preview_scheme();
            }
            GuiMessage::ChromaInputChanged(raw) => {
                self.chroma_input = raw.clone();
                if let Ok(parsed) = raw.trim().parse::<f64>() {
                    if parsed.is_finite() {
                        self.config.chroma_multiplier = parsed.clamp(0.0, 4.0);
                        self.regenerate_preview_scheme();
                    }
                }
            }
            GuiMessage::ResetChromaMultiplier => {
                self.config.chroma_multiplier = 1.0;
                self.chroma_input = "1.00".to_string();
                self.regenerate_preview_scheme();
            }
            GuiMessage::ToneMultiplierChanged(val) => {
                self.config.tone_multiplier = ((val * 100.0).round() / 100.0).clamp(0.0, 1.5);
                self.tone_input = format!("{:.2}", self.config.tone_multiplier);
                self.regenerate_preview_scheme();
            }
            GuiMessage::ToneInputChanged(raw) => {
                self.tone_input = raw.clone();
                if let Ok(parsed) = raw.trim().parse::<f64>() {
                    if parsed.is_finite() {
                        self.config.tone_multiplier = parsed.clamp(0.0, 1.5);
                        self.regenerate_preview_scheme();
                    }
                }
            }
            GuiMessage::ResetToneMultiplier => {
                self.config.tone_multiplier = 1.0;
                self.tone_input = "1.00".to_string();
                self.regenerate_preview_scheme();
            }
            GuiMessage::ContrastLevelChanged(val) => {
                self.config.contrast_level = ((val * 100.0).round() / 100.0).clamp(-1.0, 1.0);
                self.contrast_input = format!("{:.2}", self.config.contrast_level);
                self.regenerate_preview_scheme();
            }
            GuiMessage::ContrastInputChanged(raw) => {
                self.contrast_input = raw.clone();
                if let Ok(parsed) = raw.trim().parse::<f64>() {
                    if parsed.is_finite() {
                        self.config.contrast_level = parsed.clamp(-1.0, 1.0);
                        self.regenerate_preview_scheme();
                    }
                }
            }
            GuiMessage::ResetContrastLevel => {
                self.config.contrast_level = 0.0;
                self.contrast_input = "0.00".to_string();
                self.regenerate_preview_scheme();
            }
            GuiMessage::FrameContrastChanged(val) => {
                self.config.frame_contrast = ((val * 100.0).round() / 100.0).clamp(0.0, 1.0);
                self.frame_contrast_input = format!("{:.2}", self.config.frame_contrast);
                self.regenerate_preview_scheme();
            }
            GuiMessage::FrameContrastInputChanged(raw) => {
                self.frame_contrast_input = raw.clone();
                if let Ok(parsed) = raw.trim().parse::<f64>() {
                    if parsed.is_finite() {
                        self.config.frame_contrast = parsed.clamp(0.0, 1.0);
                        self.regenerate_preview_scheme();
                    }
                }
            }
            GuiMessage::ResetFrameContrast => {
                self.config.frame_contrast = 0.0;
                self.frame_contrast_input = "0.00".to_string();
                self.regenerate_preview_scheme();
            }
            GuiMessage::SpecVersionChanged(spec) => {
                if spec == "2021" || spec == "2025" {
                    self.config.spec_version = spec;
                }
            }
            GuiMessage::ToggleAdvanced(show) => {
                mutated = false;
                self.show_advanced = show;
                if show {
                    self.active_section = SettingsSection::Advanced;
                }
            }
            GuiMessage::ApplyKonsoleChanged(apply_konsole) => {
                self.config.disable_konsole = !apply_konsole;
            }
            GuiMessage::KonsoleOpacityChanged(val) => {
                self.config.konsole_opacity = val.clamp(0, 100);
            }
            GuiMessage::KonsoleOpacityDarkChanged(val) => {
                self.config.konsole_opacity_dark = val.clamp(0, 100);
            }
            GuiMessage::KonsoleBlurChanged(blur) => {
                self.config.konsole_blur = blur;
            }
            GuiMessage::ApplyPywalChanged(pywal) => {
                self.config.pywal = pywal;
            }
            GuiMessage::ApplyKittyChanged(kitty) => {
                self.config.kitty = kitty;
            }
            GuiMessage::TitlebarOpacityOverrideChanged(override_on) => {
                self.config.titlebar_opacity_override = override_on;
            }
            GuiMessage::TitlebarOpacityChanged(val) => {
                self.config.titlebar_opacity = val.clamp(0, 100);
            }
            GuiMessage::TitlebarOpacityDarkChanged(val) => {
                self.config.titlebar_opacity_dark = val.clamp(0, 100);
            }
            GuiMessage::ToolbarOpacityOverrideChanged(override_on) => {
                self.config.toolbar_opacity_override = override_on;
            }
            GuiMessage::ToolbarOpacityChanged(val) => {
                self.config.toolbar_opacity = val.clamp(0, 100);
            }
            GuiMessage::ToolbarOpacityDarkChanged(val) => {
                self.config.toolbar_opacity_dark = val.clamp(0, 100);
            }
            GuiMessage::SierraBreezeButtonsChanged(enabled) => {
                self.config.sierra_breeze_buttons_color = enabled;
            }
            GuiMessage::KlassyWindecoOutlineChanged(enabled) => {
                self.config.klassy_windeco_outline = enabled;
            }
            GuiMessage::KdeRoundedCornersOutlineChanged(enabled) => {
                self.config.kde_rounded_corners_effect_outline = enabled;
            }
            GuiMessage::DarkerWindowListChanged(list) => {
                let trimmed = list.trim();
                self.config.darker_window_list = if trimmed.is_empty() {
                    None
                } else {
                    Some(list)
                };
            }
            GuiMessage::IconsLightChanged(icons) => {
                let trimmed = icons.trim();
                self.config.iconslight = if trimmed.is_empty() {
                    None
                } else {
                    Some(icons)
                };
            }
            GuiMessage::IconsDarkChanged(icons) => {
                let trimmed = icons.trim();
                self.config.iconsdark = if trimmed.is_empty() {
                    None
                } else {
                    Some(icons)
                };
            }
            GuiMessage::OnChangeHookChanged(hook) => {
                let trimmed = hook.trim();
                self.config.on_change_hook = if trimmed.is_empty() {
                    None
                } else {
                    Some(hook)
                };
            }
            GuiMessage::PauseModeChanged(paused) => {
                self.config.pause_mode = paused;
            }
            GuiMessage::ScreenshotOnlyModeChanged(screenshot_only) => {
                self.config.screenshot_only_mode = screenshot_only;
            }
            GuiMessage::ManualFetchChanged(manual) => {
                self.config.manual_fetch = manual;
            }
            GuiMessage::SaveAndApply => {
                mutated = false;
                match self.save_and_touch_reload() {
                    Ok(_) => {
                        self.has_unsaved_changes = false;
                        self.status_message =
                            format!("Saved configuration to {}", self.config_path.display());
                    }
                    Err(err) => {
                        self.status_message = format!("Failed to save configuration: {err}");
                    }
                }
            }
        }
        if mutated {
            self.has_unsaved_changes = true;
        }
        Task::none()
    }

    /// Return whether the effective preview/window theme is light mode.
    pub fn is_light_theme(&self) -> bool {
        match self.gui_theme_mode {
            GuiThemeMode::System => self.system_light,
            GuiThemeMode::Light => true,
            GuiThemeMode::Dark => false,
        }
    }

    /// Lookup an `iced::Color` for a Material You role with a fallback color.
    fn role_color(&self, role: &str, fallback: Color) -> Color {
        self.active_role_hex(role)
            .and_then(|hex| hex_to_iced_color(&hex))
            .unwrap_or(fallback)
    }

    /// Return a dynamic `iced::Theme` driven by the active Material You palette.
    pub fn theme(&self) -> Theme {
        let is_light = self.is_light_theme();
        let (fb_bg, fb_text, fb_primary, fb_success, fb_danger) = if is_light {
            (
                Color::from_rgb8(254, 247, 255),
                Color::from_rgb8(29, 27, 32),
                Color::from_rgb8(103, 80, 164),
                Color::from_rgb8(125, 82, 96),
                Color::from_rgb8(179, 38, 30),
            )
        } else {
            (
                Color::from_rgb8(20, 18, 24),
                Color::from_rgb8(230, 224, 233),
                Color::from_rgb8(208, 188, 255),
                Color::from_rgb8(239, 184, 200),
                Color::from_rgb8(242, 184, 181),
            )
        };

        Theme::custom(
            "Material You".to_string(),
            Palette {
                background: self.role_color("background", fb_bg),
                text: self.role_color("onBackground", fb_text),
                primary: self.role_color("primary", fb_primary),
                success: self.role_color("tertiary", fb_success),
                danger: self.role_color("error", fb_danger),
            },
        )
    }

    /// Lookup a role hex color from the currently active light/dark map of `self.current_scheme`.
    pub fn active_role_hex(&self, role: &str) -> Option<String> {
        let scheme = self.current_scheme.as_ref()?;
        let is_light = self.is_light_theme();
        let map = if is_light {
            &scheme.schemes.light
        } else {
            &scheme.schemes.dark
        };
        map.get(role).cloned()
    }

    /// Extract the resolved Material You UI palette for consistent widget styling.
    fn ui_palette(&self) -> UiPalette {
        let is_light = self.is_light_theme();
        if is_light {
            UiPalette {
                background: self.role_color("background", Color::from_rgb8(254, 247, 255)),
                surface: self.role_color("surfaceContainerLow", Color::from_rgb8(243, 237, 247)),
                surface_high: self.role_color("surfaceContainerHigh", Color::from_rgb8(230, 224, 233)),
                surface_highest: self.role_color("surfaceContainerHighest", Color::from_rgb8(219, 213, 224)),
                on_surface: self.role_color("onSurface", Color::from_rgb8(29, 27, 32)),
                on_surface_var: self.role_color("onSurfaceVariant", Color::from_rgb8(73, 69, 79)),
                outline: self.role_color("outlineVariant", Color::from_rgb8(202, 196, 208)),
                primary: self.role_color("primary", Color::from_rgb8(103, 80, 164)),
                on_primary: self.role_color("onPrimary", Color::WHITE),
                primary_container: self.role_color("primaryContainer", Color::from_rgb8(234, 221, 255)),
                on_primary_container: self.role_color("onPrimaryContainer", Color::from_rgb8(33, 0, 93)),
                tertiary: self.role_color("tertiary", Color::from_rgb8(125, 82, 96)),
                on_tertiary: self.role_color("onTertiary", Color::WHITE),
                error: self.role_color("error", Color::from_rgb8(179, 38, 30)),
            }
        } else {
            UiPalette {
                background: self.role_color("background", Color::from_rgb8(20, 18, 24)),
                surface: self.role_color("surfaceContainerLow", Color::from_rgb8(29, 27, 32)),
                surface_high: self.role_color("surfaceContainerHigh", Color::from_rgb8(43, 41, 48)),
                surface_highest: self.role_color("surfaceContainerHighest", Color::from_rgb8(54, 52, 59)),
                on_surface: self.role_color("onSurface", Color::from_rgb8(230, 224, 233)),
                on_surface_var: self.role_color("onSurfaceVariant", Color::from_rgb8(202, 196, 208)),
                outline: self.role_color("outlineVariant", Color::from_rgb8(73, 69, 79)),
                primary: self.role_color("primary", Color::from_rgb8(208, 188, 255)),
                on_primary: self.role_color("onPrimary", Color::from_rgb8(56, 30, 114)),
                primary_container: self.role_color("primaryContainer", Color::from_rgb8(79, 55, 139)),
                on_primary_container: self.role_color("onPrimaryContainer", Color::from_rgb8(234, 221, 255)),
                tertiary: self.role_color("tertiary", Color::from_rgb8(239, 184, 200)),
                on_tertiary: self.role_color("onTertiary", Color::from_rgb8(73, 37, 50)),
                error: self.role_color("error", Color::from_rgb8(242, 184, 181)),
            }
        }
    }

    /// Build the responsive Material You Studio Split-View interface.
    pub fn view(&self) -> Element<'_, GuiMessage> {
        let pal = self.ui_palette();

        let active_seed_hex = self
            .current_scheme
            .as_ref()
            .map(|s| s.seed.color.as_str())
            .unwrap_or("#4285f4");
        let active_variant = SchemeVariant::from_u8(self.config.scheme_variant);
        let mode_label = if self.is_light_theme() {
            "Light"
        } else {
            "Dark"
        };

        let fetch_btn = button(text("Fetch now").size(13))
            .padding([8, 13])
            .style(move |_theme: &Theme, _status| button::Style {
                background: Some(Background::Color(pal.surface_high)),
                text_color: pal.on_surface,
                border: Border {
                    color: pal.outline,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            })
            .on_press(GuiMessage::FetchColorsNow);

        let save_label = if self.has_unsaved_changes {
            "• Save & Apply"
        } else {
            "Save & Apply"
        };
        let save_btn = button(text(save_label).size(13))
            .padding([8, 15])
            .style(move |_theme: &Theme, _status| button::Style {
                background: Some(Background::Color(pal.primary)),
                text_color: pal.on_primary,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            })
            .on_press(GuiMessage::SaveAndApply);

        let header = container(
            Row::with_children(vec![
                column![
                    text("Material You Colors").size(19),
                    text(format!(
                        "Active seed {active_seed_hex} · {active_variant} · {mode_label}"
                    ))
                    .size(12)
                    .color(pal.on_surface_var),
                ]
                .spacing(2)
                .width(Length::FillPortion(1))
                .into(),
                Row::with_children(vec![
                    fetch_btn.into(),
                    save_btn.into(),
                ])
                .spacing(8)
                .align_y(Alignment::Center)
                .wrap()
                .into(),
            ])
            .spacing(12)
            .align_y(Alignment::Center)
            .wrap(),
        )
        .padding([12, 18])
        .width(Length::Fill);

        let body = responsive(move |size| {
            let is_wide = size.width >= 760.0;
            let right_section = match self.active_section {
                SettingsSection::ColorSource => self.view_color_source_section(pal),
                SettingsSection::Appearance => self.view_mode_and_scheme_section(pal),
                SettingsSection::Targets => self.view_targets_section(pal),
                SettingsSection::Advanced => self.view_advanced_section(pal),
            };

            if is_wide {
                let preview_sidebar = self.view_palette_preview(pal, true);

                let mut nav_col = column![].spacing(4).width(Length::Fill);
                for section in SettingsSection::ALL {
                    let is_active = self.active_section == section;
                    nav_col = nav_col.push(
                        button(text(section.label()).size(13))
                            .width(Length::Fill)
                            .padding([8, 12])
                            .style(move |_theme: &Theme, _status| button::Style {
                                background: Some(Background::Color(if is_active {
                                    pal.primary_container
                                } else {
                                    Color::TRANSPARENT
                                })),
                                text_color: if is_active {
                                    pal.on_primary_container
                                } else {
                                    pal.on_surface_var
                                },
                                border: Border {
                                    color: Color::TRANSPARENT,
                                    width: 0.0,
                                    radius: 10.0.into(),
                                },
                                ..Default::default()
                            })
                            .on_press(GuiMessage::SelectSection(section)),
                    );
                }

                let sidebar_inner = column![preview_sidebar, horizontal_rule(1), nav_col]
                    .spacing(12)
                    .width(Length::Fill);

                let sidebar = container(
                    scrollable(sidebar_inner).height(Length::Fill),
                )
                .width(Length::Fixed(272.0))
                .height(Length::Fill)
                .padding(14)
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(pal.surface)),
                    border: Border {
                        color: pal.outline,
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                });

                let right_canvas = scrollable(
                    container(right_section)
                        .padding(18)
                        .width(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill);

                row![sidebar, right_canvas]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            } else {
                // Narrow / tiling half-screen responsive layout: stacked top nav + palette + full-width section
                let nav_pills: Vec<Element<'_, GuiMessage>> = SettingsSection::ALL
                    .into_iter()
                    .map(|section| {
                        let is_active = self.active_section == section;
                        button(text(section.label()).size(12))
                            .padding([6, 10])
                            .style(move |_theme: &Theme, _status| button::Style {
                                background: Some(Background::Color(if is_active {
                                    pal.primary_container
                                } else {
                                    pal.surface_high
                                })),
                                text_color: if is_active {
                                    pal.on_primary_container
                                } else {
                                    pal.on_surface_var
                                },
                                border: Border {
                                    color: if is_active {
                                        pal.primary
                                    } else {
                                        pal.outline
                                    },
                                    width: 1.0,
                                    radius: 8.0.into(),
                                },
                                ..Default::default()
                            })
                            .on_press(GuiMessage::SelectSection(section))
                            .into()
                    })
                    .collect();

                let nav_wrap = Row::with_children(nav_pills).spacing(6).wrap();
                let compact_palette = self.view_palette_preview(pal, false);

                scrollable(
                    container(
                        column![nav_wrap, compact_palette, right_section]
                            .spacing(12)
                            .width(Length::Fill),
                    )
                    .padding(14)
                    .width(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            }
        });

        let mut status_row = row![
            text(&self.status_message)
                .size(12)
                .color(pal.on_surface_var)
                .width(Length::Fill),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        if self.has_unsaved_changes {
            status_row = status_row.push(
                container(text("Unsaved changes").size(11).color(pal.on_tertiary))
                    .padding([3, 10])
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(pal.tertiary)),
                        border: Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 999.0.into(),
                        },
                        ..Default::default()
                    }),
            );
        }

        let status_bar = container(status_row)
            .padding([8, 18])
            .width(Length::Fill);

        column![
            header,
            horizontal_rule(1),
            container(body).width(Length::Fill).height(Length::Fill),
            horizontal_rule(1),
            status_bar,
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_palette_preview(&self, pal: UiPalette, three_col: bool) -> Element<'_, GuiMessage> {
        let cols = if three_col { 3 } else { 5 };
        let mut grid_col = column![].spacing(6).width(Length::Fill);
        let mut current_row = row![].spacing(6).width(Length::Fill);

        for (idx, (role_key, role_label, on_role_key)) in PREVIEW_ROLES.iter().enumerate() {
            let bg_hex = self
                .active_role_hex(role_key)
                .unwrap_or_else(|| "#2b2930".to_string());
            let fg_hex = self
                .active_role_hex(on_role_key)
                .unwrap_or_else(|| "#e6e0e9".to_string());

            let swatch_bg = hex_to_iced_color(&bg_hex).unwrap_or(pal.surface_high);
            let swatch_fg = hex_to_iced_color(&fg_hex).unwrap_or(pal.on_surface);

            let short_label = match *role_label {
                "Background" => "Bg",
                "Surface Low" => "Surf Low",
                "Surface Container" => "Surf Cont",
                "Surface High" => "Surf High",
                other => other,
            };

            let inner_swatch = container(
                column![
                    text(short_label).size(10).color(swatch_fg),
                    text(bg_hex).size(9).color(swatch_fg),
                ]
                .spacing(1),
            )
            .padding([5, 6])
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(swatch_bg)),
                text_color: Some(swatch_fg),
                border: Border {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.08),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

            let outer_card = container(inner_swatch)
                .padding(4)
                .width(Length::FillPortion(1))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(pal.surface_high)),
                    border: Border {
                        color: pal.outline,
                        width: 1.0,
                        radius: 10.0.into(),
                    },
                    ..Default::default()
                });

            current_row = current_row.push(outer_card);
            if idx % cols == cols - 1 || idx == PREVIEW_ROLES.len() - 1 {
                grid_col = grid_col.push(current_row);
                current_row = row![].spacing(6).width(Length::Fill);
            }
        }

        column![
            text("Live Material You Palette").size(13).color(pal.on_surface),
            grid_col,
        ]
        .spacing(6)
        .into()
    }

    fn view_color_source_section(&self, pal: UiPalette) -> Element<'_, GuiMessage> {
        let using_wallpaper = self.config.color.is_none();
        let custom_color_val = self.config.color.as_deref().unwrap_or("");
        let monitor_str = self.config.monitor.to_string();

        let wallpaper_btn: Element<'_, GuiMessage> = button(text("Wallpaper").size(13))
            .padding([7, 14])
            .style(move |_theme: &Theme, _status| button::Style {
                background: Some(Background::Color(if using_wallpaper {
                    pal.primary
                } else {
                    pal.surface_high
                })),
                text_color: if using_wallpaper {
                    pal.on_primary
                } else {
                    pal.on_surface_var
                },
                border: Border {
                    color: if using_wallpaper {
                        Color::TRANSPARENT
                    } else {
                        pal.outline
                    },
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
            .on_press(GuiMessage::UseWallpaperSource(true))
            .into();

        let custom_btn: Element<'_, GuiMessage> = button(text("Custom Hex").size(13))
            .padding([7, 14])
            .style(move |_theme: &Theme, _status| button::Style {
                background: Some(Background::Color(if !using_wallpaper {
                    pal.primary
                } else {
                    pal.surface_high
                })),
                text_color: if !using_wallpaper {
                    pal.on_primary
                } else {
                    pal.on_surface_var
                },
                border: Border {
                    color: if !using_wallpaper {
                        Color::TRANSPARENT
                    } else {
                        pal.outline
                    },
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
            .on_press(GuiMessage::UseWallpaperSource(false))
            .into();

        let source_segmented: Element<'_, GuiMessage> =
            container(row![wallpaper_btn, custom_btn].spacing(4))
                .padding(3)
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(pal.surface_high)),
                    border: Border {
                        color: pal.outline,
                        width: 1.0,
                        radius: 10.0.into(),
                    },
                    ..Default::default()
                })
                .into();

        let monitor_stepper: Element<'_, GuiMessage> = container(
            row![
                button(text("−").size(14))
                    .padding([5, 11])
                    .style(move |_theme: &Theme, _status| button::Style {
                        background: Some(Background::Color(pal.surface_high)),
                        text_color: pal.on_surface,
                        border: Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 7.0.into(),
                        },
                        ..Default::default()
                    })
                    .on_press(GuiMessage::MonitorChanged(
                        self.config.monitor.saturating_sub(1)
                    )),
                text_input("0", &monitor_str)
                    .on_input(GuiMessage::MonitorInputChanged)
                    .width(Length::Fixed(48.0))
                    .padding([5, 8])
                    .style(move |_theme, status| pal.text_input_style(status, false)),
                button(text("+").size(14))
                    .padding([5, 11])
                    .style(move |_theme: &Theme, _status| button::Style {
                        background: Some(Background::Color(pal.surface_high)),
                        text_color: pal.on_surface,
                        border: Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 7.0.into(),
                        },
                        ..Default::default()
                    })
                    .on_press(GuiMessage::MonitorChanged(self.config.monitor + 1)),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .padding(3)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(pal.surface_high)),
            border: Border {
                color: pal.outline,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        })
        .into();

        let top_controls = Row::with_children(vec![
            column![
                text("Source mode").size(12).color(pal.on_surface_var),
                source_segmented,
            ]
            .spacing(5)
            .into(),
            column![
                text("Monitor index").size(12).color(pal.on_surface_var),
                monitor_stepper,
            ]
            .spacing(5)
            .into(),
        ])
        .spacing(16)
        .align_y(Alignment::End)
        .wrap();

        let mut custom_col = column![].spacing(6);
        if !using_wallpaper {
            let parsed_custom = normalize_color(custom_color_val)
                .as_deref()
                .and_then(hex_to_iced_color);
            let preview_color = parsed_custom.unwrap_or(Color::TRANSPARENT);
            let is_invalid = !custom_color_val.trim().is_empty() && parsed_custom.is_none();

            let swatch_dot = container(Space::with_width(Length::Fixed(24.0)))
                .height(Length::Fixed(24.0))
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(preview_color)),
                    border: Border {
                        color: if is_invalid { pal.error } else { pal.outline },
                        width: 1.5,
                        radius: 7.0.into(),
                    },
                    ..Default::default()
                });

            custom_col = custom_col.push(
                row![
                    text("Custom seed hex:").size(13).color(pal.on_surface_var),
                    swatch_dot,
                    text_input("#4285f4", custom_color_val)
                        .on_input(GuiMessage::CustomColorChanged)
                        .width(Length::Fixed(140.0))
                        .padding([6, 10])
                        .style(move |_theme, status| pal.text_input_style(status, is_invalid)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );

            if is_invalid {
                custom_col = custom_col.push(
                    text("Invalid hex color (use #RGB, #RRGGBB, or RRGGBB)")
                        .size(12)
                        .color(pal.error),
                );
            }
        }

        let best_colors: Vec<String> = self
            .current_scheme
            .as_ref()
            .map(|s| s.best_list.clone())
            .unwrap_or_else(|| vec!["#4285f4".to_string()]);

        let seed_pills: Vec<Element<'_, GuiMessage>> = best_colors
            .into_iter()
            .take(7)
            .enumerate()
            .map(|(idx, hex_str)| {
                let is_selected = (self.config.ncolor as usize) == idx && self.config.color.is_none();
                let swatch_color =
                    hex_to_iced_color(&hex_str).unwrap_or(Color::from_rgb8(66, 133, 244));

                let circle = container(Space::with_width(Length::Fixed(16.0)))
                    .height(Length::Fixed(16.0))
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(swatch_color)),
                        border: Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.35),
                            width: 1.0,
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    });

                let label = format!("#{} {}", idx + 1, hex_str);
                let bg = if is_selected {
                    pal.primary_container
                } else {
                    pal.surface_high
                };
                let fg = if is_selected {
                    pal.on_primary_container
                } else {
                    pal.on_surface
                };
                let border_c = if is_selected {
                    pal.primary
                } else {
                    pal.outline
                };

                button(
                    row![circle, text(label).size(12)]
                        .spacing(7)
                        .align_y(Alignment::Center),
                )
                .padding([6, 11])
                .style(move |_theme: &Theme, _status| button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: fg,
                    border: Border {
                        color: border_c,
                        width: if is_selected { 1.5 } else { 1.0 },
                        radius: 999.0.into(),
                    },
                    ..Default::default()
                })
                .on_press(GuiMessage::SeedIndexSelected(idx as u32))
                .into()
            })
            .collect();

        let seed_pills_wrap = Row::with_children(seed_pills).spacing(8).wrap();

        Self::styled_section_card(
            column![
                text("Color Source & Wallpaper Seeds").size(16),
                top_controls,
                custom_col,
                text("Extracted wallpaper seeds")
                    .size(12)
                    .color(pal.on_surface_var),
                seed_pills_wrap,
            ]
            .spacing(12)
            .into(),
            pal,
        )
    }

    fn view_mode_and_scheme_section(&self, pal: UiPalette) -> Element<'_, GuiMessage> {
        let plasma_mode = ThemeMode::from_plasma_config(&self.config);
        let pywal_mode = ThemeMode::from_pywal_config(&self.config);
        let current_variant = SchemeVariant::from_u8(self.config.scheme_variant);

        let modes_wrap = Row::with_children(vec![
            column![
                text("Plasma mode").size(12).color(pal.on_surface_var),
                pick_list(
                    ThemeMode::ALL.as_slice(),
                    Some(plasma_mode),
                    GuiMessage::PlasmaModeChanged,
                )
                .padding([7, 10])
                .style(move |_theme, status| pal.pick_list_style(status)),
            ]
            .spacing(5)
            .into(),
            column![
                text("Konsole / Pywal / KSyntax")
                    .size(12)
                    .color(pal.on_surface_var),
                pick_list(
                    ThemeMode::ALL.as_slice(),
                    Some(pywal_mode),
                    GuiMessage::PywalModeChanged,
                )
                .padding([7, 10])
                .style(move |_theme, status| pal.pick_list_style(status)),
            ]
            .spacing(5)
            .into(),
            column![
                text("Palette style").size(12).color(pal.on_surface_var),
                pick_list(
                    ALL_SCHEME_VARIANTS.as_slice(),
                    Some(current_variant),
                    GuiMessage::SchemeVariantChanged,
                )
                .padding([7, 10])
                .style(move |_theme, status| pal.pick_list_style(status)),
            ]
            .spacing(5)
            .into(),
        ])
        .spacing(14)
        .align_y(Alignment::Center)
        .wrap();

        let is_2025 = self.config.spec_version == "2025";
        let spec_row = Row::with_children(vec![
            text("Material spec version:")
                .size(13)
                .color(pal.on_surface_var)
                .into(),
            button(text("2025 (M3 Expressive)").size(12))
                .padding([6, 12])
                .style(move |_theme: &Theme, _status| button::Style {
                    background: Some(Background::Color(if is_2025 {
                        pal.primary_container
                    } else {
                        pal.surface_high
                    })),
                    text_color: if is_2025 {
                        pal.on_primary_container
                    } else {
                        pal.on_surface
                    },
                    border: Border {
                        color: if is_2025 { pal.primary } else { pal.outline },
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                })
                .on_press(GuiMessage::SpecVersionChanged("2025".to_string()))
                .into(),
            button(text("2021 (Classic M3)").size(12))
                .padding([6, 12])
                .style(move |_theme: &Theme, _status| button::Style {
                    background: Some(Background::Color(if !is_2025 {
                        pal.primary_container
                    } else {
                        pal.surface_high
                    })),
                    text_color: if !is_2025 {
                        pal.on_primary_container
                    } else {
                        pal.on_surface
                    },
                    border: Border {
                        color: if !is_2025 { pal.primary } else { pal.outline },
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                })
                .on_press(GuiMessage::SpecVersionChanged("2021".to_string()))
                .into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center)
        .wrap();

        let sliders_col = column![
            Self::multiplier_slider_row(
                pal,
                "Chroma multiplier",
                self.config.chroma_multiplier,
                &self.chroma_input,
                1.0,
                0.0..=4.0,
                GuiMessage::ChromaMultiplierChanged,
                GuiMessage::ChromaInputChanged,
                GuiMessage::ResetChromaMultiplier,
            ),
            Self::multiplier_slider_row(
                pal,
                "Tone / lightness",
                self.config.tone_multiplier,
                &self.tone_input,
                1.0,
                0.0..=1.5,
                GuiMessage::ToneMultiplierChanged,
                GuiMessage::ToneInputChanged,
                GuiMessage::ResetToneMultiplier,
            ),
            Self::multiplier_slider_row(
                pal,
                "Contrast level",
                self.config.contrast_level,
                &self.contrast_input,
                0.0,
                -1.0..=1.0,
                GuiMessage::ContrastLevelChanged,
                GuiMessage::ContrastInputChanged,
                GuiMessage::ResetContrastLevel,
            ),
            Self::multiplier_slider_row(
                pal,
                "Frame contrast",
                self.config.frame_contrast,
                &self.frame_contrast_input,
                0.0,
                0.0..=1.0,
                GuiMessage::FrameContrastChanged,
                GuiMessage::FrameContrastInputChanged,
                GuiMessage::ResetFrameContrast,
            ),
        ]
        .spacing(10);

        Self::styled_section_card(
            column![
                text("Appearance & Scheme Multipliers").size(16),
                modes_wrap,
                spec_row,
                horizontal_rule(1),
                sliders_col,
            ]
            .spacing(14)
            .into(),
            pal,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn multiplier_slider_row<'a>(
        pal: UiPalette,
        label: &'a str,
        value: f64,
        input_str: &str,
        default_val: f64,
        range: std::ops::RangeInclusive<f64>,
        on_change: fn(f64) -> GuiMessage,
        on_input: fn(String) -> GuiMessage,
        on_reset: GuiMessage,
    ) -> Element<'a, GuiMessage> {
        let is_invalid = input_str.trim().parse::<f64>().is_err();

        let reset_slot: Element<'a, GuiMessage> = if (value - default_val).abs() > 0.01 {
            button(text("Reset").size(11))
                .width(Length::Fixed(52.0))
                .padding([5, 8])
                .style(move |_theme: &Theme, _status| button::Style {
                    background: Some(Background::Color(pal.surface_high)),
                    text_color: pal.on_surface_var,
                    border: Border {
                        color: pal.outline,
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                })
                .on_press(on_reset)
                .into()
        } else {
            Space::with_width(Length::Fixed(52.0)).into()
        };

        let controls = row![
            slider(range, value, on_change)
                .step(0.05)
                .width(Length::Fill)
                .style(move |_theme, status| pal.slider_style(status)),
            text_input("0.00", input_str)
                .on_input(on_input)
                .width(Length::Fixed(64.0))
                .padding([5, 8])
                .style(move |_theme, status| pal.text_input_style(status, is_invalid)),
            reset_slot,
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        Row::with_children(vec![
            text(label)
                .size(13)
                .color(pal.on_surface_var)
                .width(Length::Fixed(140.0))
                .into(),
            controls.width(Length::FillPortion(1)).into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center)
        .wrap()
        .into()
    }

    fn view_targets_section(&self, pal: UiPalette) -> Element<'_, GuiMessage> {
        let apply_konsole = !self.config.disable_konsole;

        let mut konsole_col = column![
            checkbox("Apply theme to Konsole", apply_konsole)
                .style(move |_theme, status| pal.checkbox_style(status))
                .on_toggle(GuiMessage::ApplyKonsoleChanged),
        ]
        .spacing(8);

        if apply_konsole {
            konsole_col = konsole_col.push(
                container(
                    column![
                        checkbox("Enable Konsole background blur", self.config.konsole_blur)
                            .style(move |_theme, status| pal.checkbox_style(status))
                            .on_toggle(GuiMessage::KonsoleBlurChanged),
                        Row::with_children(vec![
                            row![
                                text(format!("Light opacity: {}%", self.config.konsole_opacity))
                                    .width(Length::Fixed(130.0))
                                    .size(13)
                                    .color(pal.on_surface_var),
                                slider(
                                    0..=100,
                                    self.config.konsole_opacity,
                                    GuiMessage::KonsoleOpacityChanged,
                                )
                                .width(Length::Fixed(130.0))
                                .style(move |_theme, status| pal.slider_style(status)),
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center)
                            .into(),
                            row![
                                text(format!(
                                    "Dark opacity: {}%",
                                    self.config.konsole_opacity_dark
                                ))
                                .width(Length::Fixed(130.0))
                                .size(13)
                                .color(pal.on_surface_var),
                                slider(
                                    0..=100,
                                    self.config.konsole_opacity_dark,
                                    GuiMessage::KonsoleOpacityDarkChanged,
                                )
                                .width(Length::Fixed(130.0))
                                .style(move |_theme, status| pal.slider_style(status)),
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center)
                            .into(),
                        ])
                        .spacing(14)
                        .wrap(),
                    ]
                    .spacing(8),
                )
                .padding(iced::Padding {
                    top: 4.0,
                    right: 0.0,
                    bottom: 4.0,
                    left: 24.0,
                }),
            );
        }

        let mut titlebar_col = column![
            checkbox(
                "Override window titlebar opacity",
                self.config.titlebar_opacity_override
            )
            .style(move |_theme, status| pal.checkbox_style(status))
            .on_toggle(GuiMessage::TitlebarOpacityOverrideChanged),
        ]
        .spacing(8);

        if self.config.titlebar_opacity_override {
            titlebar_col = titlebar_col.push(
                container(
                    Row::with_children(vec![
                        row![
                            text(format!("Light: {}%", self.config.titlebar_opacity))
                                .width(Length::Fixed(95.0))
                                .size(13)
                                .color(pal.on_surface_var),
                            slider(
                                0..=100,
                                self.config.titlebar_opacity,
                                GuiMessage::TitlebarOpacityChanged,
                            )
                            .width(Length::Fixed(130.0))
                            .style(move |_theme, status| pal.slider_style(status)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .into(),
                        row![
                            text(format!("Dark: {}%", self.config.titlebar_opacity_dark))
                                .width(Length::Fixed(95.0))
                                .size(13)
                                .color(pal.on_surface_var),
                            slider(
                                0..=100,
                                self.config.titlebar_opacity_dark,
                                GuiMessage::TitlebarOpacityDarkChanged,
                            )
                            .width(Length::Fixed(130.0))
                            .style(move |_theme, status| pal.slider_style(status)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .into(),
                    ])
                    .spacing(14)
                    .wrap(),
                )
                .padding(iced::Padding {
                    top: 4.0,
                    right: 0.0,
                    bottom: 4.0,
                    left: 24.0,
                }),
            );
        }

        let mut toolbar_col = column![
            checkbox(
                "Override application toolbar opacity (Breeze)",
                self.config.toolbar_opacity_override
            )
            .style(move |_theme, status| pal.checkbox_style(status))
            .on_toggle(GuiMessage::ToolbarOpacityOverrideChanged),
        ]
        .spacing(8);

        if self.config.toolbar_opacity_override {
            toolbar_col = toolbar_col.push(
                container(
                    Row::with_children(vec![
                        row![
                            text(format!("Light: {}%", self.config.toolbar_opacity))
                                .width(Length::Fixed(95.0))
                                .size(13)
                                .color(pal.on_surface_var),
                            slider(
                                0..=100,
                                self.config.toolbar_opacity,
                                GuiMessage::ToolbarOpacityChanged,
                            )
                            .width(Length::Fixed(130.0))
                            .style(move |_theme, status| pal.slider_style(status)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .into(),
                        row![
                            text(format!("Dark: {}%", self.config.toolbar_opacity_dark))
                                .width(Length::Fixed(95.0))
                                .size(13)
                                .color(pal.on_surface_var),
                            slider(
                                0..=100,
                                self.config.toolbar_opacity_dark,
                                GuiMessage::ToolbarOpacityDarkChanged,
                            )
                            .width(Length::Fixed(130.0))
                            .style(move |_theme, status| pal.slider_style(status)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .into(),
                    ])
                    .spacing(14)
                    .wrap(),
                )
                .padding(iced::Padding {
                    top: 4.0,
                    right: 0.0,
                    bottom: 4.0,
                    left: 24.0,
                }),
            );
        }

        let integrations_wrap = Row::with_children(vec![
            checkbox("Export Pywal palette files", self.config.pywal)
                .style(move |_theme, status| pal.checkbox_style(status))
                .on_toggle(GuiMessage::ApplyPywalChanged)
                .into(),
            checkbox("Export Kitty theme files", self.config.kitty)
                .style(move |_theme, status| pal.checkbox_style(status))
                .on_toggle(GuiMessage::ApplyKittyChanged)
                .into(),
            checkbox(
                "Tint Sierra Breeze window buttons",
                self.config.sierra_breeze_buttons_color,
            )
            .style(move |_theme, status| pal.checkbox_style(status))
            .on_toggle(GuiMessage::SierraBreezeButtonsChanged)
            .into(),
            checkbox(
                "Tint Klassy window decoration outline",
                self.config.klassy_windeco_outline,
            )
            .style(move |_theme, status| pal.checkbox_style(status))
            .on_toggle(GuiMessage::KlassyWindecoOutlineChanged)
            .into(),
            checkbox(
                "Tint KDE Rounded Corners outline",
                self.config.kde_rounded_corners_effect_outline,
            )
            .style(move |_theme, status| pal.checkbox_style(status))
            .on_toggle(GuiMessage::KdeRoundedCornersOutlineChanged)
            .into(),
        ])
        .spacing(14)
        .align_y(Alignment::Center)
        .wrap();

        Self::styled_section_card(
            column![
                text("Theme Targets & Window Decorations").size(16),
                konsole_col,
                horizontal_rule(1),
                titlebar_col,
                toolbar_col,
                horizontal_rule(1),
                text("Additional exports & decorations")
                    .size(13)
                    .color(pal.on_surface_var),
                integrations_wrap,
            ]
            .spacing(12)
            .into(),
            pal,
        )
    }

    fn view_advanced_section(&self, pal: UiPalette) -> Element<'_, GuiMessage> {
        let darker_list = self.config.darker_window_list.as_deref().unwrap_or("");
        let icons_light = self.config.iconslight.as_deref().unwrap_or("");
        let icons_dark = self.config.iconsdark.as_deref().unwrap_or("");
        let hook = self.config.on_change_hook.as_deref().unwrap_or("");

        let inputs_col = column![
            column![
                text("Darker titlebar window classes")
                    .size(12)
                    .color(pal.on_surface_var),
                text_input("konsole Alacritty kitty", darker_list)
                    .on_input(GuiMessage::DarkerWindowListChanged)
                    .padding([7, 10])
                    .style(move |_theme, status| pal.text_input_style(status, false))
                    .width(Length::Fill),
            ]
            .spacing(4),
            Row::with_children(vec![
                column![
                    text("Light mode icon theme")
                        .size(12)
                        .color(pal.on_surface_var),
                    text_input("breeze", icons_light)
                        .on_input(GuiMessage::IconsLightChanged)
                        .padding([7, 10])
                        .style(move |_theme, status| pal.text_input_style(status, false))
                        .width(Length::Fixed(210.0)),
                ]
                .spacing(4)
                .into(),
                column![
                    text("Dark mode icon theme")
                        .size(12)
                        .color(pal.on_surface_var),
                    text_input("breeze-dark", icons_dark)
                        .on_input(GuiMessage::IconsDarkChanged)
                        .padding([7, 10])
                        .style(move |_theme, status| pal.text_input_style(status, false))
                        .width(Length::Fixed(210.0)),
                ]
                .spacing(4)
                .into(),
            ])
            .spacing(14)
            .wrap(),
            column![
                text("On-change hook command / script")
                    .size(12)
                    .color(pal.on_surface_var),
                text_input("/path/to/hook.sh", hook)
                    .on_input(GuiMessage::OnChangeHookChanged)
                    .padding([7, 10])
                    .style(move |_theme, status| pal.text_input_style(status, false))
                    .width(Length::Fill),
            ]
            .spacing(4),
        ]
        .spacing(12);

        let flags_wrap = Row::with_children(vec![
            checkbox("Pause automatic theme updates", self.config.pause_mode)
                .style(move |_theme, status| pal.checkbox_style(status))
                .on_toggle(GuiMessage::PauseModeChanged)
                .into(),
            checkbox(
                "Force KWin screenshot capture mode",
                self.config.screenshot_only_mode,
            )
            .style(move |_theme, status| pal.checkbox_style(status))
            .on_toggle(GuiMessage::ScreenshotOnlyModeChanged)
            .into(),
            checkbox("Manual wallpaper fetch only", self.config.manual_fetch)
                .style(move |_theme, status| pal.checkbox_style(status))
                .on_toggle(GuiMessage::ManualFetchChanged)
                .into(),
        ])
        .spacing(14)
        .align_y(Alignment::Center)
        .wrap();

        let advanced_card = Self::styled_section_card(
            column![
                text("Advanced Automation & Hooks").size(16),
                inputs_col,
                horizontal_rule(1),
                flags_wrap,
            ]
            .spacing(14)
            .into(),
            pal,
        );

        let mut theme_mode_pills = row![].spacing(4).align_y(Alignment::Center);
        for mode in GuiThemeMode::ALL {
            let selected = self.gui_theme_mode == mode;
            theme_mode_pills = theme_mode_pills.push(
                button(text(mode.label()).size(12))
                    .padding([6, 12])
                    .style(move |_theme: &Theme, _status| button::Style {
                        background: Some(Background::Color(if selected {
                            pal.primary
                        } else {
                            Color::TRANSPARENT
                        })),
                        text_color: if selected {
                            pal.on_primary
                        } else {
                            pal.on_surface_var
                        },
                        border: Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 7.0.into(),
                        },
                        ..Default::default()
                    })
                    .on_press(GuiMessage::GuiThemeModeChanged(mode)),
            );
        }

        let gui_theme_segmented: Element<'_, GuiMessage> = container(theme_mode_pills)
            .padding(3)
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(pal.surface_high)),
                border: Border {
                    color: pal.outline,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            })
            .into();

        let misc_card = Self::styled_section_card(
            column![
                text("Misc · Settings Window").size(16),
                text(
                    "Controls only the appearance of this Settings window. \
                     To change your KDE Plasma or terminal theme mode, use Appearance & Multipliers."
                )
                .size(12)
                .color(pal.on_surface_var),
                Row::with_children(vec![
                    text("Settings window theme")
                        .size(13)
                        .color(pal.on_surface)
                        .into(),
                    gui_theme_segmented,
                ])
                .spacing(12)
                .align_y(Alignment::Center)
                .wrap(),
            ]
            .spacing(10)
            .into(),
            pal,
        );

        column![advanced_card, misc_card]
            .spacing(14)
            .width(Length::Fill)
            .into()
    }

    fn styled_section_card<'a>(
        inner: Element<'a, GuiMessage>,
        pal: UiPalette,
    ) -> Element<'a, GuiMessage> {
        container(inner)
            .padding(SECTION_CARD_PADDING)
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(pal.surface)),
                border: Border {
                    color: pal.outline,
                    width: 1.0,
                    radius: SECTION_OUTER_RADIUS.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

#[derive(Debug, Clone, Copy)]
struct UiPalette {
    background: Color,
    surface: Color,
    surface_high: Color,
    surface_highest: Color,
    on_surface: Color,
    on_surface_var: Color,
    outline: Color,
    primary: Color,
    on_primary: Color,
    primary_container: Color,
    on_primary_container: Color,
    tertiary: Color,
    on_tertiary: Color,
    error: Color,
}

impl UiPalette {
    fn text_input_style(&self, status: text_input::Status, is_invalid: bool) -> text_input::Style {
        let border_color = if is_invalid {
            self.error
        } else {
            match status {
                text_input::Status::Focused => self.primary,
                text_input::Status::Hovered => self.on_surface_var,
                _ => self.outline,
            }
        };

        text_input::Style {
            background: Background::Color(self.surface_high),
            border: Border {
                color: border_color,
                width: if matches!(status, text_input::Status::Focused) || is_invalid {
                    1.5
                } else {
                    1.0
                },
                radius: 10.0.into(),
            },
            icon: self.on_surface_var,
            placeholder: self.on_surface_var,
            value: self.on_surface,
            selection: self.primary_container,
        }
    }

    fn slider_style(&self, status: slider::Status) -> slider::Style {
        let handle_bg = match status {
            slider::Status::Hovered | slider::Status::Dragged => self.primary_container,
            _ => self.primary,
        };
        let handle_border = match status {
            slider::Status::Hovered | slider::Status::Dragged => self.primary,
            _ => self.background,
        };

        slider::Style {
            rail: slider::Rail {
                backgrounds: (
                    Background::Color(self.primary),
                    Background::Color(self.surface_highest),
                ),
                width: 5.0,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 3.0.into(),
                },
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 8.0 },
                background: Background::Color(handle_bg),
                border_width: 1.5,
                border_color: handle_border,
            },
        }
    }

    fn pick_list_style(&self, status: pick_list::Status) -> pick_list::Style {
        let border_color = match status {
            pick_list::Status::Hovered | pick_list::Status::Opened => self.primary,
            _ => self.outline,
        };
        pick_list::Style {
            text_color: self.on_surface,
            placeholder_color: self.on_surface_var,
            handle_color: self.on_surface_var,
            background: Background::Color(self.surface_high),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 10.0.into(),
            },
        }
    }

    fn checkbox_style(&self, status: checkbox::Status) -> checkbox::Style {
        let (is_checked, is_hovered) = match status {
            checkbox::Status::Active { is_checked } => (is_checked, false),
            checkbox::Status::Hovered { is_checked } => (is_checked, true),
            checkbox::Status::Disabled { is_checked } => (is_checked, false),
        };
        checkbox::Style {
            background: Background::Color(if is_checked {
                self.primary
            } else {
                self.surface_high
            }),
            icon_color: self.on_primary,
            border: Border {
                color: if is_checked || is_hovered {
                    self.primary
                } else {
                    self.outline
                },
                width: 1.2,
                radius: 5.0.into(),
            },
            text_color: Some(self.on_surface),
        }
    }
}

/// Launch the standalone `iced` Material You configuration and live palette preview window.
pub fn run_settings_window(config_path: PathBuf, runtime_json_path: PathBuf) -> Result<()> {
    iced::application(
        "KDE Material You Colors — Settings",
        SettingsGui::update,
        SettingsGui::view,
    )
    .window(iced::window::Settings {
        size: Size::new(960.0, 680.0),
        min_size: Some(Size::new(360.0, 420.0)),
        ..Default::default()
    })
    .theme(SettingsGui::theme)
    .run_with(move || {
        (
            SettingsGui::new_with_paths(config_path, runtime_json_path),
            Task::none(),
        )
    })
    .map_err(|e| anyhow::anyhow!("iced settings window exited with error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_hex_to_iced_color_parsing() {
        let blue = hex_to_iced_color("#4285f4").expect("valid 6-digit hex");
        assert!((blue.r - (0x42 as f32 / 255.0)).abs() < 1e-4);
        assert!((blue.g - (0x85 as f32 / 255.0)).abs() < 1e-4);
        assert!((blue.b - (0xf4 as f32 / 255.0)).abs() < 1e-4);
        assert!((blue.a - 1.0).abs() < 1e-4);

        let short = hex_to_iced_color("#f0a").expect("valid 3-digit hex");
        assert!((short.r - 1.0).abs() < 1e-4);
        assert!((short.g - 0.0).abs() < 1e-4);
        assert!((short.b - (0xaa as f32 / 255.0)).abs() < 1e-4);

        let with_alpha = hex_to_iced_color("#80ff0000").expect("valid 8-digit ARGB hex");
        assert!((with_alpha.r - 1.0).abs() < 1e-4);
        assert!((with_alpha.g - 0.0).abs() < 1e-4);
        assert!((with_alpha.a - (0x80 as f32 / 255.0)).abs() < 1e-4);

        assert!(hex_to_iced_color("invalid-hex").is_none());
        assert!(hex_to_iced_color("#12345").is_none());
    }

    #[test]
    fn test_settings_gui_initialization_and_concentric_radii() {
        let dir = tempdir().expect("create tempdir");
        let cfg_path = dir.path().join("config.conf");
        let json_path = dir.path().join("runtime.json");

        let gui = SettingsGui::new_with_paths(cfg_path.clone(), json_path.clone());
        assert_eq!(gui.config_path, cfg_path);
        assert_eq!(gui.runtime_json_path, json_path);
        assert!(!gui.show_advanced);
        assert!(gui.current_scheme.is_some());

        // Verify concentric border radius invariant (outer = inner + padding)
        assert!((SWATCH_OUTER_RADIUS - (SWATCH_INNER_RADIUS + SWATCH_CARD_PADDING)).abs() < f32::EPSILON);
        assert!((SECTION_OUTER_RADIUS - (SECTION_INNER_RADIUS + SECTION_CARD_PADDING)).abs() < f32::EPSILON);

        // Verify all 9 preview roles resolve valid hex strings from the generated scheme
        for (role_key, _, on_role_key) in PREVIEW_ROLES {
            let role_hex = gui
                .active_role_hex(role_key)
                .unwrap_or_else(|| panic!("missing preview role {role_key}"));
            let on_role_hex = gui
                .active_role_hex(on_role_key)
                .unwrap_or_else(|| panic!("missing preview on-role {on_role_key}"));
            assert!(hex_to_iced_color(&role_hex).is_some());
            assert!(hex_to_iced_color(&on_role_hex).is_some());
        }
    }

    #[test]
    fn test_settings_gui_update_transitions_and_save_reload() {
        let dir = tempdir().expect("create tempdir");
        let cfg_path = dir.path().join("config.conf");
        let json_path = dir.path().join("kde-material-you-colors-test.json");

        let mut gui = SettingsGui::new_with_paths(cfg_path.clone(), json_path.clone());
        let initial_primary = gui.active_role_hex("primary").unwrap();

        // 1. Change custom hex color & scheme variant -> live preview updates
        let _ = gui.update(GuiMessage::UseWallpaperSource(false));
        let _ = gui.update(GuiMessage::CustomColorChanged("#e91e63".to_string()));
        let _ = gui.update(GuiMessage::SchemeVariantChanged(SchemeVariant::Vibrant));
        assert_eq!(gui.config.color.as_deref(), Some("#e91e63"));
        assert_eq!(gui.config.scheme_variant, SchemeVariant::Vibrant.as_u8());
        let updated_primary = gui.active_role_hex("primary").unwrap();
        assert_ne!(initial_primary, updated_primary);

        // 2. Toggle Dark / Light / Follow Color Scheme modes
        let _ = gui.update(GuiMessage::PlasmaModeChanged(ThemeMode::Light));
        assert!(!gui.config.plasma_follows_scheme);
        assert_eq!(gui.config.light, Some(true));
        assert!(gui.is_light_theme());
        assert!(gui.has_unsaved_changes);

        let _ = gui.update(GuiMessage::PlasmaModeChanged(ThemeMode::FollowScheme));
        assert!(gui.config.plasma_follows_scheme);
        assert_eq!(gui.config.light, None);

        let _ = gui.update(GuiMessage::PywalModeChanged(ThemeMode::Dark));
        assert!(!gui.config.pywal_follows_scheme);
        assert_eq!(gui.config.pywal_light, Some(false));

        // 3. Multipliers & spec version
        let _ = gui.update(GuiMessage::ChromaMultiplierChanged(2.25));
        let _ = gui.update(GuiMessage::ToneMultiplierChanged(1.15));
        let _ = gui.update(GuiMessage::ContrastLevelChanged(0.5));
        let _ = gui.update(GuiMessage::FrameContrastChanged(0.35));
        let _ = gui.update(GuiMessage::SpecVersionChanged("2021".to_string()));
        assert!((gui.config.chroma_multiplier - 2.25).abs() < 1e-6);
        assert!((gui.config.tone_multiplier - 1.15).abs() < 1e-6);
        assert!((gui.config.contrast_level - 0.5).abs() < 1e-6);
        assert!((gui.config.frame_contrast - 0.35).abs() < 1e-6);
        assert_eq!(gui.config.spec_version, "2021");

        // 4. Targets & Advanced settings
        let _ = gui.update(GuiMessage::ToggleAdvanced(true));
        assert!(gui.show_advanced);
        let _ = gui.update(GuiMessage::ApplyKonsoleChanged(false));
        assert!(gui.config.disable_konsole);
        let _ = gui.update(GuiMessage::KonsoleOpacityChanged(85));
        let _ = gui.update(GuiMessage::KonsoleOpacityDarkChanged(75));
        let _ = gui.update(GuiMessage::KonsoleBlurChanged(true));
        let _ = gui.update(GuiMessage::ApplyPywalChanged(true));
        let _ = gui.update(GuiMessage::ApplyKittyChanged(true));
        let _ = gui.update(GuiMessage::TitlebarOpacityOverrideChanged(true));
        let _ = gui.update(GuiMessage::TitlebarOpacityChanged(90));
        let _ = gui.update(GuiMessage::TitlebarOpacityDarkChanged(80));
        let _ = gui.update(GuiMessage::ToolbarOpacityOverrideChanged(true));
        let _ = gui.update(GuiMessage::ToolbarOpacityChanged(95));
        let _ = gui.update(GuiMessage::ToolbarOpacityDarkChanged(88));
        let _ = gui.update(GuiMessage::SierraBreezeButtonsChanged(true));
        let _ = gui.update(GuiMessage::KlassyWindecoOutlineChanged(true));
        let _ = gui.update(GuiMessage::KdeRoundedCornersOutlineChanged(true));
        let _ = gui.update(GuiMessage::DarkerWindowListChanged("konsole kitty".to_string()));
        let _ = gui.update(GuiMessage::IconsLightChanged("Papirus-Light".to_string()));
        let _ = gui.update(GuiMessage::IconsDarkChanged("Papirus-Dark".to_string()));
        let _ = gui.update(GuiMessage::OnChangeHookChanged("/usr/local/bin/hook.sh".to_string()));
        let _ = gui.update(GuiMessage::PauseModeChanged(true));
        let _ = gui.update(GuiMessage::ScreenshotOnlyModeChanged(true));
        let _ = gui.update(GuiMessage::ManualFetchChanged(true));

        // 5. Save & Apply writes config.conf and .reload trigger file
        let _ = gui.update(GuiMessage::SaveAndApply);
        assert!(cfg_path.exists(), "config.conf must be created on SaveAndApply");
        let reload_path = gui.reload_trigger_path();
        assert!(
            reload_path.exists(),
            "reload trigger file must be created on SaveAndApply"
        );

        let reloaded_cfg = AppConfig::load_from_path(&cfg_path).expect("reload saved config.conf");
        assert_eq!(reloaded_cfg.scheme_variant, SchemeVariant::Vibrant.as_u8());
        assert_eq!(reloaded_cfg.color.as_deref(), Some("#e91e63"));
        assert!(reloaded_cfg.disable_konsole);
        assert_eq!(reloaded_cfg.konsole_opacity, 85);
        assert_eq!(reloaded_cfg.konsole_opacity_dark, 75);
        assert!(reloaded_cfg.konsole_blur);
        assert!(reloaded_cfg.pywal);
        assert!(reloaded_cfg.kitty);
        assert!(reloaded_cfg.titlebar_opacity_override);
        assert_eq!(reloaded_cfg.titlebar_opacity, 90);
        assert_eq!(reloaded_cfg.titlebar_opacity_dark, 80);
        assert!(reloaded_cfg.toolbar_opacity_override);
        assert_eq!(reloaded_cfg.toolbar_opacity, 95);
        assert_eq!(reloaded_cfg.toolbar_opacity_dark, 88);
        assert!(reloaded_cfg.sierra_breeze_buttons_color);
        assert!(reloaded_cfg.klassy_windeco_outline);
        assert!(reloaded_cfg.kde_rounded_corners_effect_outline);
        assert_eq!(
            reloaded_cfg.darker_window_list.as_deref(),
            Some("konsole kitty")
        );
        assert_eq!(reloaded_cfg.iconslight.as_deref(), Some("Papirus-Light"));
        assert_eq!(reloaded_cfg.iconsdark.as_deref(), Some("Papirus-Dark"));
        assert_eq!(
            reloaded_cfg.on_change_hook.as_deref(),
            Some("/usr/local/bin/hook.sh")
        );
        assert!(reloaded_cfg.pause_mode);
        assert!(reloaded_cfg.screenshot_only_mode);
        assert!(reloaded_cfg.manual_fetch);
    }

    #[test]
    fn test_seed_swatch_selection_monitor_and_fetch_colors_now() {
        let dir = tempdir().expect("create tempdir");
        let cfg_path = dir.path().join("config.conf");
        let json_path = dir.path().join("runtime.json");

        // Write a runtime JSON containing multiple extracted wallpaper seed colors
        let runtime_payload = serde_json::json!({
            "best": ["#4285f4", "#ea4335", "#34a853", "#fbbc05"],
            "seed": { "index": 0, "color": "#4285f4" }
        });
        std::fs::write(&json_path, serde_json::to_string(&runtime_payload).unwrap())
            .expect("write runtime.json");

        let mut gui = SettingsGui::new_with_paths(cfg_path.clone(), json_path.clone());
        let scheme = gui.current_scheme.as_ref().expect("scheme loaded");
        assert_eq!(scheme.best_list.len(), 4);
        assert_eq!(scheme.seed.color, "#4285f4");

        // Select swatch index 2 (#34a853)
        let _ = gui.update(GuiMessage::SeedIndexSelected(2));
        assert_eq!(gui.config.ncolor, 2);
        assert!(gui.config.color.is_none());
        let scheme_after = gui.current_scheme.as_ref().expect("scheme regenerated");
        assert_eq!(scheme_after.seed.index, 2);
        assert_eq!(scheme_after.seed.color, "#34a853");

        // Monitor number controls
        let _ = gui.update(GuiMessage::MonitorChanged(1));
        assert_eq!(gui.config.monitor, 1);
        let _ = gui.update(GuiMessage::MonitorInputChanged("3".to_string()));
        assert_eq!(gui.config.monitor, 3);

        // FetchColorsNow sets fetch_colors = true and writes config.conf + .reload
        let _ = gui.update(GuiMessage::FetchColorsNow);
        assert!(gui.config.fetch_colors);
        assert!(cfg_path.exists());
        assert!(gui.reload_trigger_path().exists());
    }

    #[test]
    fn test_gui_theme_mode_and_editable_multiplier_inputs() {
        let dir = tempdir().expect("create tempdir");
        let cfg_path = dir.path().join("config.conf");
        let json_path = dir.path().join("runtime.json");

        let mut gui = SettingsGui::new_with_paths(cfg_path, json_path);
        assert_eq!(gui.gui_theme_mode, GuiThemeMode::System);

        // System mode follows `system_light` (detected from kdeglobals)
        gui.system_light = true;
        assert!(gui.is_light_theme());
        gui.system_light = false;
        assert!(!gui.is_light_theme());

        // Explicit Light / Dark GUI theme mode overrides `system_light`
        let _ = gui.update(GuiMessage::GuiThemeModeChanged(GuiThemeMode::Light));
        assert!(gui.is_light_theme());
        gui.system_light = true;
        let _ = gui.update(GuiMessage::GuiThemeModeChanged(GuiThemeMode::Dark));
        assert!(!gui.is_light_theme());
        let _ = gui.update(GuiMessage::GuiThemeModeChanged(GuiThemeMode::System));
        assert!(gui.is_light_theme());

        // Editable multiplier inputs update config + clamp + sync on reset
        let _ = gui.update(GuiMessage::ChromaInputChanged("1.73".to_string()));
        assert!((gui.config.chroma_multiplier - 1.73).abs() < 1e-6);
        assert_eq!(gui.chroma_input, "1.73");
        let _ = gui.update(GuiMessage::ResetChromaMultiplier);
        assert!((gui.config.chroma_multiplier - 1.0).abs() < 1e-6);
        assert_eq!(gui.chroma_input, "1.00");

        let _ = gui.update(GuiMessage::ContrastInputChanged("-0.35".to_string()));
        assert!((gui.config.contrast_level - (-0.35)).abs() < 1e-6);
        let _ = gui.update(GuiMessage::ResetContrastLevel);
        assert!((gui.config.contrast_level - 0.0).abs() < 1e-6);
        assert_eq!(gui.contrast_input, "0.00");
    }
}
