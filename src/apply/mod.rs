pub mod color_schemes;
pub mod ini_utils;
pub mod konsole;
pub mod ksyntax;
pub mod pywal;
pub mod kitty;
pub mod titlebar;

use std::path::{Path, PathBuf};

use anyhow::Result;

pub use color_schemes::{
    apply_icon_theme, generate_dark_color_scheme, generate_light_color_scheme,
    invoke_plasma_apply_colorscheme, update_kdeglobals_hash, write_and_apply_color_schemes,
    ColorSchemeArtifacts,
};
pub use ini_utils::{hex_to_rgb_csv, KdeIni};
pub use kitty::{generate_kitty_theme, write_kitty_themes, KittyArtifacts};
pub use konsole::{
    ensure_konsole_profiles, format_konsole_opacity, generate_konsole_colorscheme,
    resolve_or_set_default_konsole_profile, write_konsole_schemes_and_profiles, KonsoleArtifacts,
    KONSOLE_DEFAULT_THEMED_PROFILE,
};
pub use ksyntax::{
    generate_ksyntax_dark_theme, generate_ksyntax_light_theme, write_ksyntax_themes,
    KSyntaxArtifacts,
};
pub use pywal::{
    generate_pywal_colors_css, generate_pywal_colors_plain, generate_pywal_colors_sh,
    spawn_on_change_hook, write_pywal_files, PywalArtifacts,
};
pub use titlebar::{
    apply_kde_rounded_corners_outline, apply_klassy_windeco_outline,
    apply_kwin_darker_titlebar_rule, apply_sierra_breeze_button_colors, apply_titlebar_opacity,
    compute_sierra_breeze_colors, SierraBreezeButtonColors, DARKER_TITLEBAR_RULE_DESCRIPTION,
};

use crate::config::AppConfig;
use crate::scheme::MaterialYouScheme;

/// Configurable base directories for all KDE theme application targets.
/// Defaults to the current user's `~/.local/share`, `~/.config`, `~/.cache`, and `/tmp`,
/// while allowing 100% isolated testing inside a `tempfile::tempdir()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePaths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub username: String,
}

impl Default for ThemePaths {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let data_dir = dirs::data_local_dir().unwrap_or_else(|| home.join(".local/share"));
        let config_dir = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
        let cache_dir = dirs::cache_dir().unwrap_or_else(|| home.join(".cache"));
        let temp_dir = std::env::temp_dir();
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "user".to_string());

        Self {
            data_dir,
            config_dir,
            cache_dir,
            temp_dir,
            username,
        }
    }
}

impl ThemePaths {
    /// Construct isolated `ThemePaths` rooted inside `root` (e.g. a `tempfile::TempDir`).
    pub fn in_root(root: &Path) -> Self {
        Self {
            data_dir: root.join(".local/share"),
            config_dir: root.join(".config"),
            cache_dir: root.join(".cache"),
            temp_dir: root.join("tmp"),
            username: "testuser".to_string(),
        }
    }

    pub fn color_schemes_dir(&self) -> PathBuf {
        self.data_dir.join("color-schemes")
    }

    pub fn kdeglobals_path(&self) -> PathBuf {
        self.config_dir.join("kdeglobals")
    }

    pub fn konsole_dir(&self) -> PathBuf {
        self.data_dir.join("konsole")
    }

    pub fn konsolerc_path(&self) -> PathBuf {
        self.config_dir.join("konsolerc")
    }

    pub fn ksyntax_themes_dir(&self) -> PathBuf {
        self.data_dir
            .join("org.kde.syntax-highlighting")
            .join("themes")
    }

    pub fn breezerc_path(&self) -> PathBuf {
        self.config_dir.join("breezerc")
    }

    pub fn sberc_path(&self) -> PathBuf {
        self.config_dir.join("sierrabreezeenhancedrc")
    }

    pub fn klassyrc_path(&self) -> PathBuf {
        self.config_dir.join("klassy").join("klassyrc")
    }

