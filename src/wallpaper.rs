use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::config::{normalize_color, AppConfig};
use crate::dbus_client;
use crate::scheme::WallpaperInfo;

pub const PICTURE_OF_DAY_PLUGIN: &str = "org.kde.potd";
pub const PLAIN_COLOR_PLUGIN: &str = "org.kde.color";
pub const IMAGE_WALLPAPER_PLUGIN: &str = "org.kde.image";
pub const SLIDESHOW_WALLPAPER_PLUGIN: &str = "org.kde.slideshow";
pub const POTD_DEFAULT_PROVIDER: &str = "apod";
pub const POTD_UNSPLASH_PROVIDER: &str = "unsplash";
pub const POTD_UNSPLASH_DEFAULT_CATEGORY: &str = "1065976";
pub const POTD_BING_PROVIDER: &str = "bing";

/// Resolved wallpaper source (either a normalized `#rrggbb` color or an image file path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallpaperSource {
    Color(String),
    Image(PathBuf),
}

impl WallpaperSource {
    /// Convert to `WallpaperInfo` for `MaterialYouScheme` runtime JSON export.
    pub fn to_wallpaper_info(&self) -> WallpaperInfo {
        match self {
            Self::Color(c) => WallpaperInfo {
                source_type: "color".to_string(),
                data: c.clone(),
            },
            Self::Image(p) => WallpaperInfo {
                source_type: "image".to_string(),
                data: p.to_string_lossy().into_owned(),
            },
        }
    }
}

/// Return the default KDE Plasma 6 desktop appletsrc path `~/.config/plasma-org.kde.plasma.desktop-appletsrc`.
pub fn default_appletsrc_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("plasma-org.kde.plasma.desktop-appletsrc"))
}

/// Return the default `kdeglobals` path `~/.config/kdeglobals`.
pub fn default_kdeglobals_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("kdeglobals"))
}

/// Return the default Picture of the Day cache directory `~/.cache/plasma_engine_potd`.
pub fn default_potd_cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("plasma_engine_potd"))
}

/// Return the default desktop screenshot path `/tmp/kde-material-you-colors-desktop-screenshot-$USER.png`.
pub fn default_screenshot_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "kde-material-you-colors-desktop-screenshot-{}.png",
        dbus_client::current_username()
    ))
}

/// Strip `file://` prefix and decode basic percent-encoded characters (such as `%20`).
pub fn clean_file_uri(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    let without_scheme = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    if without_scheme.contains('%') {
        percent_decode(without_scheme)
    } else {
        without_scheme.to_string()
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Find the smallest image inside `directory` by file size, preferring landscape (`width > height`)
/// over portrait (`width < height`) or square images, matching `file_utils.py:get_smallest_image`.
pub fn get_smallest_image(directory: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(directory).ok()?;
    let mut files_with_size: Vec<(PathBuf, u64)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let meta = e.metadata().ok()?;
            if meta.is_file() {
                Some((path, meta.len()))
            } else {
                None
            }
        })
        .collect();

    if files_with_size.is_empty() {
        return None;
    }

    files_with_size.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

    let mut landscape_imgs = Vec::new();
    let mut portrait_imgs = Vec::new();
    let mut valid_imgs = Vec::new();

    for (path, _) in &files_with_size {
        let dims = image::ImageReader::open(path)
            .and_then(|r| r.with_guessed_format())
            .ok()
            .and_then(|r| r.into_dimensions().ok());
        if let Some((w, h)) = dims {
            valid_imgs.push(path.clone());
            if w > h {
                landscape_imgs.push(path.clone());
            } else if w < h {
                portrait_imgs.push(path.clone());
            }
        }
    }

    if let Some(first) = landscape_imgs.into_iter().next() {
        return Some(first);
    }
    if let Some(first) = portrait_imgs.into_iter().next() {
        return Some(first);
    }
    valid_imgs.into_iter().next()
}

/// Resolve an image path or KDE wallpaper package directory (`contents/images_dark` or `contents/images`)
/// into a concrete image `PathBuf`, matching `get_wallpaper_image` in `wallpaper_utils.py`.
pub fn resolve_package_or_image_path(raw_path: &Path, light: bool) -> PathBuf {
    if !raw_path.is_dir() {
        return raw_path.to_path_buf();
    }

    let dark_path = raw_path.join("contents").join("images_dark");
    let normal_path = raw_path.join("contents").join("images");

    if !light && dark_path.is_dir() {
        if let Some(smallest) = get_smallest_image(&dark_path) {
            return smallest;
        }
    }
    if normal_path.is_dir() {
        if let Some(smallest) = get_smallest_image(&normal_path) {
            return smallest;
        }
    }
    if dark_path.is_dir() {
        if let Some(smallest) = get_smallest_image(&dark_path) {
            return smallest;
        }
    }
    if let Some(smallest) = get_smallest_image(raw_path) {
        return smallest;
    }

    raw_path.to_path_buf()
}

