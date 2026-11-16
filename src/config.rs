use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::cli::CliArgs;

/// Full configuration state for `kde-material-you-colors`, compatible with
/// `~/.config/kde-material-you-colors/config.conf` (`[CUSTOM]` section),
/// `QtCore.Settings` in the Plasma widget, and Python `configparser`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub monitor: u32,
    pub file: Option<String>,
    pub color: Option<String>,
    pub custom_colors_list: Option<Vec<String>>,
    /// `None` when `plasma_follows_scheme` is `true` or not explicitly set.
    pub light: Option<bool>,
    pub ncolor: u32,
    pub iconslight: Option<String>,
    pub iconsdark: Option<String>,
    pub pywal: bool,
    pub kitty: bool,
    /// `None` when `pywal_follows_scheme` is `true` or not explicitly set.
    pub pywal_light: Option<bool>,
    pub light_blend_multiplier: f64,
    pub dark_blend_multiplier: f64,
    pub on_change_hook: Option<String>,
    pub sierra_breeze_buttons_color: bool,
    pub disable_konsole: bool,
    pub titlebar_opacity_override: bool,
    pub titlebar_opacity: u32,
    pub titlebar_opacity_dark: u32,
    pub toolbar_opacity_override: bool,
    pub toolbar_opacity: u32,
    pub toolbar_opacity_dark: u32,
    pub konsole_opacity: u32,
    pub konsole_opacity_dark: u32,
    pub konsole_blur: bool,
    pub klassy_windeco_outline: bool,
    pub darker_window_list: Option<String>,
    pub use_startup_delay: bool,
    pub startup_delay: u32,
    pub plasma_follows_scheme: bool,
    pub pywal_follows_scheme: bool,
    pub main_loop_delay: f64,
    pub screenshot_delay: f64,
    pub once_after_change: bool,
    pub pause_mode: bool,
    pub screenshot_only_mode: bool,
    pub scheme_variant: u8,
    pub chroma_multiplier: f64,
    pub tone_multiplier: f64,
    pub frame_contrast: f64,
    pub contrast_level: f64,
    pub qdbus_executable: Option<String>,
    pub manual_fetch: bool,
    pub fetch_colors: bool,
    pub spec_version: String,
    pub kde_rounded_corners_effect_outline: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            monitor: 0,
            file: None,
            color: None,
            custom_colors_list: None,
            light: None,
            ncolor: 0,
            iconslight: None,
            iconsdark: None,
            pywal: false,
            kitty: false,
            pywal_light: None,
            light_blend_multiplier: 1.0,
            dark_blend_multiplier: 1.0,
            on_change_hook: None,
            sierra_breeze_buttons_color: false,
            disable_konsole: false,
            titlebar_opacity_override: false,
            titlebar_opacity: 100,
            titlebar_opacity_dark: 100,
            toolbar_opacity_override: false,
            toolbar_opacity: 100,
            toolbar_opacity_dark: 100,
            konsole_opacity: 100,
            konsole_opacity_dark: 100,
            konsole_blur: false,
            klassy_windeco_outline: false,
            darker_window_list: None,
            use_startup_delay: false,
            startup_delay: 0,
            plasma_follows_scheme: true,
            pywal_follows_scheme: true,
            main_loop_delay: 1.0,
            screenshot_delay: 900.0,
            once_after_change: false,
            pause_mode: false,
            screenshot_only_mode: false,
            scheme_variant: 5,
            chroma_multiplier: 1.0,
            tone_multiplier: 1.0,
            frame_contrast: 0.2,
            contrast_level: 0.0,
            qdbus_executable: None,
            manual_fetch: false,
            fetch_colors: false,
            spec_version: "2025".to_string(),
            kde_rounded_corners_effect_outline: false,
        }
    }
}

/// Parse a boolean string case-insensitively (`true`/`false`/`1`/`0`/`yes`/`no`/`on`/`off`).
pub fn parse_bool(val: &str) -> Option<bool> {
    match val.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Normalize a hex (`#rgb` or `#rrggbb`) or comma-separated RGB (`255,0,128`) color string
/// into a 6-digit lowercase hex color `#rrggbb`. Returns `None` if invalid.
pub fn normalize_color(input: &str) -> Option<String> {
    let s = input.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let mut out = String::with_capacity(7);
            out.push('#');
            for c in hex.chars() {
                let lower = c.to_ascii_lowercase();
                out.push(lower);
                out.push(lower);
            }
            return Some(out);
        } else if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(format!("#{}", hex.to_ascii_lowercase()));
        }
        return None;
    }

    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    if parts.len() == 3 {
        let r = parts[0].parse::<u8>().ok()?;
        let g = parts[1].parse::<u8>().ok()?;
        let b = parts[2].parse::<u8>().ok()?;
        return Some(format!("#{:02x}{:02x}{:02x}", r, g, b));
    }

    None
}

