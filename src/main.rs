use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use kde_material_you_colors::apply::{apply_all_themes, ThemePaths};
use kde_material_you_colors::cli::CliArgs;
use kde_material_you_colors::config::AppConfig;
use kde_material_you_colors::dbus_client::{
    self, InstanceAction,
};
use kde_material_you_colors::gui;
use kde_material_you_colors::scheme::{MaterialYouScheme, SchemeRequest, SchemeVariant};
use kde_material_you_colors::tray::{self, ThemeModeSelection, TrayCommand};
use kde_material_you_colors::wallpaper::{
    self, ConfigAndWallpaperWatcher, WallpaperSource, WatchEvent,
};

const AUTOSTART_DESKTOP_ENTRY: &str = "[Desktop Entry]
Type=Application
Name=KDE Material You Colors
Comment=Automatic Material You Colors Generator from your wallpaper for the Plasma Desktop
Exec=kde-material-you-colors
Icon=preferences-desktop-color
Terminal=false
Categories=Qt;KDE;Settings;
X-KDE-autostart-after=panel
";

const LAUNCHER_DESKTOP_ENTRY: &str = "[Desktop Entry]
Type=Application
Name=KDE Material You Colors Settings
Comment=Configure Material You dynamic colors for KDE Plasma
Exec=kde-material-you-colors --gui
Icon=preferences-desktop-color
Terminal=false
Categories=Qt;KDE;Settings;DesktopSettings;
";

/// Resolve whether Light mode is active by checking explicit config/CLI `light` first,
/// and falling back to `~/.config/kdeglobals` (`MaterialYouLight` vs `MaterialYouDark` or SHA-1 hash).
pub fn resolve_effective_light_mode(config: &AppConfig, paths: &ThemePaths) -> bool {
    if let Some(explicit_light) = config.light {
        return explicit_light;
    }
    let light_scheme = paths.color_schemes_dir().join("MaterialYouLight.colors");
    let dark_scheme = paths.color_schemes_dir().join("MaterialYouDark.colors");
    wallpaper::read_kdeglobals_light_mode(&paths.kdeglobals_path(), &light_scheme, &dark_scheme)
        .unwrap_or(false)
}

/// Construct a `SchemeRequest` from the current `AppConfig`, resolved `WallpaperSource`, and effective light mode.
pub fn build_scheme_request(
    config: &AppConfig,
    wallpaper: &WallpaperSource,
    effective_light: bool,
) -> SchemeRequest {
    let (source_color_hex, image_path) = match wallpaper {
        WallpaperSource::Color(hex) => (Some(hex.clone()), None),
        WallpaperSource::Image(path) => (None, Some(path.clone())),
    };

    let pywal_light = config.pywal_light.unwrap_or(effective_light);
    let custom_colors_list = config.custom_colors_list.clone().unwrap_or_default();

    SchemeRequest {
        source_color_hex,
        image_path,
        ncolor: config.ncolor as usize,
        variant: SchemeVariant::from_u8(config.scheme_variant),
        chroma_multiplier: config.chroma_multiplier,
        tone_multiplier: config.tone_multiplier,
        contrast_level: config.contrast_level,
        frame_contrast: config.frame_contrast,
        light: effective_light,
        pywal_light,
        light_saturation_multiplier: config.light_blend_multiplier,
        dark_saturation_multiplier: config.dark_blend_multiplier,
        light_brightness_multiplier: 1.0,
        dark_brightness_multiplier: 1.0,
        custom_colors_list,
    }
}

fn write_desktop_file(target_path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    std::fs::write(target_path, content)
        .with_context(|| format!("Failed to write desktop file {}", target_path.display()))
}

