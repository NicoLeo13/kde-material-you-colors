use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::apply::ini_utils::KdeIni;
use crate::apply::ThemePaths;
use crate::color::hex_to_alpha;
use crate::config::AppConfig;
use crate::scheme::{CustomRoleEntry, MaterialYouScheme};

/// Paths of all written KDE `.colors` scheme files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorSchemeArtifacts {
    pub light_colors: PathBuf,
    pub light2_colors: PathBuf,
    pub dark_colors: PathBuf,
    pub dark2_colors: PathBuf,
    pub light_darker_titlebar_colors: PathBuf,
    pub light_darker_titlebar2_colors: PathBuf,
    pub dark_darker_titlebar_colors: PathBuf,
    pub dark_darker_titlebar2_colors: PathBuf,
    pub active_scheme_hash: Option<String>,
}

fn role<'a>(map: &'a BTreeMap<String, String>, key: &str, fallback: &'a str) -> &'a str {
    map.get(key).map(String::as_str).unwrap_or(fallback)
}

fn extra_color<'a>(
    custom: &'a BTreeMap<String, CustomRoleEntry>,
    name: &str,
    light: bool,
    container: bool,
) -> &'a str {
    if let Some(entry) = custom.get(name) {
        let set = if light { &entry.light } else { &entry.dark };
        if container {
            set.container.as_str()
        } else {
            set.color.as_str()
        }
    } else {
        "#000000"
    }
}

/// Format a `f64` matching Python float formatting (`0.2`, `0.0`, `1.0`).
fn format_frame_contrast(fc: f64) -> String {
    let clamped = fc.clamp(0.0, 1.0);
    if clamped.fract() == 0.0 {
        format!("{clamped:.1}")
    } else {
        format!("{clamped}")
    }
}

/// Generate the complete KDE `.colors` INI content for `MaterialYouLight` (or darker titlebar variant),
/// matching `schemeconfigs.py` and `plasma_utils.py:plasma_darker_header`.
pub fn generate_light_color_scheme(
    scheme: &MaterialYouScheme,
    toolbar_opacity: u32,
    frame_contrast: f64,
    darker_header_bg: Option<&str>,
    scheme_display_name: &str,
) -> String {
    let cl = &scheme.schemes.light;
    let ex = &scheme.custom;
    let opacity = toolbar_opacity.clamp(0, 100) as u8;
    let fc_str = format_frame_contrast(frame_contrast);

    let header_bg_normal =
        darker_header_bg.unwrap_or_else(|| role(cl, "surfaceContainer", "#f0f0f0"));
    let active_bg = hex_to_alpha(role(cl, "surfaceContainerHighest", "#e0e0e0"), opacity);
    let inactive_bg = hex_to_alpha(role(cl, "secondaryContainer", "#e5e5e5"), opacity);

    format!(
        "[ColorEffects:Disabled]
Color={disabled_color}
ColorAmount=0.5
ColorEffect=3
ContrastAmount=0
ContrastEffect=0
IntensityAmount=0
IntensityEffect=0

[ColorEffects:Inactive]
ChangeSelectionColor=true
Color={inactive_color}
ColorAmount=0.025
ColorEffect=0
ContrastAmount=0.1
ContrastEffect=0
Enable=false
IntensityAmount=0
IntensityEffect=0

[Colors:Button]
BackgroundAlternate={surface_variant}
BackgroundNormal={surface_container_high}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={on_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Complementary]
BackgroundAlternate={surface}
BackgroundNormal={surface_container}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={inverse_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface_variant}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Header]
BackgroundAlternate={surface_container}
BackgroundNormal={header_bg_normal}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={inverse_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface_variant}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Header][Inactive]
BackgroundAlternate={surface_container}
BackgroundNormal={header_bg_normal}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={inverse_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface_variant}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Selection]
BackgroundAlternate={primary}
BackgroundNormal={primary}
DecorationFocus={primary}
DecorationHover={secondary}
ForegroundActive={on_primary}
ForegroundInactive={on_primary}
ForegroundLink={link_container}
ForegroundNegative={negative_container}
ForegroundNeutral={neutral_container}
ForegroundNormal={on_primary}
ForegroundPositive={positive_container}
ForegroundVisited={visited_container}

[Colors:Tooltip]
BackgroundAlternate={surface_variant}
BackgroundNormal={surface_container}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={on_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:View]
BackgroundAlternate={surface_container}
BackgroundNormal={surface_bright}
DecorationFocus={primary}
#-----------------------------------------------
DecorationHover={secondary_fixed}
ForegroundActive={inverse_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Window]
BackgroundAlternate={surface_variant}
BackgroundNormal={surface_container}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={link_color}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
#--- Window titles, context icons
ForegroundNormal={on_surface_variant}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[General]
ColorScheme=MaterialYouLight
Name={scheme_display_name}
shadeSortColumn=true