    pub fn kwinrc_path(&self) -> PathBuf {
        self.config_dir.join("kwinrc")
    }

    pub fn kwinrulesrc_path(&self) -> PathBuf {
        self.config_dir.join("kwinrulesrc")
    }

    pub fn wal_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("wal")
    }

    pub fn kitty_dir(&self) -> PathBuf {
        self.config_dir.join("kitty")
    }

    pub fn kitty_themes_dir(&self) -> PathBuf {
        self.config_dir.join("kitty").join("themes")
    }

    pub fn kitty_current_theme_path(&self) -> PathBuf {
        self.config_dir.join("kitty").join("current-theme.conf")
    }

    pub fn runtime_json_path(&self) -> PathBuf {
        self.temp_dir
            .join(format!("kde-material-you-colors-{}.json", self.username))
    }

    pub fn konsole_active_profile_path(&self) -> PathBuf {
        self.temp_dir.join(format!(
            "kde-material-you-colors-konsole-profile-{}",
            self.username
        ))
    }
}

/// Summary of all files and side effects written by `apply_all_themes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedArtifacts {
    pub runtime_json_path: PathBuf,
    pub color_schemes: ColorSchemeArtifacts,
    pub ksyntax: KSyntaxArtifacts,
    pub konsole: KonsoleArtifacts,
    pub pywal: Option<PywalArtifacts>,
    pub kitty: Option<KittyArtifacts>,
    pub applied_icon_theme: Option<String>,
    pub kwin_darker_rule_id: Option<String>,
    pub needs_kwin_reload: bool,
    pub hook_configured: bool,
}