/// Resolve a Picture of the Day (`org.kde.potd`) cache file path inside `potd_cache_dir`
/// matching `get_picture_of_the_day` in `wallpaper_utils.py`.
pub fn resolve_potd_cache_path(
    provider: Option<&str>,
    category: Option<&str>,
    potd_cache_dir: &Path,
) -> PathBuf {
    let img_provider = provider
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(POTD_DEFAULT_PROVIDER);

    if img_provider.eq_ignore_ascii_case(POTD_UNSPLASH_PROVIDER) {
        let cat = category
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(POTD_UNSPLASH_DEFAULT_CATEGORY);
        return potd_cache_dir.join(format!("{POTD_UNSPLASH_PROVIDER}:{cat}"));
    }

    if img_provider.eq_ignore_ascii_case(POTD_BING_PROVIDER) {
        if let Ok(entries) = std::fs::read_dir(potd_cache_dir) {
            let mut bing_files: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| name.starts_with(POTD_BING_PROVIDER) && !name.ends_with("json"))
                .collect();
            bing_files.sort();
            if let Some(best) = bing_files.pop() {
                return potd_cache_dir.join(best);
            }
        }
    }

    potd_cache_dir.join(img_provider)
}

#[derive(Debug, Default, Clone)]
struct ContainmentEntry {
    last_screen: Option<u32>,
    plugin: Option<String>,
    wallpaper_plugin: Option<String>,
    wallpaper_general: HashMap<String, HashMap<String, String>>,
}

