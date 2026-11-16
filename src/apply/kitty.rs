use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::apply::ThemePaths;
use crate::scheme::{MaterialYouScheme, PywalScheme};

/// Paths and status produced when exporting Kitty terminal themes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KittyArtifacts {
    pub dark_theme_path: PathBuf,
    pub light_theme_path: PathBuf,
    pub current_theme_symlink: PathBuf,
    pub reloaded: bool,
}

fn color_n<'a>(scheme: &'a PywalScheme, idx: usize) -> &'a str {
    let key = format!("color{idx}");
    scheme
        .colors
        .get(&key)
        .map(String::as_str)
        .unwrap_or("#000000")
}

/// Generate a Kitty `.conf` theme file string with metadata headers for `kitten themes`.
pub fn generate_kitty_theme(pywal_scheme: &PywalScheme, theme_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("## name: {}\n", theme_name));
    out.push_str("## author: kde-material-you-colors\n");
    out.push_str("## upstream: https://github.com/luisbocanegra/kde-material-you-colors\n\n");

    out.push_str("# 16-color palette\n");
    for i in 0..16 {
        out.push_str(&format!("color{:<2} {}\n", i, color_n(pywal_scheme, i)));
    }
    out.push('\n');

    out.push_str("# Special colors\n");
    let bg = &pywal_scheme.special.background;
    let fg = &pywal_scheme.special.foreground;
    let cursor = &pywal_scheme.special.cursor;

    out.push_str(&format!("background            {}\n", bg));
    out.push_str(&format!("foreground            {}\n", fg));
    out.push_str(&format!("cursor                {}\n", cursor));
    out.push_str(&format!("cursor_text_color     {}\n", bg));
    out.push_str(&format!("selection_foreground  {}\n", bg));
    out.push_str(&format!("selection_background  {}\n", cursor));
    out.push('\n');

    out.push_str("# Window borders\n");
    out.push_str(&format!("active_border_color   {}\n", cursor));
    out.push_str(&format!(
        "inactive_border_color {}\n",
        &pywal_scheme.special.background_intense
    ));
    let color4 = color_n(pywal_scheme, 4);
    out.push_str(&format!("url_color             {}\n\n", color4));

    out.push_str("# Tab bar\n");
    out.push_str(&format!("active_tab_foreground   {}\n", bg));
    out.push_str(&format!("active_tab_background   {}\n", cursor));
    out.push_str(&format!("inactive_tab_foreground {}\n", fg));
    out.push_str(&format!(
        "inactive_tab_background {}\n\n",
        &pywal_scheme.special.background_intense
    ));

    out.push_str("# Cursor trail\n");
    out.push_str(&format!("cursor_trail_color      {}\n", fg));

    out
}

