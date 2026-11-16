use clap::{ArgAction, Parser};

/// Automatic Material You Colors Generator from your wallpaper for the Plasma Desktop and more.
/// Any argument passed here overrides its counterpart in the configuration file (if any).
#[derive(Parser, Debug, Clone, Default, PartialEq)]
#[command(
    name = "kde-material-you-colors",
    disable_version_flag = true,
    about = "Automatic Material You Colors Generator from your wallpaper for the Plasma Desktop and more. Any argument passed here overrides their counterpart in the configuration file (if any).",
    after_help = "For more information, issues, feature requests and updates check the project page https://github.com/luisbocanegra/kde-material-you-colors"
)]
pub struct CliArgs {
    /// Monitor to get wallpaper (default is 0)
    #[arg(short = 'm', long = "monitor", value_name = "<integer>")]
    pub monitor: Option<u32>,

    /// Custom color (hex or rgb) used to generate M3 color scheme.
    /// Takes precedence over automatic wallpaper detection.
    #[arg(
        long = "color",
        visible_alias = "col",
        short_alias = 'C',
        value_name = "<color>"
    )]
    pub color: Option<String>,

    /// Image or plain text file containing the absolute path of an image
    /// (Takes precedence over automatic wallpaper detection and --color options)
    #[arg(short = 'f', long = "file", value_name = "<filename>")]
    pub file: Option<String>,

    /// Alternative color mode (default is 0), some images return more than one color,
    /// this will use either the matched or last one
    #[arg(short = 'n', long = "ncolor", value_name = "<integer>")]
    pub ncolor: Option<u32>,

    /// List of 7 space separated colors (hex or rgb) to be used for text in
    /// pywal/konsole/KSyntaxHighlighting instead of wallpaper ones
    #[arg(
        long = "custom-colors-list",
        visible_alias = "ccl",
        value_name = "<colors>"
    )]
    pub custom_colors_list: Option<String>,

    /// Enable Light mode
    #[arg(short = 'l', long = "light", action = ArgAction::SetTrue)]
    pub light: bool,

    /// Enable Dark mode
    #[arg(short = 'd', long = "dark", action = ArgAction::SetTrue)]
    pub dark: bool,

    /// Enable (copy) the startup script to automatically start with KDE
    #[arg(short = 'a', long = "autostart", action = ArgAction::SetTrue)]
    pub autostart: bool,

    /// Copies the default config to ~/.config/kde-material-you-colors/config.conf
    #[arg(short = 'c', long = "copyconfig", action = ArgAction::SetTrue)]
    pub copyconfig: bool,

    /// Copies desktop entries to ~/.local/share/applications/
    #[arg(
        long = "copylauncher",
        visible_alias = "cl",
        action = ArgAction::SetTrue
    )]
    pub copylauncher: bool,

    /// Icons theme for Light scheme
    #[arg(
        long = "iconslight",
        visible_alias = "icons-light",
        value_name = "<icon-theme-name>"
    )]
    pub iconslight: Option<String>,

    /// Icons theme for Dark scheme
    #[arg(
        long = "iconsdark",
        visible_alias = "icons-dark",
        value_name = "<icon-theme-name>"
    )]
    pub iconsdark: Option<String>,

    /// Use pywal to theme other apps with Material You
    #[arg(long = "pywal", visible_alias = "wal", action = ArgAction::SetTrue)]
    pub pywal: bool,

    /// Export Kitty terminal theme files and manage current-theme.conf symlink
    #[arg(long = "kitty", action = ArgAction::SetTrue)]
    pub kitty: bool,

    /// Use Light mode for pywal controlled apps
    #[arg(
        long = "pywallight",
        visible_alias = "pywal-light",
        visible_alias = "wall",
        action = ArgAction::SetTrue
    )]
    pub pywal_light: bool,

    /// Use Dark mode for pywal controlled apps
    #[arg(
        long = "pywaldark",
        visible_alias = "pywal-dark",
        visible_alias = "wald",
        action = ArgAction::SetTrue
    )]
    pub pywal_dark: bool,

    /// The amount of color for backgrounds in Light mode (value from 0 to 4.0, default is 1)
    #[arg(
        long = "lbmultiplier",
        visible_alias = "light-blend-multiplier",
        visible_alias = "lbm",
        value_name = "<float>"
    )]
    pub light_blend_multiplier: Option<f64>,

    /// The amount of color for backgrounds in Dark mode (value from 0 to 4.0, default is 1)
    #[arg(
        long = "dbmultiplier",
        visible_alias = "dark-blend-multiplier",
        visible_alias = "dbm",
        value_name = "<float>"
    )]
    pub dark_blend_multiplier: Option<f64>,

    /// A script/command that will be executed on start or wallpaper/dark/light/settings change
    #[arg(long = "on-change-hook", value_name = "<script or command>")]
    pub on_change_hook: Option<String>,

    /// Tint Sierra Breeze decoration buttons
    #[arg(
        long = "sierra-breeze-buttons-color",
        visible_alias = "sbb",
        action = ArgAction::SetTrue
    )]
    pub sierra_breeze_buttons_color: bool,

    /// Disable Konsole automatic theming (color schemes are still generated)
    #[arg(
        long = "disable-konsole",
        visible_alias = "dk",
        action = ArgAction::SetTrue
    )]
    pub disable_konsole: bool,

    /// Override opacity values for titlebar
    #[arg(
        long = "titlebar-opacity-override",
        visible_alias = "override-titlebar-opacity",
        action = ArgAction::SetTrue
    )]
    pub titlebar_opacity_override: bool,

    /// Titlebar opacity in normal mode (value from 0 to 100, default is 100)
    #[arg(
        long = "titlebar-opacity",
        visible_alias = "tio",
        value_name = "<integer>"
    )]
    pub titlebar_opacity: Option<u32>,

    /// Titlebar opacity in dark mode (value from 0 to 100, default is 100)
    #[arg(
        long = "titlebar-opacity-dark",
        visible_alias = "tiod",
        value_name = "<integer>"
    )]
    pub titlebar_opacity_dark: Option<u32>,

    /// Override opacity values for toolbar
    #[arg(
        long = "toolbar-opacity-override",
        visible_alias = "override-toolbar-opacity",
        action = ArgAction::SetTrue
    )]
    pub toolbar_opacity_override: bool,

    /// ToolBar opacity in normal mode (value from 0 to 100, default is 100)
    #[arg(
        long = "toolbar-opacity",
        visible_alias = "too",
        value_name = "<integer>"
    )]
    pub toolbar_opacity: Option<u32>,

    /// ToolBar opacity in dark mode (value from 0 to 100, default is 100)
    #[arg(
        long = "toolbar-opacity-dark",
        visible_alias = "tood",
        value_name = "<integer>"
    )]
    pub toolbar_opacity_dark: Option<u32>,

    /// Konsole background opacity in normal mode (value from 0 to 100, default is 100)
    #[arg(
        long = "konsole-opacity",
        visible_alias = "ko",
        value_name = "<integer>"
    )]
    pub konsole_opacity: Option<u32>,

    /// Konsole background opacity in dark mode (value from 0 to 100, default is 100)
    #[arg(
        long = "konsole-opacity-dark",
        visible_alias = "kod",
        value_name = "<integer>"
    )]
    pub konsole_opacity_dark: Option<u32>,

    /// Apply blur to Konsole background
    #[arg(long = "konsole-blur", action = ArgAction::SetTrue)]
    pub konsole_blur: bool,

    /// Kill an existing instance of kde-material-you-colors and exit
    #[arg(long = "stop", action = ArgAction::SetTrue)]
    pub stop: bool,

    /// Tint Klassy Window Decoration window outline
    #[arg(
        long = "klassy-windeco-outline",
        visible_alias = "kwo",
        action = ArgAction::SetTrue
    )]
    pub klassy_windeco_outline: bool,

    /// List of space separated window class names to apply a darker titlebar to
    #[arg(
        long = "darker-window-list",
        visible_alias = "dwl",
        value_name = "<names>"
    )]
    pub darker_window_list: Option<String>,

    /// Enable the startup delay, disabled by default, requires --startup-delay option
    #[arg(long = "use-startup-delay", action = ArgAction::SetTrue)]
    pub use_startup_delay: bool,

    /// Add a startup delay (in seconds) before doing anything (default is 0)
    #[arg(
        long = "startup-delay",
        visible_alias = "sd",
        value_name = "<integer>"
    )]
    pub startup_delay: Option<u32>,

    /// Main loop delay (in seconds, default is 1)
    #[arg(
        long = "main-loop-delay",
        visible_alias = "mld",
        value_name = "<float>"
    )]
    pub main_loop_delay: Option<f64>,

    /// Delay after taking screenshot (in seconds, default is 900)
    #[arg(
        long = "screenshot-delay",
        visible_alias = "scd",
        value_name = "<float>"
    )]
    pub screenshot_delay: Option<f64>,

    /// Apply colors from screenshot only after wallpaper plugin changes
    #[arg(
        long = "once-after-change",
        visible_alias = "ofc",
        action = ArgAction::SetTrue
    )]
    pub once_after_change: bool,

    /// Pause mode: disables wallpaper detection and automatic theming
    #[arg(long = "pause-mode", action = ArgAction::SetTrue)]
    pub pause_mode: bool,

    /// Only use screenshot method to get the current wallpaper
    #[arg(long = "screenshot-only-mode", action = ArgAction::SetTrue)]
    pub screenshot_only_mode: bool,

    /// Print version information
    #[arg(short = 'v', long = "version", action = ArgAction::SetTrue)]
    pub version: bool,

    /// Changes between Material You scheme variants (0..=8, default is 5 = TonalSpot)
    #[arg(
        long = "scheme-variant",
        visible_alias = "sv",
        value_name = "<integer>"
    )]
    pub scheme_variant: Option<u8>,

    /// Colorfulness of the theme (value from 0.5 to 10, default is 1)
    #[arg(long = "chroma-multiplier", value_name = "<float>")]
    pub chroma_multiplier: Option<f64>,

    /// Brightness of the theme (value from 0 to 1.5, default is 1)
    #[arg(long = "tone-multiplier", value_name = "<float>")]
    pub tone_multiplier: Option<f64>,

    /// (Deprecated) Name or location of the QDbus executable
    #[arg(long = "qdbus-executable", value_name = "<string>")]
    pub qdbus_executable: Option<String>,

    /// Disables automatic color fetching
    #[arg(long = "manual-fetch", action = ArgAction::SetTrue)]
    pub manual_fetch: bool,

    /// Trigger a manual color fetch
    #[arg(long = "fetch-colors", action = ArgAction::SetTrue)]
    pub fetch_colors: bool,

    /// Frames and outlines contrast (value from 0 to 1, default is 0.2)
    #[arg(long = "frame-contrast", value_name = "<float>")]
    pub frame_contrast: Option<f64>,

    /// General contrast level (value from -1 to 1, default is 0)
    #[arg(long = "contrast-level", value_name = "<float>")]
    pub contrast_level: Option<f64>,

    /// The version of the material color specification to use (2021, 2025)
    #[arg(long = "spec-version", value_name = "<version>")]
    pub spec_version: Option<String>,

    /// Tint KDE Rounded Corners desktop effect window outline
    #[arg(long = "kde-rounded-corners-effect-outline", action = ArgAction::SetTrue)]
    pub kde_rounded_corners_effect_outline: bool,

    /// Launch the configuration and live palette preview GUI window
    #[arg(long = "gui", action = ArgAction::SetTrue)]
    pub gui: bool,
}