[KDE]
contrast=4
frameContrast={fc_str}

[WM]
activeBackground={active_bg}
activeBlend=227,229,231
activeForeground={on_surface}
inactiveBackground={inactive_bg}
inactiveBlend=239,240,241
inactiveForeground={on_surface_variant}
",
        disabled_color = role(cl, "surfaceContainer", "#f0f0f0"),
        inactive_color = role(cl, "surfaceContainerLowest", "#ffffff"),
        surface_variant = role(cl, "surfaceVariant", "#e0e0e0"),
        surface_container_high = role(cl, "surfaceContainerHigh", "#e8e8e8"),
        surface = role(cl, "surface", "#fafafa"),
        surface_container = role(cl, "surfaceContainer", "#f0f0f0"),
        surface_bright = role(cl, "surfaceBright", "#ffffff"),
        primary = role(cl, "primary", "#4285f4"),
        on_primary = role(cl, "onPrimary", "#ffffff"),
        secondary = role(cl, "secondary", "#5f6368"),
        secondary_fixed = role(cl, "secondaryFixed", "#d2e3fc"),
        on_surface = role(cl, "onSurface", "#1f1f1f"),
        on_surface_variant = role(cl, "onSurfaceVariant", "#444746"),
        inverse_surface = role(cl, "inverseSurface", "#303030"),
        outline = role(cl, "outline", "#747775"),
        link_color = extra_color(ex, "link", true, false),
        negative_color = extra_color(ex, "negative", true, false),
        neutral_color = extra_color(ex, "neutral", true, false),
        positive_color = extra_color(ex, "positive", true, false),
        visited_color = extra_color(ex, "visited", true, false),
        link_container = extra_color(ex, "link", true, true),
        negative_container = extra_color(ex, "negative", true, true),
        neutral_container = extra_color(ex, "neutral", true, true),
        positive_container = extra_color(ex, "positive", true, true),
        visited_container = extra_color(ex, "visited", true, true),
    )
}

/// Generate the complete KDE `.colors` INI content for `MaterialYouDark` (or darker titlebar variant),
/// matching `schemeconfigs.py` and `plasma_utils.py:plasma_darker_header`.
pub fn generate_dark_color_scheme(
    scheme: &MaterialYouScheme,
    toolbar_opacity_dark: u32,
    frame_contrast: f64,
    darker_header_bg: Option<&str>,
    scheme_display_name: &str,
) -> String {
    let cd = &scheme.schemes.dark;
    let ex = &scheme.custom;
    let opacity = toolbar_opacity_dark.clamp(0, 100) as u8;
    let fc_str = format_frame_contrast(frame_contrast);

    let header_bg_normal =
        darker_header_bg.unwrap_or_else(|| role(cd, "surfaceContainer", "#1e1e1e"));
    let active_bg = hex_to_alpha(role(cd, "surfaceContainerHighest", "#333333"), opacity);
    let inactive_bg = hex_to_alpha(role(cd, "secondaryContainer", "#2c2c2c"), opacity);

    format!(
        "[ColorEffects:Disabled]
Color={disabled_color}
ColorAmount=0.5
ColorEffect=3
ContrastAmount=0
ContrastEffect=0
IntensityAmount=0
IntensityEffect=0

[ColorEffects:Inactive]
ChangeSelectionColor=true
Color={inactive_color}
ColorAmount=0.025
ColorEffect=0
ContrastAmount=0.1
ContrastEffect=0
Enable=true
IntensityAmount=0
IntensityEffect=0

[Colors:Button]
BackgroundAlternate={surface_variant}
BackgroundNormal={surface_container_high}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={on_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Complementary]
BackgroundAlternate={surface}
BackgroundNormal={surface_container}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={inverse_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface_variant}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Header]
BackgroundAlternate={surface_container}
BackgroundNormal={header_bg_normal}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={inverse_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface_variant}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Header][Inactive]
BackgroundAlternate={surface_container}
BackgroundNormal={header_bg_normal}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={inverse_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface_variant}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Selection]
BackgroundAlternate={primary}
BackgroundNormal={primary}
DecorationFocus={primary}
DecorationHover={secondary}
ForegroundActive={on_primary}
ForegroundInactive={on_primary}
ForegroundLink={link_container}
ForegroundNegative={negative_container}
ForegroundNeutral={neutral_container}
ForegroundNormal={on_primary}
ForegroundPositive={positive_container}
ForegroundVisited={visited_container}

