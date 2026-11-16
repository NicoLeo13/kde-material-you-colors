use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use iced::widget::{
    button, checkbox, column, container, horizontal_rule, pick_list, row, scrollable, slider, text,
    text_input, Space,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Task, Theme};

use crate::color::hex_to_rgb;
use crate::config::{normalize_color, AppConfig};
use crate::dbus_client;
use crate::scheme::{MaterialYouScheme, SchemeRequest, SchemeVariant};

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

/// User interaction messages handled by `SettingsGui`.
#[derive(Debug, Clone, PartialEq)]
pub enum GuiMessage {
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
    ToneMultiplierChanged(f64),
    ContrastLevelChanged(f64),
    FrameContrastChanged(f64),
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
    pub show_advanced: bool,
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

        let mut gui = Self {
            config,
            config_path,
            runtime_json_path,
            current_scheme: loaded_scheme,
            show_advanced: false,
            status_message: "Ready".to_string(),
        };

        if gui.current_scheme.is_none() {
            gui.regenerate_preview_scheme();
        }
        gui
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

        let is_light = self.config.light.unwrap_or(false);
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
        match message {
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
                self.config.fetch_colors = true;
                match self.save_and_touch_reload() {
                    Ok(_) => {
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
                self.config.chroma_multiplier = (val * 100.0).round() / 100.0;
                self.config.chroma_multiplier = self.config.chroma_multiplier.clamp(0.0, 4.0);
                self.regenerate_preview_scheme();
            }
            GuiMessage::ToneMultiplierChanged(val) => {
                self.config.tone_multiplier = (val * 100.0).round() / 100.0;
                self.config.tone_multiplier = self.config.tone_multiplier.clamp(0.0, 1.5);
                self.regenerate_preview_scheme();
            }
            GuiMessage::ContrastLevelChanged(val) => {
                self.config.contrast_level = (val * 100.0).round() / 100.0;
                self.config.contrast_level = self.config.contrast_level.clamp(-1.0, 1.0);
                self.regenerate_preview_scheme();
            }
            GuiMessage::FrameContrastChanged(val) => {
                self.config.frame_contrast = (val * 100.0).round() / 100.0;
                self.config.frame_contrast = self.config.frame_contrast.clamp(0.0, 1.0);
                self.regenerate_preview_scheme();
            }
            GuiMessage::SpecVersionChanged(spec) => {
                if spec == "2021" || spec == "2025" {
                    self.config.spec_version = spec;
                }
            }
            GuiMessage::ToggleAdvanced(show) => {
                self.show_advanced = show;
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
            GuiMessage::SaveAndApply => match self.save_and_touch_reload() {
                Ok(_) => {
                    self.status_message =
                        format!("Saved configuration to {}", self.config_path.display());
                }
                Err(err) => {
                    self.status_message = format!("Failed to save configuration: {err}");
                }
            },
        }
        Task::none()
    }

    /// Return the active `iced::Theme` matching the selected dark/light state.
    pub fn theme(&self) -> Theme {
        if self.config.light == Some(true) {
            Theme::Light
        } else {
            Theme::Dark
        }
    }

    /// Lookup a role hex color from the currently active light/dark map of `self.current_scheme`.
    pub fn active_role_hex(&self, role: &str) -> Option<String> {
        let scheme = self.current_scheme.as_ref()?;
        let is_light = self.config.light.unwrap_or(scheme.light);
        let map = if is_light {
            &scheme.schemes.light
        } else {
            &scheme.schemes.dark
        };
        map.get(role).cloned()
    }

    /// Build the responsive Material You Settings & Live Palette Preview interface.
    pub fn view(&self) -> Element<'_, GuiMessage> {
        let surface_low_hex = self
            .active_role_hex("surfaceContainerLow")
            .unwrap_or_else(|| "#1d1b20".to_string());
        let outline_var_hex = self
            .active_role_hex("outlineVariant")
            .unwrap_or_else(|| "#49454f".to_string());

        let header = row![
            column![
                text("Material You Colors").size(24),
                text("Dynamic palette extraction and KDE Plasma 6 theme synchronization").size(13),
            ]
            .spacing(4)
            .width(Length::Fill),
            button(text("Fetch colors now").size(14))
                .padding([8, 14])
                .on_press(GuiMessage::FetchColorsNow),
            button(text("Save changes").size(14))
                .padding([8, 16])
                .on_press(GuiMessage::SaveAndApply),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let preview_section = self.view_palette_preview(&surface_low_hex, &outline_var_hex);
        let source_section = self.view_color_source_section(&surface_low_hex, &outline_var_hex);
        let mode_and_scheme_section =
            self.view_mode_and_scheme_section(&surface_low_hex, &outline_var_hex);
        let targets_section = self.view_targets_section(&surface_low_hex, &outline_var_hex);

        let status_bar = row![
            text(&self.status_message).size(13).width(Length::Fill),
            checkbox("Show advanced settings", self.show_advanced)
                .on_toggle(GuiMessage::ToggleAdvanced),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let mut content_col = column![
            header,
            horizontal_rule(1),
            preview_section,
            source_section,
            mode_and_scheme_section,
            targets_section,
        ]
        .spacing(16)
        .padding(20)
        .width(Length::Fill);

        if self.show_advanced {
            content_col = content_col.push(
                self.view_advanced_section(&surface_low_hex, &outline_var_hex),
            );
        }

        content_col = content_col.push(horizontal_rule(1)).push(status_bar);

        scrollable(container(content_col).width(Length::Fill)).into()
    }

    fn view_palette_preview(
        &self,
        card_bg_hex: &str,
        border_hex: &str,
    ) -> Element<'_, GuiMessage> {
        let mut cards_row_1 = row![].spacing(10).width(Length::Fill);
        let mut cards_row_2 = row![].spacing(10).width(Length::Fill);

        for (idx, (role_key, role_label, on_role_key)) in PREVIEW_ROLES.iter().enumerate() {
            let bg_hex = self
                .active_role_hex(role_key)
                .unwrap_or_else(|| "#2b2930".to_string());
            let fg_hex = self
                .active_role_hex(on_role_key)
                .unwrap_or_else(|| "#e6e0e9".to_string());

            let swatch_bg = hex_to_iced_color(&bg_hex).unwrap_or(Color::from_rgb8(43, 41, 48));
            let swatch_fg = hex_to_iced_color(&fg_hex).unwrap_or(Color::WHITE);
            let outer_bg = hex_to_iced_color(card_bg_hex).unwrap_or(Color::from_rgb8(29, 27, 32));
            let outer_border =
                hex_to_iced_color(border_hex).unwrap_or(Color::from_rgba(1.0, 1.0, 1.0, 0.12));

            let inner_swatch = container(
                column![
                    text(*role_label).size(13).color(swatch_fg),
                    text(bg_hex).size(11).color(swatch_fg),
                ]
                .spacing(4),
            )
            .padding(10)
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(swatch_bg)),
                text_color: Some(swatch_fg),
                border: Border {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.08),
                    width: 1.0,
                    radius: SWATCH_INNER_RADIUS.into(),
                },
                ..Default::default()
            });

            // Concentric border radius: outer radius (16px) = inner radius (8px) + card padding (8px)
            let outer_card = container(inner_swatch)
                .padding(SWATCH_CARD_PADDING)
                .width(Length::FillPortion(1))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(outer_bg)),
                    border: Border {
                        color: outer_border,
                        width: 1.0,
                        radius: SWATCH_OUTER_RADIUS.into(),
                    },
                    ..Default::default()
                });

            if idx < 5 {
                cards_row_1 = cards_row_1.push(outer_card);
            } else {
                cards_row_2 = cards_row_2.push(outer_card);
            }
        }

        column![
            text("Live Material You palette").size(16),
            cards_row_1,
            cards_row_2,
        ]
        .spacing(10)
        .into()
    }

    fn view_color_source_section(
        &self,
        card_bg_hex: &str,
        border_hex: &str,
    ) -> Element<'_, GuiMessage> {
        let using_wallpaper = self.config.color.is_none();
        let custom_color_val = self.config.color.as_deref().unwrap_or("");
        let monitor_str = self.config.monitor.to_string();

        let source_toggle_row = row![
            checkbox("Extract colors from wallpaper", using_wallpaper)
                .on_toggle(GuiMessage::UseWallpaperSource),
            Space::with_width(Length::Fixed(16.0)),
            text("Monitor index:").size(14),
            button(text("-").size(14))
                .padding([4, 10])
                .on_press(GuiMessage::MonitorChanged(
                    self.config.monitor.saturating_sub(1)
                )),
            text_input("0", &monitor_str)
                .on_input(GuiMessage::MonitorInputChanged)
                .width(Length::Fixed(56.0)),
            button(text("+").size(14))
                .padding([4, 10])
                .on_press(GuiMessage::MonitorChanged(self.config.monitor + 1)),
            Space::with_width(Length::Fixed(16.0)),
            text("Custom seed hex:").size(14),
            text_input("#4285f4", custom_color_val)
                .on_input(GuiMessage::CustomColorChanged)
                .width(Length::Fixed(120.0)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let mut seed_pills_row = row![text("Wallpaper seed swatches:").size(14)]
            .spacing(8)
            .align_y(Alignment::Center);

        let best_colors: Vec<String> = self
            .current_scheme
            .as_ref()
            .map(|s| s.best_list.clone())
            .unwrap_or_else(|| vec!["#4285f4".to_string()]);

        for (idx, hex_str) in best_colors.into_iter().take(7).enumerate() {
            let is_selected = (self.config.ncolor as usize) == idx && self.config.color.is_none();
            let label = if is_selected {
                format!("#{} {} ✓", idx + 1, hex_str)
            } else {
                format!("#{} {}", idx + 1, hex_str)
            };
            seed_pills_row = seed_pills_row.push(
                button(text(label).size(13))
                    .padding([6, 12])
                    .on_press(GuiMessage::SeedIndexSelected(idx as u32)),
            );
        }

        Self::styled_section_card(
            column![
                text("Color source").size(16),
                source_toggle_row,
                seed_pills_row,
            ]
            .spacing(12)
            .into(),
            card_bg_hex,
            border_hex,
        )
    }

    fn view_mode_and_scheme_section(
        &self,
        card_bg_hex: &str,
        border_hex: &str,
    ) -> Element<'_, GuiMessage> {
        let plasma_mode = ThemeMode::from_plasma_config(&self.config);
        let pywal_mode = ThemeMode::from_pywal_config(&self.config);
        let current_variant = SchemeVariant::from_u8(self.config.scheme_variant);

        let modes_row = row![
            text("Plasma mode:").size(14),
            pick_list(
                ThemeMode::ALL.as_slice(),
                Some(plasma_mode),
                GuiMessage::PlasmaModeChanged,
            ),
            Space::with_width(Length::Fixed(16.0)),
            text("Konsole / Pywal / KSyntax mode:").size(14),
            pick_list(
                ThemeMode::ALL.as_slice(),
                Some(pywal_mode),
                GuiMessage::PywalModeChanged,
            ),
            Space::with_width(Length::Fixed(16.0)),
            text("Palette style:").size(14),
            pick_list(
                ALL_SCHEME_VARIANTS.as_slice(),
                Some(current_variant),
                GuiMessage::SchemeVariantChanged,
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let spec_row = row![
            text("Material spec version:").size(14),
            button(
                text(if self.config.spec_version == "2025" {
                    "2025 ( Material 3 Expressive ) ✓"
                } else {
                    "2025"
                })
                .size(13)
            )
            .padding([6, 12])
            .on_press(GuiMessage::SpecVersionChanged("2025".to_string())),
            button(
                text(if self.config.spec_version == "2021" {
                    "2021 ( Classic M3 ) ✓"
                } else {
                    "2021"
                })
                .size(13)
            )
            .padding([6, 12])
            .on_press(GuiMessage::SpecVersionChanged("2021".to_string())),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let sliders_col = column![
            row![
                text(format!(
                    "Chroma multiplier: {:.2}",
                    self.config.chroma_multiplier
                ))
                .width(Length::Fixed(210.0))
                .size(14),
                slider(
                    0.0..=4.0,
                    self.config.chroma_multiplier,
                    GuiMessage::ChromaMultiplierChanged,
                )
                .step(0.05),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            row![
                text(format!(
                    "Tone / lightness multiplier: {:.2}",
                    self.config.tone_multiplier
                ))
                .width(Length::Fixed(210.0))
                .size(14),
                slider(
                    0.0..=1.5,
                    self.config.tone_multiplier,
                    GuiMessage::ToneMultiplierChanged,
                )
                .step(0.05),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            row![
                text(format!("Contrast level: {:.2}", self.config.contrast_level))
                    .width(Length::Fixed(210.0))
                    .size(14),
                slider(
                    -1.0..=1.0,
                    self.config.contrast_level,
                    GuiMessage::ContrastLevelChanged,
                )
                .step(0.05),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            row![
                text(format!("Frame contrast: {:.2}", self.config.frame_contrast))
                    .width(Length::Fixed(210.0))
                    .size(14),
                slider(
                    0.0..=1.0,
                    self.config.frame_contrast,
                    GuiMessage::FrameContrastChanged,
                )
                .step(0.05),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        ]
        .spacing(10);

        Self::styled_section_card(
            column![
                text("Appearance & scheme multipliers").size(16),
                modes_row,
                spec_row,
                sliders_col,
            ]
            .spacing(12)
            .into(),
            card_bg_hex,
            border_hex,
        )
    }

    fn view_targets_section(
        &self,
        card_bg_hex: &str,
        border_hex: &str,
    ) -> Element<'_, GuiMessage> {
        let apply_konsole = !self.config.disable_konsole;

        let konsole_col = column![
            row![
                checkbox("Apply theme to Konsole", apply_konsole)
                    .on_toggle(GuiMessage::ApplyKonsoleChanged),
                Space::with_width(Length::Fixed(16.0)),
                checkbox("Enable Konsole background blur", self.config.konsole_blur)
                    .on_toggle(GuiMessage::KonsoleBlurChanged),
                Space::with_width(Length::Fixed(16.0)),
                checkbox("Export Pywal palette files", self.config.pywal)
                    .on_toggle(GuiMessage::ApplyPywalChanged),
                Space::with_width(Length::Fixed(16.0)),
                checkbox("Export Kitty theme files", self.config.kitty)
                    .on_toggle(GuiMessage::ApplyKittyChanged),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                text(format!(
                    "Konsole opacity (light): {}%",
                    self.config.konsole_opacity
                ))
                .width(Length::Fixed(210.0))
                .size(14),
                slider(
                    0..=100,
                    self.config.konsole_opacity,
                    GuiMessage::KonsoleOpacityChanged,
                ),
                Space::with_width(Length::Fixed(16.0)),
                text(format!(
                    "Konsole opacity (dark): {}%",
                    self.config.konsole_opacity_dark
                ))
                .width(Length::Fixed(210.0))
                .size(14),
                slider(
                    0..=100,
                    self.config.konsole_opacity_dark,
                    GuiMessage::KonsoleOpacityDarkChanged,
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(10);

        let titlebar_col = column![
            checkbox(
                "Override window titlebar opacity",
                self.config.titlebar_opacity_override
            )
            .on_toggle(GuiMessage::TitlebarOpacityOverrideChanged),
            row![
                text(format!(
                    "Titlebar opacity (light): {}%",
                    self.config.titlebar_opacity
                ))
                .width(Length::Fixed(210.0))
                .size(14),
                slider(
                    0..=100,
                    self.config.titlebar_opacity,
                    GuiMessage::TitlebarOpacityChanged,
                ),
                Space::with_width(Length::Fixed(16.0)),
                text(format!(
                    "Titlebar opacity (dark): {}%",
                    self.config.titlebar_opacity_dark
                ))
                .width(Length::Fixed(210.0))
                .size(14),
                slider(
                    0..=100,
                    self.config.titlebar_opacity_dark,
                    GuiMessage::TitlebarOpacityDarkChanged,
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(8);

        let toolbar_col = column![
            checkbox(
                "Override application toolbar opacity (Breeze)",
                self.config.toolbar_opacity_override
            )
            .on_toggle(GuiMessage::ToolbarOpacityOverrideChanged),
            row![
                text(format!(
                    "Toolbar opacity (light): {}%",
                    self.config.toolbar_opacity
                ))
                .width(Length::Fixed(210.0))
                .size(14),
                slider(
                    0..=100,
                    self.config.toolbar_opacity,
                    GuiMessage::ToolbarOpacityChanged,
                ),
                Space::with_width(Length::Fixed(16.0)),
                text(format!(
                    "Toolbar opacity (dark): {}%",
                    self.config.toolbar_opacity_dark
                ))
                .width(Length::Fixed(210.0))
                .size(14),
                slider(
                    0..=100,
                    self.config.toolbar_opacity_dark,
                    GuiMessage::ToolbarOpacityDarkChanged,
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(8);

        let windeco_row = row![
            checkbox(
                "Tint Sierra Breeze window buttons",
                self.config.sierra_breeze_buttons_color
            )
            .on_toggle(GuiMessage::SierraBreezeButtonsChanged),
            Space::with_width(Length::Fixed(16.0)),
            checkbox(
                "Tint Klassy window decoration outline",
                self.config.klassy_windeco_outline
            )
            .on_toggle(GuiMessage::KlassyWindecoOutlineChanged),
            Space::with_width(Length::Fixed(16.0)),
            checkbox(
                "Tint KDE Rounded Corners outline",
                self.config.kde_rounded_corners_effect_outline
            )
            .on_toggle(GuiMessage::KdeRoundedCornersOutlineChanged),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        Self::styled_section_card(
            column![
                text("Theme targets & window decorations").size(16),
                konsole_col,
                titlebar_col,
                toolbar_col,
                windeco_row,
            ]
            .spacing(12)
            .into(),
            card_bg_hex,
            border_hex,
        )
    }

    fn view_advanced_section(
        &self,
        card_bg_hex: &str,
        border_hex: &str,
    ) -> Element<'_, GuiMessage> {
        let darker_list = self.config.darker_window_list.as_deref().unwrap_or("");
        let icons_light = self.config.iconslight.as_deref().unwrap_or("");
        let icons_dark = self.config.iconsdark.as_deref().unwrap_or("");
        let hook = self.config.on_change_hook.as_deref().unwrap_or("");

        let inputs_col = column![
            row![
                text("Darker titlebar window classes:")
                    .width(Length::Fixed(230.0))
                    .size(14),
                text_input("konsole Alacritty kitty", darker_list)
                    .on_input(GuiMessage::DarkerWindowListChanged)
                    .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                text("Light mode icon theme:")
                    .width(Length::Fixed(230.0))
                    .size(14),
                text_input("breeze", icons_light)
                    .on_input(GuiMessage::IconsLightChanged)
                    .width(Length::Fill),
                Space::with_width(Length::Fixed(16.0)),
                text("Dark mode icon theme:")
                    .width(Length::Fixed(190.0))
                    .size(14),
                text_input("breeze-dark", icons_dark)
                    .on_input(GuiMessage::IconsDarkChanged)
                    .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                text("On-change hook command / script:")
                    .width(Length::Fixed(230.0))
                    .size(14),
                text_input("/path/to/hook.sh", hook)
                    .on_input(GuiMessage::OnChangeHookChanged)
                    .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(10);

        let flags_row = row![
            checkbox("Pause automatic theme updates", self.config.pause_mode)
                .on_toggle(GuiMessage::PauseModeChanged),
            Space::with_width(Length::Fixed(16.0)),
            checkbox(
                "Force KWin screenshot capture mode",
                self.config.screenshot_only_mode
            )
            .on_toggle(GuiMessage::ScreenshotOnlyModeChanged),
            Space::with_width(Length::Fixed(16.0)),
            checkbox("Manual wallpaper fetch only", self.config.manual_fetch)
                .on_toggle(GuiMessage::ManualFetchChanged),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        Self::styled_section_card(
            column![
                text("Advanced automation & hooks").size(16),
                inputs_col,
                flags_row,
            ]
            .spacing(12)
            .into(),
            card_bg_hex,
            border_hex,
        )
    }

    fn styled_section_card<'a>(
        inner: Element<'a, GuiMessage>,
        card_bg_hex: &str,
        border_hex: &str,
    ) -> Element<'a, GuiMessage> {
        let bg = hex_to_iced_color(card_bg_hex).unwrap_or(Color::from_rgb8(29, 27, 32));
        let border_color =
            hex_to_iced_color(border_hex).unwrap_or(Color::from_rgba(1.0, 1.0, 1.0, 0.12));

        container(inner)
            .padding(SECTION_CARD_PADDING)
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: SECTION_OUTER_RADIUS.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

/// Launch the standalone `iced` Material You configuration and live palette preview window.
pub fn run_settings_window(config_path: PathBuf, runtime_json_path: PathBuf) -> Result<()> {
    iced::application(
        "KDE Material You Colors — Settings",
        SettingsGui::update,
        SettingsGui::view,
    )
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
        assert_eq!(gui.theme(), Theme::Light);

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
}