async fn execute_theme_cycle(
    config: &AppConfig,
    paths: &ThemePaths,
    appletsrc_path: &Path,
    potd_cache_dir: &Path,
) -> Result<MaterialYouScheme> {
    let effective_light = resolve_effective_light_mode(config, paths);
    let mut effective_config = config.clone();
    effective_config.light = Some(effective_light);
    if effective_config.pywal_light.is_none() {
        effective_config.pywal_light = Some(effective_light);
    }

    let wallpaper_source =
        wallpaper::resolve_wallpaper_async(&effective_config, appletsrc_path, potd_cache_dir)
            .await?;

    let req = build_scheme_request(&effective_config, &wallpaper_source, effective_light);
    let mut scheme = MaterialYouScheme::generate(&req)?;
    scheme.wallpaper = wallpaper_source.to_wallpaper_info();

    let _ = dbus_client::start_kwin_blend_changes().await;
    let applied = apply_all_themes(&scheme, &effective_config, paths, true)?;

    if applied.needs_kwin_reload {
        let _ = dbus_client::reconfigure_kwin().await;
    }
    if effective_config.klassy_windeco_outline {
        let _ = dbus_client::notify_klassy_decoration().await;
    }
    if effective_config.kde_rounded_corners_effect_outline {
        let _ = dbus_client::reload_kde_rounded_corners().await;
    }
    if !effective_config.disable_konsole {
        if let Some(ref prof) = applied.konsole.active_profile_name {
            let _ = dbus_client::reload_konsole_sessions(prof).await;
        }
    }

    Ok(scheme)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = CliArgs::parse_normalized(std::env::args_os()).unwrap_or_else(|e| e.exit());

    if cli.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let paths = ThemePaths::default();
    let config_path = AppConfig::default_path()
        .unwrap_or_else(|| paths.config_dir.join("kde-material-you-colors/config.conf"));

    if cli.copyconfig {
        let cfg = AppConfig::load_from_path(&config_path).unwrap_or_default();
        cfg.save_to_path(&config_path)?;
        tracing::info!("Saved configuration to {}", config_path.display());
        return Ok(());
    }

    if cli.autostart {
        let autostart_path = paths
            .config_dir
            .join("autostart/kde-material-you-colors.desktop");
        write_desktop_file(&autostart_path, AUTOSTART_DESKTOP_ENTRY)?;
        tracing::info!("Installed autostart entry at {}", autostart_path.display());
        return Ok(());
    }

    if cli.copylauncher {
        let launcher_path = paths
            .data_dir
            .join("applications/kde-material-you-colors.desktop");
        write_desktop_file(&launcher_path, LAUNCHER_DESKTOP_ENTRY)?;
        tracing::info!("Installed application launcher at {}", launcher_path.display());
        return Ok(());
    }

    if cli.gui {
        return gui::run_settings_window(config_path, paths.runtime_json_path());
    }

    let pid_file = dbus_client::default_pid_file_path();
    let _instance_guard = match dbus_client::signal_existing_or_acquire(&pid_file, cli.stop)? {
        InstanceAction::StoppedExisting(Some(pid)) => {
            tracing::info!("Stopped running kde-material-you-colors daemon (PID {pid})");
            return Ok(());
        }
        InstanceAction::StoppedExisting(None) => {
            tracing::info!("No running kde-material-you-colors daemon found");
            return Ok(());
        }
        InstanceAction::SignaledExisting(pid) => {
            tracing::info!(
                "Signaled existing kde-material-you-colors daemon (PID {pid}) to reload"
            );
            return Ok(());
        }
        InstanceAction::Acquired(guard) => guard,
    };

    if !config_path.exists() {
        let _ = AppConfig::default().save_to_path(&config_path);
    }

    let mut config = AppConfig::load_from_path(&config_path)
        .unwrap_or_default()
        .with_cli_overrides(&cli);

    if config.use_startup_delay && config.startup_delay > 0 {
        tokio::time::sleep(Duration::from_secs(config.startup_delay as u64)).await;
    }

    let appletsrc_path = wallpaper::default_appletsrc_path()
        .unwrap_or_else(|| paths.config_dir.join("plasma-org.kde.plasma.desktop-appletsrc"));
    let potd_cache_dir = wallpaper::default_potd_cache_dir()
        .unwrap_or_else(|| paths.cache_dir.join("plasma_engine_potd"));

    let mut current_seed = "#4285f4".to_string();
    if !config.pause_mode {
        match execute_theme_cycle(&config, &paths, &appletsrc_path, &potd_cache_dir).await {
            Ok(scheme) => {
                current_seed = scheme.seed.color.clone();
                tracing::info!(
                    "Applied Material You theme (seed={}, variant={:?})",
                    current_seed,
                    SchemeVariant::from_u8(config.scheme_variant)
                );
            }
            Err(err) => {
                tracing::warn!("Initial theme cycle skipped: {err}");
            }
        }
    }

    let (tray_handle, tray_rx) = tray::spawn_tray(config.clone(), current_seed.clone());
    let watcher = ConfigAndWallpaperWatcher::new(
        config_path.clone(),
        appletsrc_path.clone(),
        paths.kdeglobals_path(),
        _instance_guard.reload_file().to_path_buf(),
    )?;

    let mut last_config_sha1 = AppConfig::sha1_file(&config_path);
    let mut last_effective_light = resolve_effective_light_mode(&config, &paths);
    let mut last_wallpaper =
        wallpaper::resolve_wallpaper_async(&config, &appletsrc_path, &potd_cache_dir)
            .await
            .ok();

    loop {
        let mut needs_apply = false;
        let mut force_apply = false;

        while let Ok(cmd) = tray_rx.try_recv() {
            match cmd {
                TrayCommand::OpenSettingsWindow => {
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = Command::new(exe).arg("--gui").spawn();
                    }
                }
                TrayCommand::FetchColorsNow => {
                    force_apply = true;
                    needs_apply = true;
                }
                TrayCommand::TogglePause(paused) => {
                    config.pause_mode = paused;
                    let _ = config.save_to_path(&config_path);
                    last_config_sha1 = AppConfig::sha1_file(&config_path);
                    if !paused {
                        force_apply = true;
                        needs_apply = true;
                    }
                }
                TrayCommand::SetThemeMode(mode) => {
                    match mode {
                        ThemeModeSelection::Dark => {
                            config.plasma_follows_scheme = false;
                            config.light = Some(false);
                        }
                        ThemeModeSelection::Light => {
                            config.plasma_follows_scheme = false;
                            config.light = Some(true);
                        }
                        ThemeModeSelection::FollowScheme => {
                            config.plasma_follows_scheme = true;
                            config.light = None;
                        }
                    }
                    let _ = config.save_to_path(&config_path);
                    last_config_sha1 = AppConfig::sha1_file(&config_path);
                    force_apply = true;
                    needs_apply = true;
                }
                TrayCommand::SetSchemeVariant(v) => {
                    config.scheme_variant = v.min(8);
                    let _ = config.save_to_path(&config_path);
                    last_config_sha1 = AppConfig::sha1_file(&config_path);
                    force_apply = true;
                    needs_apply = true;
                }
                TrayCommand::Quit => {
                    tracing::info!("Shutting down kde-material-you-colors daemon");
                    return Ok(());
                }
            }
        }

        if let Some(first_event) = watcher.recv_timeout(Duration::from_millis(250)) {
            let mut events = vec![first_event];
            tokio::time::sleep(Duration::from_millis(80)).await;
            while let Some(ev) = watcher.try_recv() {
                events.push(ev);
            }

            for ev in events {
                match ev {
                    WatchEvent::ConfigChanged(_) => {
                        let new_sha1 = AppConfig::sha1_file(&config_path);
                        if new_sha1 != last_config_sha1 {
                            last_config_sha1 = new_sha1;
                            if let Ok(loaded) = AppConfig::load_from_path(&config_path) {
                                let fetch_requested = loaded.fetch_colors;
                                config = loaded.with_cli_overrides(&cli);
                                if fetch_requested {
                                    force_apply = true;
                                }
                                needs_apply = true;
                            }
                        }
                    }
                    WatchEvent::WallpaperChanged(_) => {
                        if !config.manual_fetch {
                            if let Ok(new_wp) = wallpaper::resolve_wallpaper_async(
                                &config,
                                &appletsrc_path,
                                &potd_cache_dir,
                            )
                            .await
                            {
                                if Some(&new_wp) != last_wallpaper.as_ref() {
                                    last_wallpaper = Some(new_wp);
                                    needs_apply = true;
                                }
                            }
                        }
                    }
                    WatchEvent::KdeGlobalsChanged(_) => {
                        let new_light = resolve_effective_light_mode(&config, &paths);
                        if new_light != last_effective_light {
                            last_effective_light = new_light;
                            needs_apply = true;
                        }
                    }
                    WatchEvent::ReloadTriggered(_) => {
                        if let Ok(loaded) = AppConfig::load_from_path(&config_path) {
                            config = loaded.with_cli_overrides(&cli);
                        }
                        force_apply = true;
                        needs_apply = true;
                    }
                }
            }
        }

        if (needs_apply && !config.pause_mode && (!config.manual_fetch || force_apply))
            || force_apply
        {
            match execute_theme_cycle(&config, &paths, &appletsrc_path, &potd_cache_dir).await {
                Ok(scheme) => {
                    current_seed = scheme.seed.color.clone();
                    last_effective_light = resolve_effective_light_mode(&config, &paths);
                    last_wallpaper = wallpaper::resolve_wallpaper_async(
                        &config,
                        &appletsrc_path,
                        &potd_cache_dir,
                    )
                    .await
                    .ok();
                    let cfg_snapshot = config.clone();
                    let seed_snapshot = current_seed.clone();
                    tray_handle.update(|tray: &mut tray::MaterialYouTray| {
                        tray.config = cfg_snapshot;
                        tray.seed_color_hex = seed_snapshot;
                    });
                }
                Err(err) => {
                    tracing::warn!("Theme update cycle error: {err}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_scheme_request_from_color_and_image_sources() {
        let mut cfg = AppConfig::default();
        cfg.scheme_variant = 6; // Vibrant
        cfg.ncolor = 2;
        cfg.chroma_multiplier = 1.4;
        cfg.tone_multiplier = 0.9;
        cfg.custom_colors_list = Some(
            "#ff0000 #00ff00 #0000ff #ffff00 #ff00ff #00ffff #ffffff"
                .split_whitespace()
                .map(String::from)
                .collect(),
        );

        let wp_color = WallpaperSource::Color("#4285f4".to_string());
        let req = build_scheme_request(&cfg, &wp_color, true);
        assert_eq!(req.source_color_hex.as_deref(), Some("#4285f4"));
        assert!(req.image_path.is_none());
        assert_eq!(req.variant, SchemeVariant::Vibrant);
        assert_eq!(req.ncolor, 2);
        assert!(req.light);
        assert!(req.pywal_light);
        assert_eq!(req.custom_colors_list.len(), 7);

        let wp_img = WallpaperSource::Image(PathBuf::from("/tmp/wall.png"));
        let req_img = build_scheme_request(&cfg, &wp_img, false);
        assert!(req_img.source_color_hex.is_none());
        assert_eq!(req_img.image_path, Some(PathBuf::from("/tmp/wall.png")));
        assert!(!req_img.light);
        assert!(!req_img.pywal_light);
    }

    #[test]
    fn test_resolve_effective_light_mode_and_desktop_files() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = ThemePaths::in_root(tmp.path());
        std::fs::create_dir_all(paths.kdeglobals_path().parent().unwrap()).unwrap();
        std::fs::write(
            paths.kdeglobals_path(),
            "[General]\nColorScheme=MaterialYouLight\n",
        )
        .unwrap();

        let mut cfg = AppConfig::default();
        cfg.light = None;
        assert!(resolve_effective_light_mode(&cfg, &paths));

        cfg.light = Some(false);
        assert!(!resolve_effective_light_mode(&cfg, &paths));

        let desktop_path = tmp.path().join("autostart/kde-material-you-colors.desktop");
        write_desktop_file(&desktop_path, AUTOSTART_DESKTOP_ENTRY).unwrap();
        let content = std::fs::read_to_string(&desktop_path).unwrap();
        assert!(content.contains("Exec=kde-material-you-colors"));
    }
}
