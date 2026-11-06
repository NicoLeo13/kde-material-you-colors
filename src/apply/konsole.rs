use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::apply::ini_utils::{hex_to_rgb_csv, KdeIni};
use crate::apply::ThemePaths;
use crate::config::AppConfig;
use crate::scheme::{MaterialYouScheme, PywalScheme};

pub const KONSOLE_DEFAULT_THEMED_PROFILE: &str = "Profile 1";

/// Paths and metadata produced when applying Konsole schemes and profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KonsoleArtifacts {
    pub colorscheme_path: PathBuf,
    pub colorscheme_alt_path: PathBuf,
    pub active_profile_name: Option<String>,
    pub profile_path: Option<PathBuf>,
    pub temp_profile_path: Option<PathBuf>,
}

/// Format a Konsole opacity percentage (`0..=100`) as a float string (`"1.0"`, `"0.85"`, `"0.0"`),
/// matching Python `str(opacity / 100)`.
pub fn format_konsole_opacity(opacity_pct: u32) -> String {
    let val = (opacity_pct.clamp(0, 100) as f64) / 100.0;
    if val.fract() == 0.0 {
        format!("{val:.1}")
    } else {
        format!("{val}")
    }
}

/// Generate a Konsole `.colorscheme` INI string matching `konsole_utils.py:export_scheme`.
pub fn generate_konsole_colorscheme(
    pywal_scheme: &PywalScheme,
    description: &str,
    opacity_pct: u32,
    blur: bool,
) -> String {
    let mut ini = KdeIni::new();

    ini.set(
        "Background",
        "Color",
        &hex_to_rgb_csv(&pywal_scheme.special.background),
    );
    ini.set(
        "BackgroundIntense",
        "Color",
        &hex_to_rgb_csv(&pywal_scheme.special.background_intense),
    );
    ini.set(
        "BackgroundFaint",
        "Color",
        &hex_to_rgb_csv(&pywal_scheme.special.background_faint),
    );

    for i in 0..8 {
        let base_key = format!("color{i}");
        let intense_key = format!("color{}", i + 8);
        let faint_key = format!("color{}", i + 16);

        let c_base = pywal_scheme
            .colors
            .get(&base_key)
            .map(String::as_str)
            .unwrap_or("#000000");
        let c_intense = pywal_scheme
            .colors
            .get(&intense_key)
            .map(String::as_str)
            .unwrap_or("#000000");
        let c_faint = pywal_scheme
            .colors
            .get(&faint_key)
            .map(String::as_str)
            .unwrap_or("#000000");

        ini.set(&format!("Color{i}"), "Color", &hex_to_rgb_csv(c_base));
        ini.set(
            &format!("Color{i}Intense"),
            "Color",
            &hex_to_rgb_csv(c_intense),
        );
        ini.set(&format!("Color{i}Faint"), "Color", &hex_to_rgb_csv(c_faint));
    }

    ini.set(
        "Foreground",
        "Color",
        &hex_to_rgb_csv(&pywal_scheme.special.foreground),
    );
    ini.set(
        "ForegroundIntense",
        "Color",
        &hex_to_rgb_csv(&pywal_scheme.special.foreground_intense),
    );
    ini.set(
        "ForegroundFaint",
        "Color",
        &hex_to_rgb_csv(&pywal_scheme.special.foreground_faint),
    );

    ini.set("General", "Description", description);
    ini.set(
        "General",
        "Opacity",
        &format_konsole_opacity(opacity_pct),
    );
    ini.set(
        "General",
        "Blur",
        if blur { "true" } else { "false" },
    );

    ini.to_ini_string()
}

/// Read or initialize `DefaultProfile` in `<config_dir>/konsolerc`, returning the profile name
/// without the `.profile` suffix (matching `konsole_utils.py:set_default_profile`).
pub fn resolve_or_set_default_konsole_profile(
    konsolerc_path: &Path,
    fallback_profile_name: &str,
) -> Result<String> {
    if konsolerc_path.exists() {
        let content = fs::read_to_string(konsolerc_path)
            .with_context(|| format!("failed to read {}", konsolerc_path.display()))?;

        let mut found_line = false;
        let mut existing_profile = String::new();
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("DefaultProfile=") {
                found_line = true;
                existing_profile = rest.trim().trim_end_matches(".profile").to_string();
                break;
            }
        }

        if !existing_profile.is_empty() {
            return Ok(existing_profile);
        }

        let updated = if !found_line {
            if content.contains("[Desktop Entry]") {
                content.replacen(
                    "[Desktop Entry]",
                    &format!("[Desktop Entry]\nDefaultProfile={fallback_profile_name}.profile"),
                    1,
                )
            } else {
                format!("{content}[Desktop Entry]\nDefaultProfile={fallback_profile_name}.profile\n")
            }
        } else {
            content.replacen(
                "DefaultProfile=",
                &format!("DefaultProfile={fallback_profile_name}.profile"),
                1,
            )
        };

        fs::write(konsolerc_path, updated)
            .with_context(|| format!("failed to write {}", konsolerc_path.display()))?;
        Ok(fallback_profile_name.to_string())
    } else {
        if let Some(parent) = konsolerc_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let content = format!("[Desktop Entry]\nDefaultProfile={fallback_profile_name}.profile\n");
        fs::write(konsolerc_path, content)
            .with_context(|| format!("failed to create {}", konsolerc_path.display()))?;
        Ok(fallback_profile_name.to_string())
    }
}