/// Write `material-you-dark.conf` and `material-you-light.conf` into `<config_dir>/kitty/themes/`,
/// manage the `<config_dir>/kitty/current-theme.conf` symlink (locked to dark theme), and
/// reload running Kitty instances via `pkill -USR1 -x kitty`.
pub fn write_kitty_themes(
    scheme: &MaterialYouScheme,
    paths: &ThemePaths,
    run_external_commands: bool,
) -> Result<KittyArtifacts> {
    let themes_dir = paths.kitty_themes_dir();
    fs::create_dir_all(&themes_dir).with_context(|| {
        format!(
            "failed to create kitty themes directory at {}",
            themes_dir.display()
        )
    })?;

    let dark_theme_path = themes_dir.join("material-you-dark.conf");
    let light_theme_path = themes_dir.join("material-you-light.conf");

    let dark_content = generate_kitty_theme(&scheme.pywal.dark, "Material You Dark");
    let light_content = generate_kitty_theme(&scheme.pywal.light, "Material You Light");

    fs::write(&dark_theme_path, dark_content).with_context(|| {
        format!(
            "failed to write kitty dark theme at {}",
            dark_theme_path.display()
        )
    })?;
    fs::write(&light_theme_path, light_content).with_context(|| {
        format!(
            "failed to write kitty light theme at {}",
            light_theme_path.display()
        )
    })?;

    let current_theme_symlink = paths.kitty_current_theme_path();
    if fs::symlink_metadata(&current_theme_symlink).is_ok() {
        let _ = fs::remove_file(&current_theme_symlink);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        // Point to "themes/material-you-dark.conf" relative to ~/.config/kitty/
        let _ = symlink(Path::new("themes/material-you-dark.conf"), &current_theme_symlink);
    }

    let mut reloaded = false;
    if run_external_commands {
        let res = Command::new("pkill")
            .arg("-USR1")
            .arg("-x")
            .arg("kitty")
            .output();
        if let Ok(out) = res {
            reloaded = out.status.success();
        }
    }

    Ok(KittyArtifacts {
        dark_theme_path,
        light_theme_path,
        current_theme_symlink,
        reloaded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheme::{SchemeRequest, SchemeVariant};
    use tempfile::tempdir;

    fn sample_scheme() -> MaterialYouScheme {
        let req = SchemeRequest {
            source_color_hex: Some("#4285F4".to_string()),
            variant: SchemeVariant::TonalSpot,
            ..Default::default()
        };
        MaterialYouScheme::generate(&req).unwrap()
    }

    #[test]
    fn test_generate_kitty_theme_structure_and_headers() {
        let scheme = sample_scheme();
        let dark_conf = generate_kitty_theme(&scheme.pywal.dark, "Material You Dark");

        assert!(dark_conf.starts_with("## name: Material You Dark\n"));
        assert!(dark_conf.contains("## author: kde-material-you-colors\n"));
        assert!(dark_conf.contains("## upstream: https://github.com/luisbocanegra/kde-material-you-colors\n"));
        assert!(dark_conf.contains("color0  "));
        assert!(dark_conf.contains("color15 "));
        assert!(dark_conf.contains("background            "));
        assert!(dark_conf.contains("foreground            "));
        assert!(dark_conf.contains("cursor                "));
        assert!(dark_conf.contains("cursor_text_color     "));
        assert!(dark_conf.contains("selection_foreground  "));
        assert!(dark_conf.contains("selection_background  "));
        assert!(dark_conf.contains("active_border_color   "));
        assert!(dark_conf.contains("inactive_border_color "));
        assert!(dark_conf.contains("url_color             "));
        assert!(dark_conf.contains("active_tab_foreground   "));
        assert!(dark_conf.contains("active_tab_background   "));
        assert!(dark_conf.contains("cursor_trail_color      "));

        let light_conf = generate_kitty_theme(&scheme.pywal.light, "Material You Light");
        assert!(light_conf.starts_with("## name: Material You Light\n"));
    }

    #[test]
    fn test_write_kitty_themes_creates_files_and_symlink_isolated() {
        let tmp = tempdir().unwrap();
        let paths = ThemePaths::in_root(tmp.path());
        let scheme = sample_scheme();

        let artifacts = write_kitty_themes(&scheme, &paths, false).unwrap();

        assert!(artifacts.dark_theme_path.exists());
        assert!(artifacts.light_theme_path.exists());
        assert_eq!(
            artifacts.dark_theme_path,
            paths.kitty_themes_dir().join("material-you-dark.conf")
        );
        assert_eq!(
            artifacts.light_theme_path,
            paths.kitty_themes_dir().join("material-you-light.conf")
        );

        let dark_content = fs::read_to_string(&artifacts.dark_theme_path).unwrap();
        assert!(dark_content.contains("## name: Material You Dark"));

        let light_content = fs::read_to_string(&artifacts.light_theme_path).unwrap();
        assert!(light_content.contains("## name: Material You Light"));

        #[cfg(unix)]
        {
            let symlink_path = paths.kitty_current_theme_path();
            assert!(fs::symlink_metadata(&symlink_path).is_ok());
            let target = fs::read_link(&symlink_path).unwrap();
            assert_eq!(target, Path::new("themes/material-you-dark.conf"));
        }
    }

    #[test]
    fn test_write_kitty_themes_replaces_existing_symlink_cleanly() {
        let tmp = tempdir().unwrap();
        let paths = ThemePaths::in_root(tmp.path());
        let scheme = sample_scheme();

        // Create dummy pre-existing current-theme.conf file
        fs::create_dir_all(paths.kitty_dir()).unwrap();
        let symlink_path = paths.kitty_current_theme_path();
        fs::write(&symlink_path, "include themes/old-theme.conf").unwrap();
        assert!(symlink_path.exists());

        let artifacts = write_kitty_themes(&scheme, &paths, false).unwrap();

        #[cfg(unix)]
        {
            assert!(fs::symlink_metadata(&artifacts.current_theme_symlink).is_ok());
            let target = fs::read_link(&artifacts.current_theme_symlink).unwrap();
            assert_eq!(target, Path::new("themes/material-you-dark.conf"));
        }
    }
}