/// Parse and validate a space-separated list of at least 7 colors (hex or RGB).
pub fn parse_custom_colors_list(raw: &str) -> Option<Vec<String>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.len() < 7 {
        return None;
    }
    let mut normalized = Vec::with_capacity(tokens.len());
    for token in tokens {
        let hex = normalize_color(token)?;
        normalized.push(hex);
    }
    Some(normalized)
}

impl AppConfig {
    /// Returns the default user config path `~/.config/kde-material-you-colors/config.conf`.
    pub fn default_path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|dir| dir.join("kde-material-you-colors").join("config.conf"))
    }

    /// Clamp numeric fields and enforce inter-field invariants matching `config.py`.
    pub fn sanitize(&mut self) {
        self.light_blend_multiplier = self.light_blend_multiplier.clamp(0.0, 4.0);
        self.dark_blend_multiplier = self.dark_blend_multiplier.clamp(0.0, 4.0);
        self.titlebar_opacity = self.titlebar_opacity.clamp(0, 100);
        self.titlebar_opacity_dark = self.titlebar_opacity_dark.clamp(0, 100);
        self.toolbar_opacity = self.toolbar_opacity.clamp(0, 100);
        self.toolbar_opacity_dark = self.toolbar_opacity_dark.clamp(0, 100);
        self.konsole_opacity = self.konsole_opacity.clamp(0, 100);
        self.konsole_opacity_dark = self.konsole_opacity_dark.clamp(0, 100);
        self.scheme_variant = self.scheme_variant.min(8);
        self.chroma_multiplier = self.chroma_multiplier.clamp(0.5, 10.0);
        self.tone_multiplier = self.tone_multiplier.clamp(0.0, 1.5);
        self.frame_contrast = self.frame_contrast.clamp(0.0, 1.0);
        self.contrast_level = self.contrast_level.clamp(-1.0, 1.0);
        if self.main_loop_delay < 0.0 {
            self.main_loop_delay = 1.0;
        }
        if self.screenshot_delay < 0.0 {
            self.screenshot_delay = 900.0;
        }
        if self.plasma_follows_scheme {
            self.light = None;
        }
        if self.pywal_follows_scheme {
            self.pywal_light = None;
        }
    }

    /// Parse `[CUSTOM]` INI content into an `AppConfig`.
    /// Lines outside `[CUSTOM]` are ignored if a `[CUSTOM]` section header is present;
    /// if no `[CUSTOM]` section exists, returns `AppConfig::default()` (matching `config.py`).
    pub fn from_ini_str(content: &str) -> Self {
        let mut config = Self::default();
        let mut custom_map: HashMap<String, String> = HashMap::new();
        let mut current_section: Option<String> = None;
        let mut saw_custom_section = false;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                let sec = line[1..line.len() - 1].trim().to_string();
                if sec.eq_ignore_ascii_case("CUSTOM") {
                    saw_custom_section = true;
                }
                current_section = Some(sec);
                continue;
            }
            if let Some(ref sec) = current_section {
                if sec.eq_ignore_ascii_case("CUSTOM") {
                    if let Some((k, v)) = line.split_once('=') {
                        let key = k.trim().to_ascii_lowercase();
                        let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                        custom_map.insert(key, val);
                    }
                }
            }
        }

        if !saw_custom_section {
            return config;
        }

        let get_non_empty = |key: &str| -> Option<&str> {
            custom_map
                .get(key)
                .map(|s| s.as_str())
                .filter(|s| !s.is_empty())
        };

        if let Some(v) = get_non_empty("monitor") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.monitor = parsed.max(0) as u32;
            }
        }
        if let Some(v) = get_non_empty("file") {
            config.file = Some(v.to_string());
        }
        if let Some(v) = get_non_empty("color") {
            config.color = normalize_color(v).or_else(|| Some(v.to_string()));
        }
        if let Some(v) = get_non_empty("custom_colors_list") {
            config.custom_colors_list = parse_custom_colors_list(v);
        }
        if let Some(v) = get_non_empty("ncolor") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.ncolor = parsed.max(0) as u32;
            }
        }
        if let Some(v) = get_non_empty("iconslight") {
            config.iconslight = Some(v.to_string());
        }
        if let Some(v) = get_non_empty("iconsdark") {
            config.iconsdark = Some(v.to_string());
        }
        if let Some(v) = get_non_empty("pywal").and_then(parse_bool) {
            config.pywal = v;
        }
        if let Some(v) = get_non_empty("kitty").and_then(parse_bool) {
            config.kitty = v;
        }

        let raw_light = get_non_empty("light").and_then(parse_bool);
        let raw_plasma_follows = get_non_empty("plasma_follows_scheme").and_then(parse_bool);
        match (raw_plasma_follows, raw_light) {
            (Some(true), _) => {
                config.plasma_follows_scheme = true;
                config.light = None;
            }
            (Some(false), Some(l)) => {
                config.plasma_follows_scheme = false;
                config.light = Some(l);
            }
            (Some(false), None) => {
                config.plasma_follows_scheme = false;
                config.light = Some(false);
            }
            (None, Some(l)) => {
                config.plasma_follows_scheme = false;
                config.light = Some(l);
            }
            (None, None) => {
                config.plasma_follows_scheme = true;
                config.light = None;
            }
        }

        let raw_pywal_light = get_non_empty("pywal_light").and_then(parse_bool);
        let raw_pywal_follows = get_non_empty("pywal_follows_scheme").and_then(parse_bool);
        match (raw_pywal_follows, raw_pywal_light) {
            (Some(true), _) => {
                config.pywal_follows_scheme = true;
                config.pywal_light = None;
            }
            (Some(false), Some(l)) => {
                config.pywal_follows_scheme = false;
                config.pywal_light = Some(l);
            }
            (Some(false), None) => {
                config.pywal_follows_scheme = false;
                config.pywal_light = Some(false);
            }
            (None, Some(l)) => {
                config.pywal_follows_scheme = false;
                config.pywal_light = Some(l);
            }
            (None, None) => {
                config.pywal_follows_scheme = true;
                config.pywal_light = None;
            }
        }

        if let Some(v) = get_non_empty("light_blend_multiplier") {
            if let Ok(parsed) = v.parse::<f64>() {
                config.light_blend_multiplier = parsed.clamp(0.0, 4.0);
            }
        }
        if let Some(v) = get_non_empty("dark_blend_multiplier") {
            if let Ok(parsed) = v.parse::<f64>() {
                config.dark_blend_multiplier = parsed.clamp(0.0, 4.0);
            }
        }
        if let Some(v) = get_non_empty("on_change_hook") {
            config.on_change_hook = Some(v.to_string());
        }
        if let Some(v) = get_non_empty("sierra_breeze_buttons_color").and_then(parse_bool) {
            config.sierra_breeze_buttons_color = v;
        }
        if let Some(v) = get_non_empty("disable_konsole").and_then(parse_bool) {
            config.disable_konsole = v;
        }
        if let Some(v) = get_non_empty("titlebar_opacity_override").and_then(parse_bool) {
            config.titlebar_opacity_override = v;
        }
        if let Some(v) = get_non_empty("titlebar_opacity") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.titlebar_opacity = parsed.clamp(0, 100) as u32;
            }
        }
        if let Some(v) = get_non_empty("titlebar_opacity_dark") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.titlebar_opacity_dark = parsed.clamp(0, 100) as u32;
            }
        }
        if let Some(v) = get_non_empty("toolbar_opacity_override").and_then(parse_bool) {
            config.toolbar_opacity_override = v;
        }
        if let Some(v) = get_non_empty("toolbar_opacity") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.toolbar_opacity = parsed.clamp(0, 100) as u32;
            }
        }
        if let Some(v) = get_non_empty("toolbar_opacity_dark") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.toolbar_opacity_dark = parsed.clamp(0, 100) as u32;
            }
        }
        if let Some(v) = get_non_empty("konsole_opacity") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.konsole_opacity = parsed.clamp(0, 100) as u32;
            }
        }
        if let Some(v) = get_non_empty("konsole_opacity_dark") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.konsole_opacity_dark = parsed.clamp(0, 100) as u32;
            }
        }
        if let Some(v) = get_non_empty("konsole_blur").and_then(parse_bool) {
            config.konsole_blur = v;
        }
        if let Some(v) = get_non_empty("klassy_windeco_outline").and_then(parse_bool) {
            config.klassy_windeco_outline = v;
        }
        if let Some(v) = get_non_empty("darker_window_list") {
            config.darker_window_list = Some(v.to_string());
        }
        if let Some(v) = get_non_empty("use_startup_delay").and_then(parse_bool) {
            config.use_startup_delay = v;
        }
        if let Some(v) = get_non_empty("startup_delay") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.startup_delay = parsed.max(0) as u32;
            }
        }
        if let Some(v) = get_non_empty("main_loop_delay") {
            if let Ok(parsed) = v.parse::<f64>() {
                config.main_loop_delay = parsed.max(0.0);
            }
        }
        if let Some(v) = get_non_empty("screenshot_delay") {
            if let Ok(parsed) = v.parse::<f64>() {
                config.screenshot_delay = parsed.max(0.0);
            }
        }
        if let Some(v) = get_non_empty("once_after_change").and_then(parse_bool) {
            config.once_after_change = v;
        }
        if let Some(v) = get_non_empty("pause_mode").and_then(parse_bool) {
            config.pause_mode = v;
        }
        if let Some(v) = get_non_empty("screenshot_only_mode").and_then(parse_bool) {
            config.screenshot_only_mode = v;
        }
        if let Some(v) = get_non_empty("scheme_variant") {
            if let Ok(parsed) = v.parse::<i64>() {
                config.scheme_variant = parsed.clamp(0, 8) as u8;
            }
        }
        if let Some(v) = get_non_empty("chroma_multiplier") {
            if let Ok(parsed) = v.parse::<f64>() {
                config.chroma_multiplier = parsed.clamp(0.5, 10.0);
            }
        }
        if let Some(v) = get_non_empty("tone_multiplier") {
            if let Ok(parsed) = v.parse::<f64>() {
                config.tone_multiplier = parsed.clamp(0.0, 1.5);
            }
        }
        if let Some(v) = get_non_empty("frame_contrast") {
            if let Ok(parsed) = v.parse::<f64>() {
                config.frame_contrast = parsed.clamp(0.0, 1.0);
            }
        }
        if let Some(v) = get_non_empty("contrast_level") {
            if let Ok(parsed) = v.parse::<f64>() {
                config.contrast_level = parsed.clamp(-1.0, 1.0);
            }
        }
        if let Some(v) = get_non_empty("qdbus_executable") {
            config.qdbus_executable = Some(v.to_string());
        }
        if let Some(v) = get_non_empty("manual_fetch").and_then(parse_bool) {
            config.manual_fetch = v;
        }
        if let Some(v) = get_non_empty("fetch_colors").and_then(parse_bool) {
            config.fetch_colors = v;
        }
        if let Some(v) = get_non_empty("spec_version") {
            config.spec_version = v.to_string();
        }
        if let Some(v) = get_non_empty("kde_rounded_corners_effect_outline").and_then(parse_bool) {
            config.kde_rounded_corners_effect_outline = v;
        }

        config.sanitize();
        config
    }

    /// Load configuration from a file path. If the file does not exist, returns `AppConfig::default()`.
    pub fn load_from_path(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file at {}", path.display()))?;
        Ok(Self::from_ini_str(&content))
    }

    /// Apply command-line overrides on top of `self`, returning a new `AppConfig`.
    /// CLI flags take highest precedence over INI values and defaults.
    pub fn with_cli_overrides(&self, cli: &CliArgs) -> Self {
        let mut cfg = self.clone();

        if let Some(monitor) = cli.monitor {
            cfg.monitor = monitor;
        }
        if let Some(ref file) = cli.file {
            if !file.is_empty() {
                cfg.file = Some(file.clone());
            }
        }
        if let Some(ref color) = cli.color {
            if !color.is_empty() {
                cfg.color = normalize_color(color).or_else(|| Some(color.clone()));
            }
        }
        if let Some(ref ccl) = cli.custom_colors_list {
            cfg.custom_colors_list = parse_custom_colors_list(ccl);
        }
        if cli.dark {
            cfg.plasma_follows_scheme = false;
            cfg.light = Some(false);
        } else if cli.light {
            cfg.plasma_follows_scheme = false;
            cfg.light = Some(true);
        }
        if let Some(ncolor) = cli.ncolor {
            cfg.ncolor = ncolor;
        }
        if let Some(ref iconslight) = cli.iconslight {
            if !iconslight.is_empty() {
                cfg.iconslight = Some(iconslight.clone());
            }
        }
        if let Some(ref iconsdark) = cli.iconsdark {
            if !iconsdark.is_empty() {
                cfg.iconsdark = Some(iconsdark.clone());
            }
        }
        if cli.pywal {
            cfg.pywal = true;
        }
        if cli.kitty {
            cfg.kitty = true;
        }
        if cli.pywal_dark {
            cfg.pywal_follows_scheme = false;
            cfg.pywal_light = Some(false);
        } else if cli.pywal_light {
            cfg.pywal_follows_scheme = false;
            cfg.pywal_light = Some(true);
        }
        if let Some(lbm) = cli.light_blend_multiplier {
            cfg.light_blend_multiplier = lbm.clamp(0.0, 4.0);
        }
        if let Some(dbm) = cli.dark_blend_multiplier {
            cfg.dark_blend_multiplier = dbm.clamp(0.0, 4.0);
        }
        if let Some(ref hook) = cli.on_change_hook {
            if !hook.is_empty() {
                cfg.on_change_hook = Some(hook.clone());
            }
        }
        if cli.sierra_breeze_buttons_color {
            cfg.sierra_breeze_buttons_color = true;
        }
        if cli.disable_konsole {
            cfg.disable_konsole = true;
        }
        if cli.titlebar_opacity_override {
            cfg.titlebar_opacity_override = true;
        }
        if let Some(tio) = cli.titlebar_opacity {
            cfg.titlebar_opacity = tio.clamp(0, 100);
        }
        if let Some(tiod) = cli.titlebar_opacity_dark {
            cfg.titlebar_opacity_dark = tiod.clamp(0, 100);
        }
        if cli.toolbar_opacity_override {
            cfg.toolbar_opacity_override = true;
        }
        if let Some(too) = cli.toolbar_opacity {
            cfg.toolbar_opacity = too.clamp(0, 100);
        }
        if let Some(tood) = cli.toolbar_opacity_dark {
            cfg.toolbar_opacity_dark = tood.clamp(0, 100);
        }
        if let Some(ko) = cli.konsole_opacity {
            cfg.konsole_opacity = ko.clamp(0, 100);
        }
        if let Some(kod) = cli.konsole_opacity_dark {
            cfg.konsole_opacity_dark = kod.clamp(0, 100);
        }
        if cli.konsole_blur {
            cfg.konsole_blur = true;
        }
        if cli.klassy_windeco_outline {
            cfg.klassy_windeco_outline = true;
        }
        if let Some(ref dwl) = cli.darker_window_list {
            if !dwl.is_empty() {
                cfg.darker_window_list = Some(dwl.clone());
            }
        }
        if cli.use_startup_delay {
            cfg.use_startup_delay = true;
        }
        if let Some(sd) = cli.startup_delay {
            cfg.startup_delay = sd;
        }
        if let Some(mld) = cli.main_loop_delay {
            cfg.main_loop_delay = mld.max(0.0);
        }
        if let Some(scd) = cli.screenshot_delay {
            cfg.screenshot_delay = scd.max(0.0);
        }
        if cli.once_after_change {
            cfg.once_after_change = true;
        }
        if cli.pause_mode {
            cfg.pause_mode = true;
        }
        if cli.screenshot_only_mode {
            cfg.screenshot_only_mode = true;
        }
        if let Some(sv) = cli.scheme_variant {
            cfg.scheme_variant = sv.min(8);
        }
        if let Some(cm) = cli.chroma_multiplier {
            cfg.chroma_multiplier = cm.clamp(0.5, 10.0);
        }
        if let Some(tm) = cli.tone_multiplier {
            cfg.tone_multiplier = tm.clamp(0.0, 1.5);
        }
        if let Some(fc) = cli.frame_contrast {
            cfg.frame_contrast = fc.clamp(0.0, 1.0);
        }
        if let Some(cl) = cli.contrast_level {
            cfg.contrast_level = cl.clamp(-1.0, 1.0);
        }
        if let Some(ref qdbus) = cli.qdbus_executable {
            if !qdbus.is_empty() {
                cfg.qdbus_executable = Some(qdbus.clone());
            }
        }
        if cli.manual_fetch {
            cfg.manual_fetch = true;
        }
        if cli.fetch_colors {
            cfg.fetch_colors = true;
        }
        if let Some(ref spec) = cli.spec_version {
            if !spec.is_empty() {
                cfg.spec_version = spec.clone();
            }
        }
        if cli.kde_rounded_corners_effect_outline {
            cfg.kde_rounded_corners_effect_outline = true;
        }

        cfg.sanitize();
        cfg
    }

    /// Serialize `AppConfig` into a `[CUSTOM]` INI string compatible with
    /// `QtCore.Settings` and Python `configparser`.
    pub fn to_ini_string(&self) -> String {
        let mut lines = Vec::with_capacity(44);
        lines.push("[CUSTOM]".to_string());
        lines.push(format!("monitor={}", self.monitor));
        lines.push(format!("file={}", self.file.as_deref().unwrap_or("")));
        lines.push(format!("color={}", self.color.as_deref().unwrap_or("")));
        let ccl_str = self
            .custom_colors_list
            .as_ref()
            .map(|v| v.join(" "))
            .unwrap_or_default();
        lines.push(format!("custom_colors_list={}", ccl_str));
        lines.push(format!(
            "light={}",
            self.light.unwrap_or(false)
        ));
        lines.push(format!("ncolor={}", self.ncolor));
        lines.push(format!(
            "iconslight={}",
            self.iconslight.as_deref().unwrap_or("")
        ));
        lines.push(format!(
            "iconsdark={}",
            self.iconsdark.as_deref().unwrap_or("")
        ));
        lines.push(format!("pywal={}", self.pywal));
        lines.push(format!("kitty={}", self.kitty));
        lines.push(format!(
            "pywal_light={}",
            self.pywal_light.unwrap_or(false)
        ));
        lines.push(format!(
            "light_blend_multiplier={}",
            self.light_blend_multiplier
        ));
        lines.push(format!(
            "dark_blend_multiplier={}",
            self.dark_blend_multiplier
        ));
        lines.push(format!(
            "on_change_hook={}",
            self.on_change_hook.as_deref().unwrap_or("")
        ));
        lines.push(format!(
            "sierra_breeze_buttons_color={}",
            self.sierra_breeze_buttons_color
        ));
        lines.push(format!("disable_konsole={}", self.disable_konsole));
        lines.push(format!(
            "titlebar_opacity_override={}",
            self.titlebar_opacity_override
        ));
        lines.push(format!("titlebar_opacity={}", self.titlebar_opacity));
        lines.push(format!(
            "titlebar_opacity_dark={}",
            self.titlebar_opacity_dark
        ));
        lines.push(format!(
            "toolbar_opacity_override={}",
            self.toolbar_opacity_override
        ));
        lines.push(format!("toolbar_opacity={}", self.toolbar_opacity));
        lines.push(format!(
            "toolbar_opacity_dark={}",
            self.toolbar_opacity_dark
        ));
        lines.push(format!("konsole_opacity={}", self.konsole_opacity));
        lines.push(format!(
            "konsole_opacity_dark={}",
            self.konsole_opacity_dark
        ));
        lines.push(format!("konsole_blur={}", self.konsole_blur));
        lines.push(format!(
            "klassy_windeco_outline={}",
            self.klassy_windeco_outline
        ));
        lines.push(format!(
            "darker_window_list={}",
            self.darker_window_list.as_deref().unwrap_or("")
        ));
        lines.push(format!("use_startup_delay={}", self.use_startup_delay));
        lines.push(format!("startup_delay={}", self.startup_delay));
        lines.push(format!(
            "plasma_follows_scheme={}",
            self.plasma_follows_scheme
        ));
        lines.push(format!(
            "pywal_follows_scheme={}",
            self.pywal_follows_scheme
        ));
        lines.push(format!("main_loop_delay={}", self.main_loop_delay));
        lines.push(format!("screenshot_delay={}", self.screenshot_delay));
        lines.push(format!("once_after_change={}", self.once_after_change));
        lines.push(format!("pause_mode={}", self.pause_mode));
        lines.push(format!(
            "screenshot_only_mode={}",
            self.screenshot_only_mode
        ));
        lines.push(format!("scheme_variant={}", self.scheme_variant));
        lines.push(format!("chroma_multiplier={}", self.chroma_multiplier));
        lines.push(format!("tone_multiplier={}", self.tone_multiplier));
        lines.push(format!("frame_contrast={}", self.frame_contrast));
        lines.push(format!("contrast_level={}", self.contrast_level));
        lines.push(format!(
            "qdbus_executable={}",
            self.qdbus_executable.as_deref().unwrap_or("")
        ));
        lines.push(format!("manual_fetch={}", self.manual_fetch));
        lines.push(format!("fetch_colors={}", self.fetch_colors));
        lines.push(format!("spec_version={}", self.spec_version));
        lines.push(format!(
            "kde_rounded_corners_effect_outline={}",
            self.kde_rounded_corners_effect_outline
        ));
        lines.push(String::new());
        lines.join("\n")
    }

    /// Write the INI representation of `self` to `path`, creating parent directories if needed.
    pub fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create config directory {}", parent.display())
                })?;
            }
        }
        std::fs::write(path, self.to_ini_string())
            .with_context(|| format!("Failed to write config file to {}", path.display()))
    }

    /// Compute the hex-encoded SHA-1 digest of the file at `path`.
    /// Returns `None` if the file does not exist or cannot be read.
    pub fn sha1_file(path: &Path) -> Option<String> {
        let bytes = std::fs::read(path).ok()?;
        let mut hasher = Sha1::new();
        hasher.update(&bytes);
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(40);
        for b in digest {
            use std::fmt::Write;
            let _ = write!(&mut hex, "{:02x}", b);
        }
        Some(hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_app_config_defaults_match_python_and_qml() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.monitor, 0);
        assert_eq!(cfg.file, None);
        assert_eq!(cfg.color, None);
        assert_eq!(cfg.custom_colors_list, None);
        assert_eq!(cfg.light, None);
        assert_eq!(cfg.ncolor, 0);
        assert_eq!(cfg.iconslight, None);
        assert_eq!(cfg.iconsdark, None);
        assert!(!cfg.pywal);
        assert_eq!(cfg.pywal_light, None);
        assert_eq!(cfg.light_blend_multiplier, 1.0);
        assert_eq!(cfg.dark_blend_multiplier, 1.0);
        assert_eq!(cfg.on_change_hook, None);
        assert!(!cfg.sierra_breeze_buttons_color);
        assert!(!cfg.disable_konsole);
        assert!(!cfg.titlebar_opacity_override);
        assert_eq!(cfg.titlebar_opacity, 100);
        assert_eq!(cfg.titlebar_opacity_dark, 100);
        assert!(!cfg.toolbar_opacity_override);
        assert_eq!(cfg.toolbar_opacity, 100);
        assert_eq!(cfg.toolbar_opacity_dark, 100);
        assert_eq!(cfg.konsole_opacity, 100);
        assert_eq!(cfg.konsole_opacity_dark, 100);
        assert!(!cfg.konsole_blur);
        assert!(!cfg.klassy_windeco_outline);
        assert_eq!(cfg.darker_window_list, None);
        assert!(!cfg.use_startup_delay);
        assert_eq!(cfg.startup_delay, 0);
        assert!(cfg.plasma_follows_scheme);
        assert!(cfg.pywal_follows_scheme);
        assert_eq!(cfg.main_loop_delay, 1.0);
        assert_eq!(cfg.screenshot_delay, 900.0);
        assert!(!cfg.once_after_change);
        assert!(!cfg.pause_mode);
        assert!(!cfg.screenshot_only_mode);
        assert_eq!(cfg.scheme_variant, 5);
        assert_eq!(cfg.chroma_multiplier, 1.0);
        assert_eq!(cfg.tone_multiplier, 1.0);
        assert_eq!(cfg.frame_contrast, 0.2);
        assert_eq!(cfg.contrast_level, 0.0);
        assert_eq!(cfg.qdbus_executable, None);
        assert!(!cfg.manual_fetch);
        assert!(!cfg.fetch_colors);
        assert_eq!(cfg.spec_version, "2025");
        assert!(!cfg.kde_rounded_corners_effect_outline);
    }

    #[test]
    fn test_ini_parsing_booleans_and_clamping() {
        let ini = r#"
[OTHER]
monitor = 99

[CUSTOM]
# Comment line
; Another comment
monitor = -5
ncolor = -3
light = YES
plasma_follows_scheme = 0
pywal = 1
kitty = on
pywal_light = True
pywal_follows_scheme = false
light_blend_multiplier = 9.5
dark_blend_multiplier = -2.0
titlebar_opacity_override = True
titlebar_opacity = 150
titlebar_opacity_dark = -10
toolbar_opacity_override = 1
toolbar_opacity = 200
toolbar_opacity_dark = -5
konsole_opacity = 120
konsole_opacity_dark = -1
konsole_blur = yes
scheme_variant = 42
chroma_multiplier = 0.1
tone_multiplier = 5.0
frame_contrast = 2.0
contrast_level = -3.0
color = 255, 0, 128
custom_colors_list = #ED8796 #A6DA95 #EED49F #8AADF4 #F5BDE6 #8BD5CA 245,169,127
kde_rounded_corners_effect_outline = true
"#;
        let cfg = AppConfig::from_ini_str(ini);
        assert_eq!(cfg.monitor, 0);
        assert_eq!(cfg.ncolor, 0);
        assert!(!cfg.plasma_follows_scheme);
        assert_eq!(cfg.light, Some(true));
        assert!(cfg.pywal);
        assert!(cfg.kitty);
        assert!(!cfg.pywal_follows_scheme);
        assert_eq!(cfg.pywal_light, Some(true));
        assert_eq!(cfg.light_blend_multiplier, 4.0);
        assert_eq!(cfg.dark_blend_multiplier, 0.0);
        assert!(cfg.titlebar_opacity_override);
        assert_eq!(cfg.titlebar_opacity, 100);
        assert_eq!(cfg.titlebar_opacity_dark, 0);
        assert!(cfg.toolbar_opacity_override);
        assert_eq!(cfg.toolbar_opacity, 100);
        assert_eq!(cfg.toolbar_opacity_dark, 0);
        assert_eq!(cfg.konsole_opacity, 100);
        assert_eq!(cfg.konsole_opacity_dark, 0);
        assert!(cfg.konsole_blur);
        assert_eq!(cfg.scheme_variant, 8);
        assert_eq!(cfg.chroma_multiplier, 0.5);
        assert_eq!(cfg.tone_multiplier, 1.5);
        assert_eq!(cfg.frame_contrast, 1.0);
        assert_eq!(cfg.contrast_level, -1.0);
        assert_eq!(cfg.color.as_deref(), Some("#ff0080"));
        assert_eq!(
            cfg.custom_colors_list,
            Some(vec![
                "#ed8796".to_string(),
                "#a6da95".to_string(),
                "#eed49f".to_string(),
                "#8aadf4".to_string(),
                "#f5bde6".to_string(),
                "#8bd5ca".to_string(),
                "#f5a97f".to_string(),
            ])
        );
        assert!(cfg.kde_rounded_corners_effect_outline);
    }

    #[test]
    fn test_missing_custom_section_uses_defaults() {
        let ini = "monitor = 4\nscheme_variant = 2\n";
        let cfg = AppConfig::from_ini_str(ini);
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn test_invalid_custom_colors_list_falls_back_to_none() {
        let ini = "[CUSTOM]\ncustom_colors_list = #ff0000 #00ff00\n";
        let cfg = AppConfig::from_ini_str(ini);
        assert_eq!(cfg.custom_colors_list, None);

        let ini_bad_hex =
            "[CUSTOM]\ncustom_colors_list = #ff0000 #00ff00 #0000ff #112233 #445566 #778899 notacolor\n";
        let cfg_bad = AppConfig::from_ini_str(ini_bad_hex);
        assert_eq!(cfg_bad.custom_colors_list, None);
    }

    #[test]
    fn test_cli_overrides_take_precedence_over_ini_and_defaults() {
        let ini = r#"
[CUSTOM]
monitor = 1
scheme_variant = 2
light = true
plasma_follows_scheme = false
konsole_opacity = 70
iconslight = Breeze
"#;
        let base_cfg = AppConfig::from_ini_str(ini);
        assert_eq!(base_cfg.monitor, 1);
        assert_eq!(base_cfg.scheme_variant, 2);
        assert_eq!(base_cfg.light, Some(true));

        let cli = CliArgs::parse_from([
            "kde-material-you-colors",
            "--monitor",
            "3",
            "--scheme-variant",
            "7",
            "--dark",
            "--konsole-opacity",
            "95",
            "--toolbar-opacity-override",
            "--toolbar-opacity",
            "80",
        ]);

        let merged = base_cfg.with_cli_overrides(&cli);
        assert_eq!(merged.monitor, 3);
        assert_eq!(merged.scheme_variant, 7);
        assert_eq!(merged.light, Some(false));
        assert!(!merged.plasma_follows_scheme);
        assert_eq!(merged.konsole_opacity, 95);
        assert!(merged.toolbar_opacity_override);
        assert_eq!(merged.toolbar_opacity, 80);
        // Preserved from INI since CLI did not override it
        assert_eq!(merged.iconslight.as_deref(), Some("Breeze"));
    }

    #[test]
    fn test_round_trip_ini_serialization_and_file_io_and_sha1() {
        let mut original = AppConfig::default();
        original.monitor = 2;
        original.file = Some("/home/user/wall.jpg".to_string());
        original.color = Some("#d0265c".to_string());
        original.custom_colors_list = Some(vec![
            "#d0265c".to_string(),
            "#74e448".to_string(),
            "#eece4f".to_string(),
            "#66a3ef".to_string(),
            "#532066".to_string(),
            "#297d81".to_string(),
            "#ccc1c1".to_string(),
        ]);
        original.plasma_follows_scheme = false;
        original.light = Some(true);
        original.pywal = true;
        original.kitty = true;
        original.pywal_follows_scheme = false;
        original.pywal_light = Some(false);
        original.titlebar_opacity_override = true;
        original.titlebar_opacity = 85;
        original.titlebar_opacity_dark = 80;
        original.toolbar_opacity_override = true;
        original.toolbar_opacity = 90;
        original.toolbar_opacity_dark = 75;
        original.konsole_opacity = 88;
        original.konsole_opacity_dark = 82;
        original.konsole_blur = true;
        original.scheme_variant = 6;
        original.chroma_multiplier = 1.5;
        original.tone_multiplier = 0.9;
        original.frame_contrast = 0.3;
        original.contrast_level = 0.25;
        original.manual_fetch = true;
        original.spec_version = "2021".to_string();
        original.kde_rounded_corners_effect_outline = true;

        let ini_str = original.to_ini_string();
        assert!(ini_str.starts_with("[CUSTOM]\n"));
        let parsed = AppConfig::from_ini_str(&ini_str);
        assert_eq!(parsed, original);

        let dir = tempfile::tempdir().expect("create tempdir");
        let conf_path = dir.path().join("nested").join("config.conf");

        // Non-existent file returns default config and None for sha1
        assert_eq!(AppConfig::sha1_file(&conf_path), None);
        assert_eq!(
            AppConfig::load_from_path(&conf_path).expect("load non-existent"),
            AppConfig::default()
        );

        original
            .save_to_path(&conf_path)
            .expect("save config to path");
        let loaded = AppConfig::load_from_path(&conf_path).expect("load saved config");
        assert_eq!(loaded, original);

        let sha1_a = AppConfig::sha1_file(&conf_path).expect("sha1 exists");
        assert_eq!(sha1_a.len(), 40);
        assert!(sha1_a.chars().all(|c| c.is_ascii_hexdigit()));

        // Modifying and saving changes the SHA-1 digest
        let mut modified = loaded.clone();
        modified.scheme_variant = 1;
        modified
            .save_to_path(&conf_path)
            .expect("save modified config");
        let sha1_b = AppConfig::sha1_file(&conf_path).expect("sha1 modified");
        assert_ne!(sha1_a, sha1_b);
    }
}