/// Parse KDE Plasma 6 `~/.config/plasma-org.kde.plasma.desktop-appletsrc` content for `monitor`
/// with explicit `potd_cache_dir` and `light` mode context.
pub fn parse_appletsrc_wallpaper_with_context(
    content: &str,
    monitor: u32,
    potd_cache_dir: Option<&Path>,
    light: bool,
) -> Option<WallpaperSource> {
    let mut containments: BTreeMap<String, ContainmentEntry> = BTreeMap::new();
    let mut current_containment_id: Option<String> = None;
    let mut current_wallpaper_plugin: Option<String> = None;
    let mut in_wallpaper_general = false;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_containment_id = None;
            current_wallpaper_plugin = None;
            in_wallpaper_general = false;

            let groups: Vec<&str> = line
                .split("][")
                .map(|g| g.trim_start_matches('[').trim_end_matches(']'))
                .collect();

            if groups.len() >= 2 && groups[0] == "Containments" {
                let cid = groups[1].to_string();
                containments.entry(cid.clone()).or_default();

                if groups.len() == 2 {
                    current_containment_id = Some(cid);
                } else if groups.len() >= 5
                    && groups[2] == "Wallpaper"
                    && groups[4].eq_ignore_ascii_case("General")
                {
                    current_containment_id = Some(cid);
                    current_wallpaper_plugin = Some(groups[3].to_string());
                    in_wallpaper_general = true;
                }
            }
            continue;
        }

        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let val = v.trim();
            if let Some(ref cid) = current_containment_id {
                let entry = containments.entry(cid.clone()).or_default();
                if in_wallpaper_general {
                    if let Some(ref wp_plugin) = current_wallpaper_plugin {
                        entry
                            .wallpaper_general
                            .entry(wp_plugin.clone())
                            .or_default()
                            .insert(key.to_string(), val.to_string());
                    }
                } else {
                    match key {
                        "lastScreen" => {
                            if let Ok(scr) = val.parse::<u32>() {
                                entry.last_screen = Some(scr);
                            }
                        }
                        "plugin" => {
                            entry.plugin = Some(val.to_string());
                        }
                        "wallpaperplugin" | "wallpaperPlugin" => {
                            entry.wallpaper_plugin = Some(val.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Select the best matching containment for `monitor`:
    // 1. Desktop containment with `lastScreen == Some(monitor)` and wallpaper settings
    // 2. Any containment with `lastScreen == Some(monitor)` and `wallpaper_plugin`
    // 3. Fallback when `monitor == 0` and no `lastScreen` was specified in minimal config snippets
    let is_panel = |e: &ContainmentEntry| {
        e.plugin
            .as_deref()
            .map(|p| p.contains("panel"))
            .unwrap_or(false)
    };

    let selected = containments
        .values()
        .find(|e| {
            e.last_screen == Some(monitor)
                && !is_panel(e)
                && (!e.wallpaper_general.is_empty() || e.wallpaper_plugin.is_some())
        })
        .or_else(|| {
            containments
                .values()
                .find(|e| e.last_screen == Some(monitor) && !e.wallpaper_general.is_empty())
        })
        .or_else(|| {
            if monitor == 0 && containments.values().all(|e| e.last_screen.is_none()) {
                containments
                    .values()
                    .find(|e| !e.wallpaper_general.is_empty() || e.wallpaper_plugin.is_some())
            } else {
                None
            }
        })?;

    let active_plugin = selected
        .wallpaper_plugin
        .clone()
        .or_else(|| {
            if selected.wallpaper_general.contains_key(IMAGE_WALLPAPER_PLUGIN) {
                Some(IMAGE_WALLPAPER_PLUGIN.to_string())
            } else if selected.wallpaper_general.contains_key(PLAIN_COLOR_PLUGIN) {
                Some(PLAIN_COLOR_PLUGIN.to_string())
            } else if selected.wallpaper_general.contains_key(PICTURE_OF_DAY_PLUGIN) {
                Some(PICTURE_OF_DAY_PLUGIN.to_string())
            } else {
                selected.wallpaper_general.keys().next().cloned()
            }
        })?;

    let general_map = selected.wallpaper_general.get(&active_plugin);

    if active_plugin == PLAIN_COLOR_PLUGIN {
        let raw_color = general_map
            .and_then(|m| m.get("Color").or_else(|| m.get("color")))
            .map(String::as_str)?;
        let normalized = normalize_color(raw_color).unwrap_or_else(|| raw_color.trim().to_string());
        return Some(WallpaperSource::Color(normalized));
    }

    if active_plugin == PICTURE_OF_DAY_PLUGIN {
        let provider = general_map
            .and_then(|m| m.get("Provider").or_else(|| m.get("provider")))
            .map(String::as_str);
        let category = general_map
            .and_then(|m| m.get("Category").or_else(|| m.get("category")))
            .map(String::as_str);
        let default_cache = default_potd_cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp/plasma_engine_potd"));
        let cache_dir = potd_cache_dir.unwrap_or(&default_cache);
        let potd_path = resolve_potd_cache_path(provider, category, cache_dir);
        return Some(WallpaperSource::Image(potd_path));
    }

    // Default image/slideshow or custom image-based wallpaper plugin
    let raw_image = general_map
        .and_then(|m| {
            m.get("Image")
                .or_else(|| m.get("image"))
                .or_else(|| m.get("PreviewImage"))
        })
        .map(String::as_str)?;

    let cleaned = clean_file_uri(raw_image);
    if cleaned.is_empty() {
        return None;
    }

    let resolved = resolve_package_or_image_path(Path::new(&cleaned), light);
    Some(WallpaperSource::Image(resolved))
}

/// Parse KDE Plasma 6 `~/.config/plasma-org.kde.plasma.desktop-appletsrc` directly
/// (`[Containments][*]` with `lastScreen=monitor` and `org.kde.image`, `org.kde.color`, or `org.kde.potd`).
pub fn parse_appletsrc_wallpaper(content: &str, monitor: u32) -> Option<WallpaperSource> {
    let default_cache = default_potd_cache_dir();
    parse_appletsrc_wallpaper_with_context(content, monitor, default_cache.as_deref(), false)
}

/// Read `kdeglobals` to determine whether the active Plasma color scheme is in light mode:
/// 1. `[General] ColorScheme` containing "light" -> `Some(true)` (e.g. `MaterialYouLight`, `BreezeLight`)
///    `[General] ColorScheme` containing "dark" -> `Some(false)` (e.g. `MaterialYouDark`, `BreezeDark`)
/// 2. `[General] ColorSchemeHash` matching SHA-1 of `dark_scheme_path` -> `Some(false)`
///    `[General] ColorSchemeHash` matching SHA-1 of `light_scheme_path` -> `Some(true)`
/// 3. Fallback to `[Colors:Window] BackgroundNormal` relative luminance (`> 0.18` -> `Some(true)`, else `Some(false)`)
pub fn read_kdeglobals_light_mode(
    kdeglobals_path: &Path,
    light_scheme_path: &Path,
    dark_scheme_path: &Path,
) -> Option<bool> {
    let content = std::fs::read_to_string(kdeglobals_path).ok()?;
    let mut in_general = false;
    let mut in_colors_window = false;
    let mut color_scheme: Option<String> = None;
    let mut color_scheme_hash: Option<String> = None;
    let mut background_normal: Option<String> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let section = line[1..line.len() - 1].trim();
            in_general = section.eq_ignore_ascii_case("General");
            in_colors_window = section.eq_ignore_ascii_case("Colors:Window");
            continue;
        }
        if in_general {
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "ColorScheme" => color_scheme = Some(v.trim().to_string()),
                    "ColorSchemeHash" => color_scheme_hash = Some(v.trim().to_string()),
                    _ => {}
                }
            }
        } else if in_colors_window {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim().eq_ignore_ascii_case("BackgroundNormal") {
                    background_normal = Some(v.trim().to_string());
                }
            }
        }
    }

    // 1. Check ColorScheme name for generic Light / Dark patterns
    if let Some(ref scheme) = color_scheme {
        let lower = scheme.to_ascii_lowercase();
        if lower.contains("light") {
            return Some(true);
        }
        if lower.contains("dark") {
            return Some(false);
        }
    }

    // 2. Check ColorSchemeHash against generated dark / light theme files
    if let Some(ref hash) = color_scheme_hash {
        if !hash.is_empty() {
            if let Some(dark_sha1) = AppConfig::sha1_file(dark_scheme_path) {
                if hash.eq_ignore_ascii_case(&dark_sha1) {
                    return Some(false);
                }
            }
            if let Some(light_sha1) = AppConfig::sha1_file(light_scheme_path) {
                if hash.eq_ignore_ascii_case(&light_sha1) {
                    return Some(true);
                }
            }
        }
    }

    // 3. Fallback to [Colors:Window] BackgroundNormal relative luminance
    if let Some(ref bg_normal) = background_normal {
        if let Some(hex) = normalize_color(bg_normal) {
            let lum = crate::color::color_luminance(&hex);
            return Some(lum > 0.18);
        }
        let parts: Vec<&str> = bg_normal
            .split([',', ' ', '\t'])
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() == 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                parts[0].parse::<u8>(),
                parts[1].parse::<u8>(),
                parts[2].parse::<u8>(),
            ) {
                let hex = crate::color::rgb_to_hex(r, g, b);
                let lum = crate::color::color_luminance(&hex);
                return Some(lum > 0.18);
            }
        }
    }

    None
}