/// Ensure `<konsole_dir>/<profile_name>.profile` and `<konsole_dir>/TempMyou.profile` exist and
/// reference `MaterialYou` and `MaterialYouAlt` respectively.
pub fn ensure_konsole_profiles(
    konsole_dir: &Path,
    profile_name: &str,
) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(konsole_dir).with_context(|| {
        format!(
            "failed to create Konsole directory at {}",
            konsole_dir.display()
        )
    })?;

    let profile_path = konsole_dir.join(format!("{profile_name}.profile"));
    let mut profile_ini = if profile_path.exists() {
        KdeIni::load_or_default(&profile_path)?
    } else {
        let mut new_ini = KdeIni::new();
        new_ini.set("General", "Name", profile_name);
        new_ini.set("General", "Parent", "FALLBACK/");
        new_ini
    };
    profile_ini.set("Appearance", "ColorScheme", "MaterialYou");
    profile_ini.save(&profile_path)?;

    let temp_profile_path = konsole_dir.join("TempMyou.profile");
    let mut temp_ini = KdeIni::new();
    temp_ini.set("Appearance", "ColorScheme", "MaterialYouAlt");
    temp_ini.set("General", "Name", "TempMyou");
    temp_ini.set("General", "Parent", profile_name);
    temp_ini.save(&temp_profile_path)?;

    Ok((profile_path, temp_profile_path))
}

/// Write `MaterialYou.colorscheme`, `MaterialYouAlt.colorscheme`, and (when `!config.disable_konsole`)
/// manage `konsolerc`, `<default_profile>.profile`, and `TempMyou.profile`.
pub fn write_konsole_schemes_and_profiles(
    scheme: &MaterialYouScheme,
    config: &AppConfig,
    paths: &ThemePaths,
    is_light: bool,
) -> Result<KonsoleArtifacts> {
    let konsole_dir = paths.konsole_dir();
    fs::create_dir_all(&konsole_dir).with_context(|| {
        format!(
            "failed to create Konsole directory at {}",
            konsole_dir.display()
        )
    })?;

    let use_light = config.pywal_light.or(config.light).unwrap_or(is_light);
    let pywal_scheme = if use_light {
        &scheme.pywal.light
    } else {
        &scheme.pywal.dark
    };
    let opacity_pct = if use_light {
        config.konsole_opacity
    } else {
        config.konsole_opacity_dark
    };

    let main_content =
        generate_konsole_colorscheme(pywal_scheme, "MaterialYou", opacity_pct, config.konsole_blur);
    let alt_content = generate_konsole_colorscheme(
        pywal_scheme,
        "MaterialYouAlt",
        opacity_pct,
        config.konsole_blur,
    );

    let colorscheme_path = konsole_dir.join("MaterialYou.colorscheme");
    let colorscheme_alt_path = konsole_dir.join("MaterialYouAlt.colorscheme");

    fs::write(&colorscheme_path, main_content)?;
    fs::write(&colorscheme_alt_path, alt_content)?;

    if config.disable_konsole {
        Ok(KonsoleArtifacts {
            colorscheme_path,
            colorscheme_alt_path,
            active_profile_name: None,
            profile_path: None,
            temp_profile_path: None,
        })
    } else {
        let profile_name = resolve_or_set_default_konsole_profile(
            &paths.konsolerc_path(),
            KONSOLE_DEFAULT_THEMED_PROFILE,
        )?;
        let (profile_path, temp_profile_path) =
            ensure_konsole_profiles(&konsole_dir, &profile_name)?;

        Ok(KonsoleArtifacts {
            colorscheme_path,
            colorscheme_alt_path,
            active_profile_name: Some(profile_name),
            profile_path: Some(profile_path),
            temp_profile_path: Some(temp_profile_path),
        })
    }
}