[Colors:Tooltip]
BackgroundAlternate={surface_variant}
BackgroundNormal={surface_container}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={on_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:View]
BackgroundAlternate={surface_container}
BackgroundNormal={surface_dim}
DecorationFocus={primary}
#-----------------------------------------------
DecorationHover={inverse_primary}
ForegroundActive={inverse_surface}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
ForegroundNormal={on_surface}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[Colors:Window]
BackgroundAlternate={surface_variant}
BackgroundNormal={surface_container}
DecorationFocus={primary}
DecorationHover={primary}
ForegroundActive={link_color}
ForegroundInactive={outline}
ForegroundLink={link_color}
ForegroundNegative={negative_color}
ForegroundNeutral={neutral_color}
#--- Window titles, context icons
ForegroundNormal={on_surface_variant}
ForegroundPositive={positive_color}
ForegroundVisited={visited_color}

[General]
ColorScheme=MaterialYouDark
Name={scheme_display_name}
shadeSortColumn=true

[KDE]
contrast=4
frameContrast={fc_str}

[WM]
activeBackground={active_bg}
activeBlend=252,252,252
activeForeground={on_surface}
inactiveBackground={inactive_bg}
inactiveBlend=161,169,177
inactiveForeground={on_secondary_container}
",
        disabled_color = role(cd, "surfaceContainer", "#1e1e1e"),
        inactive_color = role(cd, "surfaceContainerLowest", "#000000"),
        surface_variant = role(cd, "surfaceVariant", "#444746"),
        surface_container_high = role(cd, "surfaceContainerHigh", "#282828"),
        surface = role(cd, "surface", "#131313"),
        surface_container = role(cd, "surfaceContainer", "#1e1e1e"),
        surface_dim = role(cd, "surfaceDim", "#131313"),
        primary = role(cd, "primary", "#8ab4f8"),
        on_primary = role(cd, "onPrimary", "#002e6b"),
        inverse_primary = role(cd, "inversePrimary", "#1a73e8"),
        secondary = role(cd, "secondary", "#bbc7db"),
        on_secondary_container = role(cd, "onSecondaryContainer", "#d7e3f7"),
        on_surface = role(cd, "onSurface", "#e3e3e3"),
        on_surface_variant = role(cd, "onSurfaceVariant", "#c4c7c5"),
        inverse_surface = role(cd, "inverseSurface", "#e3e3e3"),
        outline = role(cd, "outline", "#8e918f"),
        link_color = extra_color(ex, "link", false, false),
        negative_color = extra_color(ex, "negative", false, false),
        neutral_color = extra_color(ex, "neutral", false, false),
        positive_color = extra_color(ex, "positive", false, false),
        visited_color = extra_color(ex, "visited", false, false),
        link_container = extra_color(ex, "link", false, true),
        negative_container = extra_color(ex, "negative", false, true),
        neutral_container = extra_color(ex, "neutral", false, true),
        positive_container = extra_color(ex, "positive", false, true),
        visited_container = extra_color(ex, "visited", false, true),
    )
}