/// Check if a file is a plain-text pointer file (UTF-8 text and not a recognized binary image format).
fn is_plain_text_pointer(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() || bytes.len() > 16 * 1024 {
        return None;
    }
    if image::guess_format(&bytes).is_ok() {
        return None;
    }
    let text = std::str::from_utf8(&bytes).ok()?;
    let first_non_empty = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))?;
    Some(clean_file_uri(first_non_empty))
}

fn resolve_from_dbus_map(
    wallpaper_config: &HashMap<String, String>,
    potd_cache_dir: &Path,
    light: bool,
) -> Option<WallpaperSource> {
    let plugin = wallpaper_config
        .get("wallpaperPlugin")
        .or_else(|| wallpaper_config.get("wallpaperplugin"))
        .map(String::as_str)
        .unwrap_or("");

    if plugin == PICTURE_OF_DAY_PLUGIN {
        let provider = wallpaper_config
            .get("Provider")
            .or_else(|| wallpaper_config.get("provider"))
            .map(String::as_str);
        let category = wallpaper_config
            .get("Category")
            .or_else(|| wallpaper_config.get("category"))
            .map(String::as_str);
        let potd_path = resolve_potd_cache_path(provider, category, potd_cache_dir);
        if potd_path.exists() {
            return Some(WallpaperSource::Image(potd_path));
        }
    }

    if let Some(raw_img) = wallpaper_config
        .get("Image")
        .or_else(|| wallpaper_config.get("image"))
        .or_else(|| wallpaper_config.get("PreviewImage"))
    {
        let cleaned = clean_file_uri(raw_img);
        if !cleaned.is_empty() {
            let resolved = resolve_package_or_image_path(Path::new(&cleaned), light);
            if resolved.exists() && resolved.is_file() {
                return Some(WallpaperSource::Image(resolved));
            }
        }
    }

    if let Some(raw_color) = wallpaper_config
        .get("color")
        .or_else(|| wallpaper_config.get("Color"))
    {
        if let Some(hex) = normalize_color(raw_color) {
            return Some(WallpaperSource::Color(hex));
        }
        if let Ok(rgb_int) = raw_color.trim().parse::<u32>() {
            return Some(WallpaperSource::Color(format!(
                "#{:06x}",
                rgb_int & 0xFF_FFFF
            )));
        }
    }

    None
}