impl CliArgs {
    /// Normalize legacy Python multi-char short flags (such as `-col`, `-ccl`, `-sv`, etc.)
    /// into standard `--<flag>` arguments before invoking `clap::Parser::try_parse_from`.
    pub fn parse_normalized<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let normalized: Vec<std::ffi::OsString> = itr
            .into_iter()
            .map(|item| {
                let os: std::ffi::OsString = item.into();
                if let Some(s) = os.to_str() {
                    match s {
                        "-col"
                        | "-ccl"
                        | "-cl"
                        | "-wal"
                        | "-wall"
                        | "-wald"
                        | "-lbm"
                        | "-dbm"
                        | "-sbb"
                        | "-dk"
                        | "-tio"
                        | "-tiod"
                        | "-too"
                        | "-tood"
                        | "-ko"
                        | "-kod"
                        | "-kwo"
                        | "-dwl"
                        | "-sd"
                        | "-mld"
                        | "-scd"
                        | "-ofc"
                        | "-sv" => std::ffi::OsString::from(format!("-{}", s)),
                        _ => os,
                    }
                } else {
                    os
                }
            })
            .collect();
        Self::try_parse_from(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_cli_args() {
        let args = CliArgs::parse_from(["kde-material-you-colors"]);
        assert_eq!(args, CliArgs::default());
        assert!(!args.dark);
        assert!(!args.light);
        assert!(!args.gui);
        assert!(!args.stop);
        assert!(!args.version);
        assert_eq!(args.monitor, None);
        assert_eq!(args.scheme_variant, None);
    }

    #[test]
    fn test_parse_all_major_cli_flags() {
        let args = CliArgs::parse_from([
            "kde-material-you-colors",
            "--monitor",
            "2",
            "--file",
            "/tmp/wall.png",
            "--color",
            "#ff0055",
            "--dark",
            "--autostart",
            "--copyconfig",
            "--copylauncher",
            "--stop",
            "-v",
            "--scheme-variant",
            "6",
            "--chroma-multiplier",
            "2.5",
            "--tone-multiplier",
            "1.2",
            "--contrast-level",
            "0.5",
            "--frame-contrast",
            "0.35",
            "--spec-version",
            "2025",
            "--custom-colors-list",
            "#111111 #222222 #333333 #444444 #555555 #666666 #777777",
            "--ncolor",
            "1",
            "--pywal",
            "--kitty",
            "--pywal-light",
            "--light-blend-multiplier",
            "1.5",
            "--dark-blend-multiplier",
            "2.0",
            "--on-change-hook",
            "/usr/local/bin/hook.sh",
            "--sierra-breeze-buttons-color",
            "--disable-konsole",
            "--titlebar-opacity-override",
            "--titlebar-opacity",
            "85",
            "--titlebar-opacity-dark",
            "80",
            "--toolbar-opacity-override",
            "--toolbar-opacity",
            "90",
            "--toolbar-opacity-dark",
            "75",
            "--konsole-opacity",
            "88",
            "--konsole-opacity-dark",
            "82",
            "--konsole-blur",
            "--darker-window-list",
            "konsole kitty",
            "--iconslight",
            "Papirus-Light",
            "--iconsdark",
            "Papirus-Dark",
            "--pause-mode",
            "--screenshot-only-mode",
            "--once-after-change",
            "--manual-fetch",
            "--fetch-colors",
            "--use-startup-delay",
            "--startup-delay",
            "5",
            "--main-loop-delay",
            "2.5",
            "--screenshot-delay",
            "600",
            "--klassy-windeco-outline",
            "--kde-rounded-corners-effect-outline",
            "--gui",
        ]);

        assert_eq!(args.monitor, Some(2));
        assert_eq!(args.file.as_deref(), Some("/tmp/wall.png"));
        assert_eq!(args.color.as_deref(), Some("#ff0055"));
        assert!(args.dark);
        assert!(!args.light);
        assert!(args.autostart);
        assert!(args.copyconfig);
        assert!(args.copylauncher);
        assert!(args.stop);
        assert!(args.version);
        assert_eq!(args.scheme_variant, Some(6));
        assert_eq!(args.chroma_multiplier, Some(2.5));
        assert_eq!(args.tone_multiplier, Some(1.2));
        assert_eq!(args.contrast_level, Some(0.5));
        assert_eq!(args.frame_contrast, Some(0.35));
        assert_eq!(args.spec_version.as_deref(), Some("2025"));
        assert_eq!(
            args.custom_colors_list.as_deref(),
            Some("#111111 #222222 #333333 #444444 #555555 #666666 #777777")
        );
        assert_eq!(args.ncolor, Some(1));
        assert!(args.pywal);
        assert!(args.kitty);
        assert!(args.pywal_light);
        assert_eq!(args.light_blend_multiplier, Some(1.5));
        assert_eq!(args.dark_blend_multiplier, Some(2.0));
        assert_eq!(args.on_change_hook.as_deref(), Some("/usr/local/bin/hook.sh"));
        assert!(args.sierra_breeze_buttons_color);
        assert!(args.disable_konsole);
        assert!(args.titlebar_opacity_override);
        assert_eq!(args.titlebar_opacity, Some(85));
        assert_eq!(args.titlebar_opacity_dark, Some(80));
        assert!(args.toolbar_opacity_override);
        assert_eq!(args.toolbar_opacity, Some(90));
        assert_eq!(args.toolbar_opacity_dark, Some(75));
        assert_eq!(args.konsole_opacity, Some(88));
        assert_eq!(args.konsole_opacity_dark, Some(82));
        assert!(args.konsole_blur);
        assert_eq!(args.darker_window_list.as_deref(), Some("konsole kitty"));
        assert_eq!(args.iconslight.as_deref(), Some("Papirus-Light"));
        assert_eq!(args.iconsdark.as_deref(), Some("Papirus-Dark"));
        assert!(args.pause_mode);
        assert!(args.screenshot_only_mode);
        assert!(args.once_after_change);
        assert!(args.manual_fetch);
        assert!(args.fetch_colors);
        assert!(args.use_startup_delay);
        assert_eq!(args.startup_delay, Some(5));
        assert_eq!(args.main_loop_delay, Some(2.5));
        assert_eq!(args.screenshot_delay, Some(600.0));
        assert!(args.klassy_windeco_outline);
        assert!(args.kde_rounded_corners_effect_outline);
        assert!(args.gui);
    }

    #[test]
    fn test_parse_legacy_python_aliases() {
        let args = CliArgs::parse_normalized([
            "kde-material-you-colors",
            "-m",
            "1",
            "-col",
            "#123456",
            "-f",
            "/path/img.jpg",
            "-n",
            "2",
            "-l",
            "-wal",
            "-wall",
            "-lbm",
            "0.8",
            "-dbm",
            "1.2",
            "-sbb",
            "-dk",
            "-tio",
            "70",
            "-tiod",
            "65",
            "-too",
            "75",
            "-tood",
            "60",
            "-ko",
            "95",
            "-kod",
            "90",
            "-kwo",
            "-dwl",
            "alacritty",
            "-sd",
            "3",
            "-mld",
            "1.5",
            "-scd",
            "300",
            "-ofc",
            "-sv",
            "4",
        ])
        .expect("Failed to parse legacy short aliases");

        assert_eq!(args.monitor, Some(1));
        assert_eq!(args.color.as_deref(), Some("#123456"));
        assert_eq!(args.file.as_deref(), Some("/path/img.jpg"));
        assert_eq!(args.ncolor, Some(2));
        assert!(args.light);
        assert!(args.pywal);
        assert!(args.pywal_light);
        assert_eq!(args.light_blend_multiplier, Some(0.8));
        assert_eq!(args.dark_blend_multiplier, Some(1.2));
        assert!(args.sierra_breeze_buttons_color);
        assert!(args.disable_konsole);
        assert_eq!(args.titlebar_opacity, Some(70));
        assert_eq!(args.titlebar_opacity_dark, Some(65));
        assert_eq!(args.toolbar_opacity, Some(75));
        assert_eq!(args.toolbar_opacity_dark, Some(60));
        assert_eq!(args.konsole_opacity, Some(95));
        assert_eq!(args.konsole_opacity_dark, Some(90));
        assert!(args.klassy_windeco_outline);
        assert_eq!(args.darker_window_list.as_deref(), Some("alacritty"));
        assert_eq!(args.startup_delay, Some(3));
        assert_eq!(args.main_loop_delay, Some(1.5));
        assert_eq!(args.screenshot_delay, Some(300.0));
        assert!(args.once_after_change);
        assert_eq!(args.scheme_variant, Some(4));
    }
}