/// Apply all KDE theme targets (runtime JSON, `.colors` schemes, `kdeglobals` hash,
/// KSyntaxHighlighting `.theme` files, Konsole `.colorscheme` and `.profile` files,
/// SierraBreeze/Klassy/KWin window decoration configs, Pywal cache files, and user hook).
pub fn apply_all_themes(
    scheme: &MaterialYouScheme,
    config: &AppConfig,
    paths: &ThemePaths,
    run_external_commands: bool,
) -> Result<AppliedArtifacts> {
    let is_light = config.light.unwrap_or(scheme.light);
    let is_pywal_light = config
        .pywal_light
        .or(config.light)
        .unwrap_or(scheme.pywal_light);

    // 1. Export runtime JSON (/tmp/kde-material-you-colors-$USER.json)
    let runtime_json_path = paths.runtime_json_path();
    let mut scheme_for_json = scheme.clone();
    scheme_for_json.light = is_light;
    scheme_for_json.pywal_light = is_pywal_light;
    scheme_for_json.write_runtime_json(&runtime_json_path)?;

    // 2. Write and apply KDE .colors schemes + update ColorSchemeHash in kdeglobals
    let color_schemes = write_and_apply_color_schemes(
        scheme,
        config,
        paths,
        is_light,
        run_external_commands,
    )?;

    // 3. Export KSyntaxHighlighting JSON themes
    let ksyntax = write_ksyntax_themes(scheme, paths)?;

    // 4. Apply icon theme if configured
    let applied_icon_theme = apply_icon_theme(
        config.iconslight.as_deref(),
        config.iconsdark.as_deref(),
        is_light,
        run_external_commands,
    )?;

    // 5. Window decorations, Titlebars & KWin rules
    let mut needs_kwin_reload = false;
    if config.sierra_breeze_buttons_color {
        apply_sierra_breeze_button_colors(scheme, is_light, &paths.breezerc_path())?;
        needs_kwin_reload = true;
    }
    if config.klassy_windeco_outline {
        apply_klassy_windeco_outline(scheme, is_light, &paths.klassyrc_path())?;
        needs_kwin_reload = true;
    }
    if config.kde_rounded_corners_effect_outline {
        apply_kde_rounded_corners_outline(scheme, is_light, &paths.kwinrc_path())?;
        needs_kwin_reload = true;
    }
    if config.titlebar_opacity_override {
        apply_titlebar_opacity(
            config.titlebar_opacity,
            config.titlebar_opacity_dark,
            is_light,
            paths,
        )?;
        needs_kwin_reload = true;
    }

    let kwin_darker_rule_id = if let Some(ref win_list) = config.darker_window_list {
        if !win_list.trim().is_empty() {
            let rule_light = config.pywal_light.unwrap_or(is_light);
            Some(apply_kwin_darker_titlebar_rule(
                rule_light,
                Some(win_list),
                &paths.kwinrulesrc_path(),
            )?)
        } else {
            None
        }
    } else {
        None
    };

    // 6. Konsole .colorscheme and .profile files
    let konsole = write_konsole_schemes_and_profiles(scheme, config, paths, is_light)?;

    // 7. Pywal files (~/.cache/wal/colors.json, colors, colors.sh, colors.css)
    let pywal = if config.pywal {
        Some(write_pywal_files(
            scheme,
            is_pywal_light,
            paths,
            run_external_commands,
        )?)
    } else {
        None
    };

    // 8. Kitty themes (~/.config/kitty/themes/material-you-dark.conf, etc.)
    let kitty = if config.kitty {
        Some(write_kitty_themes(
            scheme,
            paths,
            run_external_commands,
        )?)
    } else {
        None
    };

    // 9. User hook (on_change_hook)
    let hook_configured =
        spawn_on_change_hook(config.on_change_hook.as_deref(), run_external_commands)?;

    Ok(AppliedArtifacts {
        runtime_json_path,
        color_schemes,
        ksyntax,
        konsole,
        pywal,
        kitty,
        applied_icon_theme,
        kwin_darker_rule_id,
        needs_kwin_reload,
        hook_configured,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheme::{SchemeRequest, SchemeVariant};
    use serde_json::Value;
    use std::fs;
    use tempfile::tempdir;

    fn sample_scheme() -> MaterialYouScheme {
        let req = SchemeRequest {
            source_color_hex: Some("#4285f4".to_string()),
            variant: SchemeVariant::TonalSpot,
            chroma_multiplier: 1.0,
            tone_multiplier: 1.0,
            contrast_level: 0.0,
            frame_contrast: 0.25,
            light: false,
            pywal_light: false,
            ..Default::default()
        };
        MaterialYouScheme::generate(&req).expect("failed to generate sample scheme")
    }

    fn assert_valid_rgb_csv(s: &str) {
        let parts: Vec<&str> = s.split(',').collect();
        assert_eq!(parts.len(), 3, "expected 3 comma-separated RGB values, got '{s}'");
        for part in parts {
            let _val: u8 = part
                .parse()
                .unwrap_or_else(|e| panic!("invalid u8 component '{part}' in '{s}': {e}"));
        }
    }

    #[test]
    fn test_kde_colors_files_sections_rgb_frame_contrast_and_opacity() {
        let dir = tempdir().expect("tempdir");
        let paths = ThemePaths::in_root(dir.path());
        let scheme = sample_scheme();
        let mut config = AppConfig::default();
        config.frame_contrast = 0.35;
        config.toolbar_opacity = 80;
        config.toolbar_opacity_dark = 60;

        let artifacts =
            write_and_apply_color_schemes(&scheme, &config, &paths, false, false).unwrap();

        assert!(artifacts.light_colors.exists());
        assert!(artifacts.light2_colors.exists());
        assert!(artifacts.dark_colors.exists());
        assert!(artifacts.dark2_colors.exists());
        assert!(artifacts.light_darker_titlebar_colors.exists());
        assert!(artifacts.light_darker_titlebar2_colors.exists());
        assert!(artifacts.dark_darker_titlebar_colors.exists());
        assert!(artifacts.dark_darker_titlebar2_colors.exists());

        let dark_text = fs::read_to_string(&artifacts.dark_colors).unwrap();
        let light_text = fs::read_to_string(&artifacts.light_colors).unwrap();

        for required_section in [
            "[ColorEffects:Disabled]",
            "[ColorEffects:Inactive]",
            "[Colors:Button]",
            "[Colors:Complementary]",
            "[Colors:Header]",
            "[Colors:Header][Inactive]",
            "[Colors:Selection]",
            "[Colors:Tooltip]",
            "[Colors:View]",
            "[Colors:Window]",
            "[General]",
            "[KDE]",
            "[WM]",
        ] {
            assert!(
                dark_text.contains(required_section),
                "missing section {required_section} in MaterialYouDark.colors"
            );
            assert!(
                light_text.contains(required_section),
                "missing section {required_section} in MaterialYouLight.colors"
            );
        }

        // Check frameContrast and toolbar opacity alpha values
        assert!(dark_text.contains("frameContrast=0.35"));
        assert!(light_text.contains("frameContrast=0.35"));
        // 60% of 255 -> ceil(60 * 2.55) = 153 = 0x99
        assert!(dark_text.contains("activeBackground=#99"));
        // 80% of 255 -> ceil(80 * 2.55) = 204 = 0xcc
        assert!(light_text.contains("activeBackground=#cc"));

        // Verify valid r,g,b values in [WM] activeBlend / inactiveBlend
        let dark_ini = KdeIni::parse(&dark_text);
        assert_valid_rgb_csv(dark_ini.get("WM", "activeBlend").unwrap());
        assert_valid_rgb_csv(dark_ini.get("WM", "inactiveBlend").unwrap());
        let light_ini = KdeIni::parse(&light_text);
        assert_valid_rgb_csv(light_ini.get("WM", "activeBlend").unwrap());
        assert_valid_rgb_csv(light_ini.get("WM", "inactiveBlend").unwrap());

        // Check darker titlebar variant overrides Header BackgroundNormal with pywal special.background
        let dark_darker_text = fs::read_to_string(&artifacts.dark_darker_titlebar_colors).unwrap();
        let dark_darker_ini = KdeIni::parse(&dark_darker_text);
        assert_eq!(
            dark_darker_ini.get("Colors:Header", "BackgroundNormal"),
            Some(scheme.pywal.dark.special.background.as_str())
        );
        assert_eq!(
            dark_darker_ini.get("Colors:Header][Inactive", "BackgroundNormal"),
            Some(scheme.pywal.dark.special.background.as_str())
        );
        assert_eq!(
            dark_darker_ini.get("General", "Name"),
            Some("Material You Dark (darker titlebar)")
        );

        // Verify ColorSchemeHash was written to kdeglobals
        let kdeglobals_ini = KdeIni::load_or_default(&paths.kdeglobals_path()).unwrap();
        let stored_hash = kdeglobals_ini.get("General", "ColorSchemeHash").unwrap();
        assert_eq!(Some(stored_hash.to_string()), artifacts.active_scheme_hash);
        assert_eq!(stored_hash.len(), 40);
    }

    #[test]
    fn test_konsole_colorscheme_opacity_blur_and_profiles() {
        let dir = tempdir().expect("tempdir");
        let paths = ThemePaths::in_root(dir.path());
        let scheme = sample_scheme();
        let mut config = AppConfig::default();
        config.konsole_opacity_dark = 85;
        config.konsole_opacity = 95;
        config.konsole_blur = true;
        config.disable_konsole = false;

        let konsole_out =
            write_konsole_schemes_and_profiles(&scheme, &config, &paths, false).unwrap();

        assert!(konsole_out.colorscheme_path.exists());
        assert!(konsole_out.colorscheme_alt_path.exists());

        let main_ini = KdeIni::load_or_default(&konsole_out.colorscheme_path).unwrap();
        assert_eq!(main_ini.get("General", "Description"), Some("MaterialYou"));
        assert_eq!(main_ini.get("General", "Opacity"), Some("0.85"));
        assert_eq!(main_ini.get("General", "Blur"), Some("true"));

        assert_valid_rgb_csv(main_ini.get("Background", "Color").unwrap());
        assert_valid_rgb_csv(main_ini.get("Foreground", "Color").unwrap());
        for i in 0..8 {
            assert_valid_rgb_csv(main_ini.get(&format!("Color{i}"), "Color").unwrap());
            assert_valid_rgb_csv(main_ini.get(&format!("Color{i}Intense"), "Color").unwrap());
            assert_valid_rgb_csv(main_ini.get(&format!("Color{i}Faint"), "Color").unwrap());
        }

        let alt_ini = KdeIni::load_or_default(&konsole_out.colorscheme_alt_path).unwrap();
        assert_eq!(alt_ini.get("General", "Description"), Some("MaterialYouAlt"));

        let temp_profile_path = konsole_out.temp_profile_path.expect("temp profile");
        let temp_ini = KdeIni::load_or_default(&temp_profile_path).unwrap();
        assert_eq!(
            temp_ini.get("Appearance", "ColorScheme"),
            Some("MaterialYouAlt")
        );
        assert_eq!(temp_ini.get("General", "Name"), Some("TempMyou"));
        assert_eq!(
            temp_ini.get("General", "Parent"),
            Some(KONSOLE_DEFAULT_THEMED_PROFILE)
        );
    }

    #[test]
    fn test_ksyntax_highlighting_themes_valid_json() {
        let dir = tempdir().expect("tempdir");
        let paths = ThemePaths::in_root(dir.path());
        let scheme = sample_scheme();

        let ksyntax = write_ksyntax_themes(&scheme, &paths).unwrap();
        assert!(ksyntax.dark_theme_path.exists());
        assert!(ksyntax.light_theme_path.exists());

        for (path, expected_name) in [
            (&ksyntax.dark_theme_path, "Material You Dark"),
            (&ksyntax.light_theme_path, "Material You Light"),
        ] {
            let content = fs::read_to_string(path).unwrap();
            let parsed: Value = serde_json::from_str(&content).expect("valid JSON");
            assert_eq!(parsed["metadata"]["name"], expected_name);
            assert!(parsed["editor-colors"].is_object());
            assert!(parsed["editor-colors"]["BackgroundColor"]
                .as_str()
                .unwrap()
                .starts_with('#'));
            assert!(parsed["text-styles"].is_object());
            assert!(parsed["text-styles"]["Normal"]["text-color"]
                .as_str()
                .unwrap()
                .starts_with('#'));
            assert!(parsed["text-styles"]["Keyword"].is_object());
            assert!(parsed["custom-styles"].is_object());
        }
    }

    #[test]
    fn test_titlebar_klassy_kwin_rules_and_apply_all_themes_isolated() {
        let dir = tempdir().expect("tempdir");
        let paths = ThemePaths::in_root(dir.path());
        let scheme = sample_scheme();

        let mut config = AppConfig::default();
        config.sierra_breeze_buttons_color = true;
        config.klassy_windeco_outline = true;
        config.kde_rounded_corners_effect_outline = true;
        config.titlebar_opacity_override = true;
        config.titlebar_opacity = 90;
        config.titlebar_opacity_dark = 75;
        config.darker_window_list = Some("konsole dolphin org.kde.kate".to_string());
        config.pywal = true;
        config.kitty = true;
        config.on_change_hook = Some("echo theme_changed".to_string());

        let applied = apply_all_themes(&scheme, &config, &paths, false).unwrap();

        assert!(applied.runtime_json_path.exists());
        assert!(applied.needs_kwin_reload);
        assert!(applied.hook_configured);
        assert_eq!(applied.kwin_darker_rule_id.as_deref(), Some("1"));

        // Verify breezerc has SierraBreeze button RGB values and BackgroundOpacity
        let breeze_ini = KdeIni::load_or_default(&paths.breezerc_path()).unwrap();
        assert_valid_rgb_csv(breeze_ini.get("Windeco", "ButtonCloseActiveColor").unwrap());
        assert_valid_rgb_csv(
            breeze_ini
                .get("Windeco", "ButtonCloseInactiveColor")
                .unwrap(),
        );
        assert_eq!(breeze_ini.get("Windeco", "BackgroundOpacity"), Some("75"));

        // Verify sierrabreezeenhancedrc
        let sbe_ini = KdeIni::load_or_default(&paths.sberc_path()).unwrap();
        assert_eq!(sbe_ini.get("Windeco", "BackgroundOpacity"), Some("75"));

        // Verify klassy/klassyrc has outline colors and titlebar opacity
        let klassy_ini = KdeIni::load_or_default(&paths.klassyrc_path()).unwrap();
        assert_eq!(
            klassy_ini.get("TitleBarOpacity", "ActiveTitleBarOpacity"),
            Some("75")
        );
        assert_eq!(
            klassy_ini.get("WindowOutlineStyle", "WindowOutlineStyleActive"),
            Some("WindowOutlineCustomColor")
        );
        assert_eq!(
            klassy_ini.get("WindowOutlineStyle", "WindowOutlineCustomColorActive"),
            scheme.schemes.dark.get("primaryFixed").map(String::as_str)
        );

        // Verify kwinrc [Round-Corners]
        let kwin_ini = KdeIni::load_or_default(&paths.kwinrc_path()).unwrap();
        assert_valid_rgb_csv(kwin_ini.get("Round-Corners", "OutlineColor").unwrap());
        assert_valid_rgb_csv(
            kwin_ini
                .get("Round-Corners", "InactiveOutlineColor")
                .unwrap(),
        );
        assert_eq!(
            kwin_ini.get("Round-Corners", "ActiveOutlineAlpha"),
            Some("255")
        );

        // Verify kwinrulesrc darker titlebar rule and scheme toggling on second application
        let rules_ini = KdeIni::load_or_default(&paths.kwinrulesrc_path()).unwrap();
        assert_eq!(rules_ini.get("General", "rules"), Some("1"));
        assert_eq!(
            rules_ini.get("1", "Description"),
            Some(DARKER_TITLEBAR_RULE_DESCRIPTION)
        );
        assert_eq!(
            rules_ini.get("1", "decocolor"),
            Some("MaterialYouDark_darker_titlebar")
        );
        assert_eq!(
            rules_ini.get("1", "wmclass"),
            Some("(konsole|dolphin|org.kde.kate)")
        );

        // Second application toggles decocolor to MaterialYouDark_darker_titlebar2
        let _ = apply_all_themes(&scheme, &config, &paths, false).unwrap();
        let rules_ini_second = KdeIni::load_or_default(&paths.kwinrulesrc_path()).unwrap();
        assert_eq!(
            rules_ini_second.get("1", "decocolor"),
            Some("MaterialYouDark_darker_titlebar2")
        );

        // Verify pywal cache files
        let pywal_artifacts = applied.pywal.expect("pywal artifacts");
        assert!(pywal_artifacts.colors_json_path.exists());
        assert!(pywal_artifacts.colors_plain_path.exists());
        assert!(pywal_artifacts.colors_sh_path.exists());
        assert!(pywal_artifacts.colors_css_path.exists());

        let plain_content = fs::read_to_string(&pywal_artifacts.colors_plain_path).unwrap();
        let plain_lines: Vec<&str> = plain_content.lines().collect();
        assert_eq!(plain_lines.len(), 16);
        assert!(plain_lines.iter().all(|l| l.starts_with('#') && l.len() == 7));

        // Verify kitty theme files and symlink
        let kitty_artifacts = applied.kitty.expect("kitty artifacts");
        assert!(kitty_artifacts.dark_theme_path.exists());
        assert!(kitty_artifacts.light_theme_path.exists());
        #[cfg(unix)]
        {
            assert!(fs::symlink_metadata(&kitty_artifacts.current_theme_symlink).is_ok());
            let target = fs::read_link(&kitty_artifacts.current_theme_symlink).unwrap();
            assert_eq!(target, Path::new("themes/material-you-dark.conf"));
        }
    }
}