fn resolve_local_sources(
    config: &AppConfig,
    appletsrc_path: &Path,
    potd_cache_dir: &Path,
) -> Result<Option<WallpaperSource>> {
    let light = config.light.unwrap_or(false);

    if config.screenshot_only_mode {
        return Ok(None);
    }

    // 1. Precedence 1: config.color
    if let Some(ref raw_color) = config.color {
        let trimmed = raw_color.trim();
        if !trimmed.is_empty() {
            if let Some(normalized) = normalize_color(trimmed) {
                return Ok(Some(WallpaperSource::Color(normalized)));
            } else {
                return Err(anyhow!("Error: Color format '{trimmed}' is incorrect"));
            }
        }
    }

    // 2. Precedence 2: config.file (image path, package dir, or plain-text file containing an image path)
    if let Some(ref raw_file) = config.file {
        let cleaned = clean_file_uri(raw_file);
        if !cleaned.is_empty() {
            let file_path = PathBuf::from(&cleaned);
            if !file_path.exists() {
                return Err(anyhow!("File '{}' does not exist", file_path.display()));
            }

            let target_path = if file_path.is_file() {
                if let Some(pointer_target) = is_plain_text_pointer(&file_path) {
                    PathBuf::from(pointer_target)
                } else {
                    file_path
                }
            } else {
                file_path
            };

            if !target_path.exists() {
                return Err(anyhow!(
                    "Resolved file '{}' does not exist",
                    target_path.display()
                ));
            }

            let resolved = resolve_package_or_image_path(&target_path, light);
            if !resolved.exists() || !resolved.is_file() {
                return Err(anyhow!(
                    "Could not resolve a valid image from '{}'",
                    target_path.display()
                ));
            }
            return Ok(Some(WallpaperSource::Image(resolved)));
        }
    }

    // 3. Precedence 3: parse_appletsrc_wallpaper (with org.kde.potd cache lookup in potd_cache_dir)
    if appletsrc_path.exists() {
        if let Ok(content) = std::fs::read_to_string(appletsrc_path) {
            if let Some(source) = parse_appletsrc_wallpaper_with_context(
                &content,
                config.monitor,
                Some(potd_cache_dir),
                light,
            ) {
                match source {
                    WallpaperSource::Color(c) => return Ok(Some(WallpaperSource::Color(c))),
                    WallpaperSource::Image(p) => {
                        if p.exists() && p.is_file() {
                            return Ok(Some(WallpaperSource::Image(p)));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

/// Async version of `resolve_wallpaper` following the exact precedence from `wallpaper_utils.py`:
/// 1. `config.color` (`WallpaperSource::Color`)
/// 2. `config.file` (image path, package directory, or plain-text file pointer)
/// 3. `parse_appletsrc_wallpaper` (with `org.kde.potd` cache lookup in `potd_cache_dir`)
/// 4. Async D-Bus `query_plasma_wallpaper` fallback / KWin `ScreenShot2` fallback.
pub async fn resolve_wallpaper_async(
    config: &AppConfig,
    appletsrc_path: &Path,
    potd_cache_dir: &Path,
) -> Result<WallpaperSource> {
    if let Some(source) = resolve_local_sources(config, appletsrc_path, potd_cache_dir)? {
        return Ok(source);
    }

    let light = config.light.unwrap_or(false);

    if !config.screenshot_only_mode {
        if let Ok(dbus_map) = dbus_client::query_plasma_wallpaper(config.monitor).await {
            if let Some(source) = resolve_from_dbus_map(&dbus_map, potd_cache_dir, light) {
                return Ok(source);
            }
        }
    }

    let screenshot_path = default_screenshot_path();
    if dbus_client::capture_workspace_screenshot(&screenshot_path)
        .await
        .unwrap_or(false)
        && screenshot_path.exists()
    {
        return Ok(WallpaperSource::Image(screenshot_path));
    }

    Err(anyhow!(
        "Could not resolve wallpaper from config, appletsrc, PlasmaShell D-Bus, or KWin ScreenShot2"
    ))
}

/// Resolve the active wallpaper following the exact precedence from `wallpaper_utils.py`:
/// 1. `config.color` (`WallpaperSource::Color`)
/// 2. `config.file` (image path, package directory, or plain-text file containing an image path)
/// 3. `parse_appletsrc_wallpaper` (with `org.kde.potd` cache lookup in `potd_cache_dir`)
/// 4. Async D-Bus `query_plasma_wallpaper` fallback / KWin `ScreenShot2` fallback.
pub fn resolve_wallpaper(
    config: &AppConfig,
    appletsrc_path: &Path,
    potd_cache_dir: &Path,
) -> Result<WallpaperSource> {
    if let Some(source) = resolve_local_sources(config, appletsrc_path, potd_cache_dir)? {
        return Ok(source);
    }

    let config_clone = config.clone();
    let appletsrc_buf = appletsrc_path.to_path_buf();
    let potd_buf = potd_cache_dir.to_path_buf();

    // Execute D-Bus / KWin fallback cleanly whether called inside or outside a Tokio runtime
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("Failed to build current-thread Tokio runtime for D-Bus fallback")?;
            rt.block_on(resolve_wallpaper_async(&config_clone, &appletsrc_buf, &potd_buf))
        })
        .join()
        .map_err(|_| anyhow!("Wallpaper D-Bus fallback thread panicked"))?
    } else {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to build current-thread Tokio runtime for D-Bus fallback")?;
        rt.block_on(resolve_wallpaper_async(&config_clone, &appletsrc_buf, &potd_buf))
    }
}

/// High-level change notifications emitted by `ConfigAndWallpaperWatcher`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// `~/.config/kde-material-you-colors/config.conf` was created or modified.
    ConfigChanged(PathBuf),
    /// `~/.config/plasma-org.kde.plasma.desktop-appletsrc` was created or modified.
    WallpaperChanged(PathBuf),
    /// `~/.config/kdeglobals` was created or modified.
    KdeGlobalsChanged(PathBuf),
    /// `/tmp/kde-material-you-colors-$USER.reload` was written by another instance.
    ReloadTriggered(PathBuf),
}

/// Event-driven filesystem watcher using `notify::RecommendedWatcher` for:
/// - `~/.config/kde-material-you-colors/config.conf`
/// - `~/.config/plasma-org.kde.plasma.desktop-appletsrc`
/// - `~/.config/kdeglobals`
/// - `/tmp/kde-material-you-colors-$USER.reload`
///
/// Watches parent directories non-recursively so atomic `KConfig`/`QSaveFile` renames
/// and newly created `.reload` trigger files are never missed.
pub struct ConfigAndWallpaperWatcher {
    _watcher: RecommendedWatcher,
    rx: Receiver<WatchEvent>,
    config_path: PathBuf,
    appletsrc_path: PathBuf,
    kdeglobals_path: PathBuf,
    reload_path: PathBuf,
}

impl ConfigAndWallpaperWatcher {
    /// Create a watcher for explicit file paths.
    pub fn new(
        config_path: PathBuf,
        appletsrc_path: PathBuf,
        kdeglobals_path: PathBuf,
        reload_path: PathBuf,
    ) -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        let cfg_target = config_path.clone();
        let app_target = appletsrc_path.clone();
        let kde_target = kdeglobals_path.clone();
        let rel_target = reload_path.clone();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            let Ok(event) = res else {
                return;
            };
            if matches!(
                event.kind,
                EventKind::Access(_) | EventKind::Other
            ) {
                return;
            }
            for path in &event.paths {
                let file_name = path.file_name();
                if path == &cfg_target
                    || (file_name.is_some()
                        && file_name == cfg_target.file_name()
                        && path.parent() == cfg_target.parent())
                {
                    let _ = tx.send(WatchEvent::ConfigChanged(cfg_target.clone()));
                } else if path == &app_target
                    || (file_name.is_some()
                        && file_name == app_target.file_name()
                        && path.parent() == app_target.parent())
                {
                    let _ = tx.send(WatchEvent::WallpaperChanged(app_target.clone()));
                } else if path == &kde_target
                    || (file_name.is_some()
                        && file_name == kde_target.file_name()
                        && path.parent() == kde_target.parent())
                {
                    let _ = tx.send(WatchEvent::KdeGlobalsChanged(kde_target.clone()));
                } else if path == &rel_target
                    || (file_name.is_some()
                        && file_name == rel_target.file_name()
                        && path.parent() == rel_target.parent())
                {
                    let _ = tx.send(WatchEvent::ReloadTriggered(rel_target.clone()));
                }
            }
        })
        .context("Failed to initialize notify::RecommendedWatcher")?;

        for target in [&config_path, &appletsrc_path, &kdeglobals_path, &reload_path] {
            if let Some(parent) = target.parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                    if parent.exists() {
                        let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
                    }
                }
            }
            if target.exists() {
                let _ = watcher.watch(target, RecursiveMode::NonRecursive);
            }
        }

        Ok(Self {
            _watcher: watcher,
            rx,
            config_path,
            appletsrc_path,
            kdeglobals_path,
            reload_path,
        })
    }

    /// Create a watcher configured with the standard user session paths.
    pub fn with_default_paths() -> Result<Self> {
        let config_path = AppConfig::default_path()
            .ok_or_else(|| anyhow!("Could not resolve user config directory"))?;
        let appletsrc_path = default_appletsrc_path()
            .ok_or_else(|| anyhow!("Could not resolve plasma appletsrc path"))?;
        let kdeglobals_path = default_kdeglobals_path()
            .ok_or_else(|| anyhow!("Could not resolve kdeglobals path"))?;
        let reload_path = dbus_client::default_reload_trigger_path();
        Self::new(config_path, appletsrc_path, kdeglobals_path, reload_path)
    }

    /// Non-blocking poll for the next queued `WatchEvent`.
    pub fn try_recv(&self) -> Option<WatchEvent> {
        self.rx.try_recv().ok()
    }

    /// Blocking wait for the next `WatchEvent` up to `timeout`.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<WatchEvent> {
        self.rx.recv_timeout(timeout).ok()
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn appletsrc_path(&self) -> &Path {
        &self.appletsrc_path
    }

    pub fn kdeglobals_path(&self) -> &Path {
        &self.kdeglobals_path
    }

    pub fn reload_path(&self) -> &Path {
        &self.reload_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn write_test_png(path: &Path, width: u32, height: u32, rgba: [u8; 4]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width, height, Rgba(rgba));
        img.save_with_format(path, image::ImageFormat::Png).unwrap();
    }

    #[test]
    fn test_parse_appletsrc_wallpaper_image_uri_and_package_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pkg_dir = dir.path().join("NextWallpaper");
        let img_small = pkg_dir
            .join("contents")
            .join("images")
            .join("1920x1080.png");
        let img_large = pkg_dir
            .join("contents")
            .join("images")
            .join("3840x2160.png");
        let img_dark = pkg_dir
            .join("contents")
            .join("images_dark")
            .join("1920x1080.png");

        write_test_png(&img_small, 16, 9, [66, 133, 244, 255]);
        write_test_png(&img_large, 64, 36, [66, 133, 244, 255]);
        write_test_png(&img_dark, 16, 9, [20, 40, 80, 255]);

        let appletsrc = format!(
            r#"
[Containments][1]
activityId=
formfactor=2
lastScreen=0
plugin=org.kde.panel

[Containments][2]
activityId=99d4a2b7-1111-2222-3333-444455556666
formfactor=0
lastScreen=0
plugin=org.kde.plasma.folder
wallpaperplugin=org.kde.image

[Containments][2][Wallpaper][org.kde.image][General]
Image=file://{pkg}

[Containments][3]
activityId=99d4a2b7-1111-2222-3333-444455556666
formfactor=0
lastScreen=1
plugin=org.kde.plasma.folder
wallpaperplugin=org.kde.image

[Containments][3][Wallpaper][org.kde.image][General]
Image=file:///usr/share/wallpapers/Custom%20Wall/hero.png
"#,
            pkg = pkg_dir.display()
        );

        // Monitor 0 resolves dark smallest image by default (light=false)
        let src_mon0_dark = parse_appletsrc_wallpaper(&appletsrc, 0);
        assert_eq!(src_mon0_dark, Some(WallpaperSource::Image(img_dark)));

        // Monitor 0 with light=true resolves normal contents/images smallest landscape image
        let src_mon0_light =
            parse_appletsrc_wallpaper_with_context(&appletsrc, 0, None, true);
        assert_eq!(src_mon0_light, Some(WallpaperSource::Image(img_small)));

        // Monitor 1 resolves file:// URI with %20 decoded
        let src_mon1 = parse_appletsrc_wallpaper(&appletsrc, 1);
        assert_eq!(
            src_mon1,
            Some(WallpaperSource::Image(PathBuf::from(
                "/usr/share/wallpapers/Custom Wall/hero.png"
            )))
        );
    }

    #[test]
    fn test_parse_appletsrc_wallpaper_color_and_potd() {
        let dir = tempfile::tempdir().expect("tempdir");
        let potd_cache = dir.path().join("plasma_engine_potd");
        std::fs::create_dir_all(&potd_cache).unwrap();

        let bing_cached = potd_cache.join("bing:2026-11-05");
        write_test_png(&bing_cached, 16, 9, [10, 20, 30, 255]);
        std::fs::write(potd_cache.join("bing.json"), "{}").unwrap();

        let appletsrc = r#"
[Containments][10]
lastScreen=0
wallpaperplugin=org.kde.color

[Containments][10][Wallpaper][org.kde.color][General]
Color=66,133,244

[Containments][11]
lastScreen=1
wallpaperplugin=org.kde.color

[Containments][11][Wallpaper][org.kde.color][General]
Color=#FF5722

[Containments][12]
lastScreen=2
wallpaperplugin=org.kde.potd

[Containments][12][Wallpaper][org.kde.potd][General]
Provider=bing

[Containments][13]
lastScreen=3
wallpaperplugin=org.kde.potd

[Containments][13][Wallpaper][org.kde.potd][General]
Provider=unsplash
Category=
"#;

        assert_eq!(
            parse_appletsrc_wallpaper(appletsrc, 0),
            Some(WallpaperSource::Color("#4285f4".to_string()))
        );
        assert_eq!(
            parse_appletsrc_wallpaper(appletsrc, 1),
            Some(WallpaperSource::Color("#ff5722".to_string()))
        );
        assert_eq!(
            parse_appletsrc_wallpaper_with_context(appletsrc, 2, Some(&potd_cache), false),
            Some(WallpaperSource::Image(bing_cached))
        );
        assert_eq!(
            parse_appletsrc_wallpaper_with_context(appletsrc, 3, Some(&potd_cache), false),
            Some(WallpaperSource::Image(potd_cache.join("unsplash:1065976")))
        );
    }

    #[test]
    fn test_read_kdeglobals_light_mode_by_name_and_sha1_hash() {
        let dir = tempfile::tempdir().expect("tempdir");
        let kdeglobals = dir.path().join("kdeglobals");
        let light_scheme = dir.path().join("MaterialYouLight.colors");
        let dark_scheme = dir.path().join("MaterialYouDark.colors");

        std::fs::write(&light_scheme, "[General]\nName=Material You Light\n").unwrap();
        std::fs::write(&dark_scheme, "[General]\nName=Material You Dark\n").unwrap();

        let light_hash = AppConfig::sha1_file(&light_scheme).unwrap();
        let dark_hash = AppConfig::sha1_file(&dark_scheme).unwrap();

        // 1. By scheme name MaterialYouLight
        std::fs::write(
            &kdeglobals,
            "[General]\nColorScheme=MaterialYouLight\n",
        )
        .unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(true)
        );

        // 2. By scheme name MaterialYouDark
        std::fs::write(
            &kdeglobals,
            "[General]\nColorScheme=MaterialYouDark2\n",
        )
        .unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(false)
        );

        // 3. By ColorSchemeHash when ColorScheme is BreezeClassic
        std::fs::write(
            &kdeglobals,
            format!("[General]\nColorScheme=BreezeClassic\nColorSchemeHash={light_hash}\n"),
        )
        .unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(true)
        );

        std::fs::write(
            &kdeglobals,
            format!("[General]\nColorScheme=BreezeClassic\nColorSchemeHash={dark_hash}\n"),
        )
        .unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(false)
        );

        // 4. Unknown scheme and hash returns None
        std::fs::write(
            &kdeglobals,
            "[General]\nColorScheme=BreezeClassic\nColorSchemeHash=deadbeef\n",
        )
        .unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            None
        );
    }

    #[test]
    fn test_read_kdeglobals_light_mode_generic_schemes_and_window_background() {
        let dir = tempfile::tempdir().expect("tempdir");
        let kdeglobals = dir.path().join("kdeglobals");
        let light_scheme = dir.path().join("dummy_light.colors");
        let dark_scheme = dir.path().join("dummy_dark.colors");
        std::fs::write(&light_scheme, "").unwrap();
        std::fs::write(&dark_scheme, "").unwrap();

        // 1. Generic KDE BreezeLight scheme
        std::fs::write(&kdeglobals, "[General]\nColorScheme=BreezeLight\n").unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(true)
        );

        // 2. Generic KDE BreezeDark scheme
        std::fs::write(&kdeglobals, "[General]\nColorScheme=BreezeDark\n").unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(false)
        );

        // 3. Custom scheme with light [Colors:Window] BackgroundNormal (RGB format)
        std::fs::write(
            &kdeglobals,
            "[General]\nColorScheme=CustomTheme\n\n[Colors:Window]\nBackgroundNormal=239,240,241\n",
        )
        .unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(true)
        );

        // 4. Custom scheme with dark [Colors:Window] BackgroundNormal (RGB format)
        std::fs::write(
            &kdeglobals,
            "[General]\nColorScheme=CustomTheme\n\n[Colors:Window]\nBackgroundNormal=24,26,31\n",
        )
        .unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(false)
        );

        // 5. Custom scheme with hex format in [Colors:Window] BackgroundNormal
        std::fs::write(
            &kdeglobals,
            "[General]\nColorScheme=CustomTheme\n\n[Colors:Window]\nBackgroundNormal=#eff0f1\n",
        )
        .unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(true)
        );

        std::fs::write(
            &kdeglobals,
            "[General]\nColorScheme=CustomTheme\n\n[Colors:Window]\nBackgroundNormal=#181a1f\n",
        )
        .unwrap();
        assert_eq!(
            read_kdeglobals_light_mode(&kdeglobals, &light_scheme, &dark_scheme),
            Some(false)
        );
    }

    #[test]
    fn test_resolve_wallpaper_precedence_color_file_pointer_appletsrc_package() {
        let dir = tempfile::tempdir().expect("tempdir");
        let potd_cache = dir.path().join("potd");
        std::fs::create_dir_all(&potd_cache).unwrap();

        let direct_img = dir.path().join("direct.png");
        let pointed_img = dir.path().join("pointed.png");
        let applet_img = dir.path().join("applet.png");
        write_test_png(&direct_img, 16, 9, [255, 0, 0, 255]);
        write_test_png(&pointed_img, 16, 9, [0, 255, 0, 255]);
        write_test_png(&applet_img, 16, 9, [0, 0, 255, 255]);

        let text_pointer = dir.path().join("current_wallpaper.txt");
        std::fs::write(&text_pointer, format!("file://{}\n", pointed_img.display())).unwrap();

        let appletsrc = dir.path().join("plasma-org.kde.plasma.desktop-appletsrc");
        std::fs::write(
            &appletsrc,
            format!(
                "[Containments][1]\nlastScreen=0\nwallpaperplugin=org.kde.image\n\n[Containments][1][Wallpaper][org.kde.image][General]\nImage=file://{}\n",
                applet_img.display()
            ),
        )
        .unwrap();

        // 1. config.color wins over config.file and appletsrc
        let mut cfg = AppConfig::default();
        cfg.color = Some("255, 0, 128".to_string());
        cfg.file = Some(direct_img.to_string_lossy().into_owned());
        let resolved_color = resolve_wallpaper(&cfg, &appletsrc, &potd_cache).unwrap();
        assert_eq!(
            resolved_color,
            WallpaperSource::Color("#ff0080".to_string())
        );

        // 2. config.file (direct image) wins over appletsrc when config.color is None
        cfg.color = None;
        let resolved_direct = resolve_wallpaper(&cfg, &appletsrc, &potd_cache).unwrap();
        assert_eq!(resolved_direct, WallpaperSource::Image(direct_img));

        // 3. config.file (plain-text file pointer) resolves pointed_img
        cfg.file = Some(text_pointer.to_string_lossy().into_owned());
        let resolved_pointer = resolve_wallpaper(&cfg, &appletsrc, &potd_cache).unwrap();
        assert_eq!(resolved_pointer, WallpaperSource::Image(pointed_img));

        // 4. appletsrc resolves when config.color and config.file are None
        cfg.file = None;
        let resolved_applet = resolve_wallpaper(&cfg, &appletsrc, &potd_cache).unwrap();
        assert_eq!(resolved_applet, WallpaperSource::Image(applet_img));
    }

    #[test]
    fn test_config_and_wallpaper_watcher_emits_events() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg_path = dir.path().join("config.conf");
        let app_path = dir.path().join("plasma-org.kde.plasma.desktop-appletsrc");
        let kde_path = dir.path().join("kdeglobals");
        let rel_path = dir.path().join("kde-material-you-colors-test.reload");

        let watcher = ConfigAndWallpaperWatcher::new(
            cfg_path.clone(),
            app_path.clone(),
            kde_path.clone(),
            rel_path.clone(),
        )
        .unwrap();

        std::fs::write(&rel_path, "reload:1\n").unwrap();
        let ev = watcher.recv_timeout(Duration::from_secs(2));
        assert_eq!(ev, Some(WatchEvent::ReloadTriggered(rel_path)));
    }
}