/// Write all 8 `.colors` files (`MaterialYouLight`, `MaterialYouDark`, their `2.colors` copies,
/// and their `_darker_titlebar` / `_darker_titlebar2` variants) to `<data_dir>/color-schemes/`,
/// update `ColorSchemeHash` in `<config_dir>/kdeglobals`, and optionally invoke `plasma-apply-colorscheme`.
pub fn write_and_apply_color_schemes(
    scheme: &MaterialYouScheme,
    config: &AppConfig,
    paths: &ThemePaths,
    is_light: bool,
    run_external_commands: bool,
) -> Result<ColorSchemeArtifacts> {
    let schemes_dir = paths.color_schemes_dir();
    fs::create_dir_all(&schemes_dir).with_context(|| {
        format!(
            "failed to create color-schemes directory at {}",
            schemes_dir.display()
        )
    })?;

    let light_content = generate_light_color_scheme(
        scheme,
        config.toolbar_opacity,
        config.frame_contrast,
        None,
        "Material You Light",
    );
    let dark_content = generate_dark_color_scheme(
        scheme,
        config.toolbar_opacity_dark,
        config.frame_contrast,
        None,
        "Material You dark",
    );

    let light_darker_bg = scheme.pywal.light.special.background.as_str();
    let dark_darker_bg = scheme.pywal.dark.special.background.as_str();

    let light_darker_content = generate_light_color_scheme(
        scheme,
        config.toolbar_opacity,
        config.frame_contrast,
        Some(light_darker_bg),
        "Material You Light (darker titlebar)",
    );
    let light_darker2_content = generate_light_color_scheme(
        scheme,
        config.toolbar_opacity,
        config.frame_contrast,
        Some(light_darker_bg),
        "Material You Light (darker titlebar2)",
    );

    let dark_darker_content = generate_dark_color_scheme(
        scheme,
        config.toolbar_opacity_dark,
        config.frame_contrast,
        Some(dark_darker_bg),
        "Material You Dark (darker titlebar)",
    );
    let dark_darker2_content = generate_dark_color_scheme(
        scheme,
        config.toolbar_opacity_dark,
        config.frame_contrast,
        Some(dark_darker_bg),
        "Material You Dark (darker titlebar2)",
    );

    let light_colors = schemes_dir.join("MaterialYouLight.colors");
    let light2_colors = schemes_dir.join("MaterialYouLight2.colors");
    let dark_colors = schemes_dir.join("MaterialYouDark.colors");
    let dark2_colors = schemes_dir.join("MaterialYouDark2.colors");

    let light_darker_titlebar_colors =
        schemes_dir.join("MaterialYouLight_darker_titlebar.colors");
    let light_darker_titlebar2_colors =
        schemes_dir.join("MaterialYouLight_darker_titlebar2.colors");
    let dark_darker_titlebar_colors =
        schemes_dir.join("MaterialYouDark_darker_titlebar.colors");
    let dark_darker_titlebar2_colors =
        schemes_dir.join("MaterialYouDark_darker_titlebar2.colors");

    fs::write(&light_colors, &light_content)?;
    fs::write(&light2_colors, &light_content)?;
    fs::write(&dark_colors, &dark_content)?;
    fs::write(&dark2_colors, &dark_content)?;
    fs::write(&light_darker_titlebar_colors, &light_darker_content)?;
    fs::write(&light_darker_titlebar2_colors, &light_darker2_content)?;
    fs::write(&dark_darker_titlebar_colors, &dark_darker_content)?;
    fs::write(&dark_darker_titlebar2_colors, &dark_darker2_content)?;

    let (active_primary, active_copy) = if is_light {
        (&light_colors, &light2_colors)
    } else {
        (&dark_colors, &dark2_colors)
    };

    if run_external_commands {
        let _ = invoke_plasma_apply_colorscheme(active_copy, active_primary);
    }

    let active_scheme_hash = update_kdeglobals_hash(&paths.kdeglobals_path(), active_primary)?;

    Ok(ColorSchemeArtifacts {
        light_colors,
        light2_colors,
        dark_colors,
        dark2_colors,
        light_darker_titlebar_colors,
        light_darker_titlebar2_colors,
        dark_darker_titlebar_colors,
        dark_darker_titlebar2_colors,
        active_scheme_hash,
    })
}

/// Update `[General]` `ColorSchemeHash=<sha1>` in `<config_dir>/kdeglobals` based on `scheme_file`.
pub fn update_kdeglobals_hash(
    kdeglobals_path: &Path,
    scheme_file: &Path,
) -> Result<Option<String>> {
    let Some(hash) = AppConfig::sha1_file(scheme_file) else {
        return Ok(None);
    };

    let mut kdeglobals = KdeIni::load_or_default(kdeglobals_path)?;
    kdeglobals.set("General", "ColorSchemeHash", &hash);
    kdeglobals.save(kdeglobals_path)?;

    Ok(Some(hash))
}

/// Invoke `plasma-apply-colorscheme` on the copy `.colors` file and then the primary `.colors` file
/// to force Plasma to reload the scheme even when the scheme name is unchanged.
pub fn invoke_plasma_apply_colorscheme(copy_path: &Path, primary_path: &Path) -> Result<()> {
    let _ = Command::new("plasma-apply-colorscheme")
        .arg(copy_path)
        .output();
    let _ = Command::new("plasma-apply-colorscheme")
        .arg(primary_path)
        .output();
    Ok(())
}

/// Apply the configured icon theme (`iconslight` or `iconsdark`) using `plasma-changeicons` when `run_external_commands` is true.
/// Returns the selected icon theme name if non-empty.
pub fn apply_icon_theme(
    icons_light: Option<&str>,
    icons_dark: Option<&str>,
    is_light: bool,
    run_external_commands: bool,
) -> Result<Option<String>> {
    let selected = if is_light { icons_light } else { icons_dark }
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    if let Some(ref icon_theme) = selected {
        if run_external_commands {
            let candidates = [
                "/usr/lib/plasma-changeicons",
                "/usr/libexec/plasma-changeicons",
                "plasma-changeicons",
            ];
            for bin in candidates {
                if Command::new(bin).arg(icon_theme).output().is_ok() {
                    break;
                }
            }
        }
    }

    Ok(selected)
}
