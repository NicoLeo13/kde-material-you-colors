use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ksni::menu::{CheckmarkItem, RadioGroup, RadioItem, StandardItem, SubMenu};
use ksni::{Handle, Icon, MenuItem, ToolTip, Tray, TrayService};

use crate::color::hex_to_argb;
use crate::config::AppConfig;
use crate::scheme::SchemeVariant;

/// Theme mode selection exposed in the system tray radio group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeModeSelection {
    Dark,
    Light,
    FollowScheme,
}

impl ThemeModeSelection {
    /// Convert a radio group index (`0..=2`) to a `ThemeModeSelection`.
    pub const fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Dark,
            1 => Self::Light,
            _ => Self::FollowScheme,
        }
    }

    /// Convert a `ThemeModeSelection` to its radio group index (`0..=2`).
    pub const fn to_index(self) -> usize {
        match self {
            Self::Dark => 0,
            Self::Light => 1,
            Self::FollowScheme => 2,
        }
    }
}

/// Commands emitted by the system tray icon and context menu to the daemon loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCommand {
    TogglePause(bool),
    FetchColorsNow,
    SetThemeMode(ThemeModeSelection),
    SetSchemeVariant(u8),
    OpenSettingsWindow,
    Quit,
}

/// Human-readable display names for the 9 Material You scheme variants (`0..=8`).
pub const SCHEME_VARIANT_LABELS: [&str; 9] = [
    "Content",
    "Expressive",
    "Fidelity",
    "Monochrome",
    "Neutral",
    "Tonal Spot",
    "Vibrant",
    "Rainbow",
    "Fruit Salad",
];

/// Return the human-readable label for a `SchemeVariant`.
pub const fn scheme_variant_label(variant: SchemeVariant) -> &'static str {
    match variant {
        SchemeVariant::Content => "Content",
        SchemeVariant::Expressive => "Expressive",
        SchemeVariant::Fidelity => "Fidelity",
        SchemeVariant::Monochrome => "Monochrome",
        SchemeVariant::Neutral => "Neutral",
        SchemeVariant::TonalSpot => "Tonal Spot",
        SchemeVariant::Vibrant => "Vibrant",
        SchemeVariant::Rainbow => "Rainbow",
        SchemeVariant::FruitSalad => "Fruit Salad",
    }
}

/// Generate a crisp 32x32 ARGB32 (network byte order `[A, R, G, B]`) `ksni::Icon`
/// circular badge tinted with `seed_color_hex`. Falls back to `#4285f4` if `seed_color_hex` is invalid.
pub fn generate_badge_pixmap(seed_color_hex: &str) -> Icon {
    const SIZE: i32 = 32;
    let argb = hex_to_argb(seed_color_hex).unwrap_or(0xFF42_85F4);
    let [_, r, g, b] = argb.to_be_bytes();

    // Slightly lighter border ring for contrast on both dark and light panels
    let ring_r = r.saturating_add(40);
    let ring_g = g.saturating_add(40);
    let ring_b = b.saturating_add(40);

    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let center = (SIZE as f32 - 1.0) / 2.0;
    let outer_radius = 13.5_f32;
    let inner_radius = 11.5_f32;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= inner_radius {
                // Solid seed color interior
                data.extend_from_slice(&[255, r, g, b]);
            } else if dist <= outer_radius {
                // Crisp ring border with smooth outer edge anti-aliasing
                let alpha = if dist > outer_radius - 1.0 {
                    ((outer_radius - dist).clamp(0.0, 1.0) * 255.0).round() as u8
                } else {
                    255
                };
                data.extend_from_slice(&[alpha, ring_r, ring_g, ring_b]);
            } else {
                // Transparent background pixel
                data.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    Icon {
        width: SIZE,
        height: SIZE,
        data,
    }
}

/// KDE StatusNotifierItem system tray implementation for KDE Material You Colors.
#[derive(Debug, Clone)]
pub struct MaterialYouTray {
    pub config: AppConfig,
    pub seed_color_hex: String,
    pub command_tx: Sender<TrayCommand>,
}

impl MaterialYouTray {
    /// Construct a new `MaterialYouTray` instance.
    pub fn new(config: AppConfig, seed_color_hex: String, command_tx: Sender<TrayCommand>) -> Self {
        Self {
            config,
            seed_color_hex,
            command_tx,
        }
    }

    /// Determine the currently active `ThemeModeSelection` from `self.config`.
    pub fn current_theme_mode(&self) -> ThemeModeSelection {
        if self.config.plasma_follows_scheme {
            ThemeModeSelection::FollowScheme
        } else {
            match self.config.light {
                Some(true) => ThemeModeSelection::Light,
                _ => ThemeModeSelection::Dark,
            }
        }
    }

    /// Apply a `ThemeModeSelection` update to `self.config` and emit `TrayCommand::SetThemeMode`.
    pub fn select_theme_mode(&mut self, index: usize) {
        let mode = ThemeModeSelection::from_index(index);
        match mode {
            ThemeModeSelection::Dark => {
                self.config.plasma_follows_scheme = false;
                self.config.light = Some(false);
            }
            ThemeModeSelection::Light => {
                self.config.plasma_follows_scheme = false;
                self.config.light = Some(true);
            }
            ThemeModeSelection::FollowScheme => {
                self.config.plasma_follows_scheme = true;
                self.config.light = None;
            }
        }
        let _ = self.command_tx.send(TrayCommand::SetThemeMode(mode));
    }

    /// Apply a `SchemeVariant` index (`0..=8`) to `self.config` and emit `TrayCommand::SetSchemeVariant`.
    pub fn select_scheme_variant(&mut self, index: usize) {
        let clamped = index.min(8) as u8;
        self.config.scheme_variant = clamped;
        let _ = self.command_tx.send(TrayCommand::SetSchemeVariant(clamped));
    }

    /// Toggle `self.config.pause_mode` and emit `TrayCommand::TogglePause`.
    pub fn toggle_pause(&mut self) {
        self.config.pause_mode = !self.config.pause_mode;
        let _ = self
            .command_tx
            .send(TrayCommand::TogglePause(self.config.pause_mode));
    }
}

impl Tray for MaterialYouTray {
    fn id(&self) -> String {
        "kde-material-you-colors".to_string()
    }

    fn title(&self) -> String {
        "KDE Material You Colors".to_string()
    }

    fn icon_name(&self) -> String {
        "preferences-desktop-color".to_string()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![generate_badge_pixmap(&self.seed_color_hex)]
    }

    fn tool_tip(&self) -> ToolTip {
        let variant = SchemeVariant::from_u8(self.config.scheme_variant);
        let variant_str = scheme_variant_label(variant);
        let status_str = if self.config.pause_mode {
            "Paused"
        } else {
            "Active"
        };
        let seed_str = if self.seed_color_hex.is_empty() {
            "#4285f4"
        } else {
            self.seed_color_hex.as_str()
        };

        ToolTip {
            icon_name: "preferences-desktop-color".to_string(),
            icon_pixmap: vec![generate_badge_pixmap(seed_str)],
            title: "Material You Colors".to_string(),
            description: format!(
                "Status: {status_str} | Variant: {variant_str} | Seed: {seed_str}"
            ),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.command_tx.send(TrayCommand::OpenSettingsWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let theme_mode_idx = self.current_theme_mode().to_index();
        let variant_idx = (self.config.scheme_variant.min(8)) as usize;

        let variant_options: Vec<RadioItem> = SCHEME_VARIANT_LABELS
            .iter()
            .map(|label| RadioItem {
                label: (*label).to_string(),
                ..Default::default()
            })
            .collect();

        vec![
            StandardItem {
                label: "Open Settings...".to_string(),
                icon_name: "configure".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.command_tx.send(TrayCommand::OpenSettingsWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Fetch Colors Now".to_string(),
                icon_name: "view-refresh".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.command_tx.send(TrayCommand::FetchColorsNow);
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Pause Automatic Theming".to_string(),
                checked: self.config.pause_mode,
                activate: Box::new(|this: &mut Self| {
                    this.toggle_pause();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            SubMenu {
                label: "Theme Mode".to_string(),
                submenu: vec![RadioGroup {
                    selected: theme_mode_idx,
                    select: Box::new(|this: &mut Self, idx: usize| {
                        this.select_theme_mode(idx);
                    }),
                    options: vec![
                        RadioItem {
                            label: "Dark".to_string(),
                            ..Default::default()
                        },
                        RadioItem {
                            label: "Light".to_string(),
                            ..Default::default()
                        },
                        RadioItem {
                            label: "Follow Color Scheme".to_string(),
                            ..Default::default()
                        },
                    ],
                }
                .into()],
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Scheme Variant".to_string(),
                submenu: vec![RadioGroup {
                    selected: variant_idx,
                    select: Box::new(|this: &mut Self, idx: usize| {
                        this.select_scheme_variant(idx);
                    }),
                    options: variant_options,
                }
                .into()],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                icon_name: "application-exit".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.command_tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Resilient wrapper around `ksni::Handle<MaterialYouTray>` that survives
/// D-Bus registration retries if `org.kde.StatusNotifierWatcher` is not yet
/// ready at KDE Plasma login or restarts mid-session.
#[derive(Clone)]
pub struct TrayHandle {
    inner: Arc<Mutex<Handle<MaterialYouTray>>>,
}

impl TrayHandle {
    /// Update the underlying `MaterialYouTray` model and notify `ksni`.
    pub fn update<R, F: FnOnce(&mut MaterialYouTray) -> R>(&self, f: F) -> R {
        let guard = self.inner.lock().unwrap();
        guard.update(f)
    }

    /// Shut down the underlying `ksni` tray service.
    pub fn shutdown(&self) {
        let guard = self.inner.lock().unwrap();
        guard.shutdown();
    }
}

/// Spawn the `ksni` StatusNotifierItem service in a supervised background thread and return
/// the resilient `TrayHandle` along with the `Receiver<TrayCommand>`.
///
/// Unlike `TrayService::spawn` (which calls `.unwrap()` and panics if D-Bus times out during
/// KDE Plasma startup), this supervisor loop catches D-Bus errors and retries every 2 seconds.
pub fn spawn_tray(
    config: AppConfig,
    seed_color_hex: String,
) -> (TrayHandle, Receiver<TrayCommand>) {
    let (tx, rx) = mpsc::channel();
    let tray = MaterialYouTray::new(config, seed_color_hex, tx);
    let initial_service = TrayService::new(tray);
    let handle = TrayHandle {
        inner: Arc::new(Mutex::new(initial_service.handle())),
    };
    let handle_for_thread = handle.clone();

    std::thread::spawn(move || {
        let mut service = initial_service;
        loop {
            match service.run() {
                Ok(()) => break,
                Err(err) => {
                    tracing::warn!(
                        "StatusNotifierItem D-Bus registration/loop failed ({err}); retrying in 2s..."
                    );
                    std::thread::sleep(Duration::from_secs(2));
                    let snapshot = handle_for_thread.update(|t| t.clone());
                    let next_service = TrayService::new(snapshot);
                    if let Ok(mut guard) = handle_for_thread.inner.lock() {
                        *guard = next_service.handle();
                    }
                    service = next_service;
                }
            }
        }
    });

    (handle, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_badge_pixmap_dimensions_and_center_color() {
        let icon = generate_badge_pixmap("#ff5722");
        assert_eq!(icon.width, 32);
        assert_eq!(icon.height, 32);
        assert_eq!(icon.data.len(), 32 * 32 * 4);

        // Center pixel at (16, 16) is well inside the inner circle
        let center_offset = ((16 * 32 + 16) * 4) as usize;
        assert_eq!(
            &icon.data[center_offset..center_offset + 4],
            &[255, 0xFF, 0x57, 0x22]
        );

        // Corner pixel at (0, 0) is outside the circle and transparent
        assert_eq!(&icon.data[0..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn test_generate_badge_pixmap_invalid_hex_falls_back_to_google_blue() {
        let icon = generate_badge_pixmap("not-a-color");
        let center_offset = ((16 * 32 + 16) * 4) as usize;
        assert_eq!(
            &icon.data[center_offset..center_offset + 4],
            &[255, 0x42, 0x85, 0xF4]
        );
    }

    #[test]
    fn test_tray_metadata_and_tooltip_reflect_state() {
        let (tx, _rx) = mpsc::channel();
        let mut config = AppConfig::default();
        config.scheme_variant = SchemeVariant::Vibrant.as_u8();
        config.pause_mode = false;

        let mut tray = MaterialYouTray::new(config, "#00bcd4".to_string(), tx);
        assert_eq!(tray.id(), "kde-material-you-colors");
        assert_eq!(tray.title(), "KDE Material You Colors");
        assert_eq!(tray.icon_name(), "preferences-desktop-color");
        assert_eq!(tray.icon_pixmap().len(), 1);

        let tip_active = tray.tool_tip();
        assert_eq!(tip_active.title, "Material You Colors");
        assert!(tip_active.description.contains("Active"));
        assert!(tip_active.description.contains("Vibrant"));
        assert!(tip_active.description.contains("#00bcd4"));

        tray.config.pause_mode = true;
        let tip_paused = tray.tool_tip();
        assert!(tip_paused.description.contains("Paused"));
    }

    #[test]
    fn test_tray_activate_sends_open_settings_window() {
        let (tx, rx) = mpsc::channel();
        let mut tray = MaterialYouTray::new(AppConfig::default(), "#4285f4".to_string(), tx);

        tray.activate(100, 200);
        assert_eq!(rx.try_recv(), Ok(TrayCommand::OpenSettingsWindow));
    }

    #[test]
    fn test_menu_structure_and_action_callbacks() {
        let (tx, rx) = mpsc::channel();
        let mut tray = MaterialYouTray::new(AppConfig::default(), "#4285f4".to_string(), tx);

        let mut items = tray.menu();
        assert_eq!(items.len(), 8);

        // 0: Open Settings...
        match &mut items[0] {
            MenuItem::Standard(item) => {
                assert_eq!(item.label, "Open Settings...");
                assert_eq!(item.icon_name, "configure");
                (item.activate)(&mut tray);
            }
            _ => panic!("Expected StandardItem at index 0"),
        }
        assert_eq!(rx.try_recv(), Ok(TrayCommand::OpenSettingsWindow));

        // 1: Fetch Colors Now
        match &mut items[1] {
            MenuItem::Standard(item) => {
                assert_eq!(item.label, "Fetch Colors Now");
                assert_eq!(item.icon_name, "view-refresh");
                (item.activate)(&mut tray);
            }
            _ => panic!("Expected StandardItem at index 1"),
        }
        assert_eq!(rx.try_recv(), Ok(TrayCommand::FetchColorsNow));

        // 2: Pause Automatic Theming CheckmarkItem
        match &mut items[2] {
            MenuItem::Checkmark(item) => {
                assert_eq!(item.label, "Pause Automatic Theming");
                assert!(!item.checked);
                (item.activate)(&mut tray);
            }
            _ => panic!("Expected CheckmarkItem at index 2"),
        }
        assert!(tray.config.pause_mode);
        assert_eq!(rx.try_recv(), Ok(TrayCommand::TogglePause(true)));

        // 3: Separator
        assert!(matches!(items[3], MenuItem::Separator));

        // 4: Theme Mode SubMenu with RadioGroup
        match &mut items[4] {
            MenuItem::SubMenu(sub) => {
                assert_eq!(sub.label, "Theme Mode");
                assert_eq!(sub.submenu.len(), 1);
                match &mut sub.submenu[0] {
                    MenuItem::RadioGroup(rg) => {
                        assert_eq!(rg.selected, 2); // Follow Color Scheme default
                        assert_eq!(rg.options.len(), 3);
                        assert_eq!(rg.options[0].label, "Dark");
                        assert_eq!(rg.options[1].label, "Light");
                        assert_eq!(rg.options[2].label, "Follow Color Scheme");

                        (rg.select)(&mut tray, 0);
                        assert!(!tray.config.plasma_follows_scheme);
                        assert_eq!(tray.config.light, Some(false));
                        assert_eq!(
                            rx.try_recv(),
                            Ok(TrayCommand::SetThemeMode(ThemeModeSelection::Dark))
                        );

                        (rg.select)(&mut tray, 1);
                        assert!(!tray.config.plasma_follows_scheme);
                        assert_eq!(tray.config.light, Some(true));
                        assert_eq!(
                            rx.try_recv(),
                            Ok(TrayCommand::SetThemeMode(ThemeModeSelection::Light))
                        );

                        (rg.select)(&mut tray, 2);
                        assert!(tray.config.plasma_follows_scheme);
                        assert_eq!(tray.config.light, None);
                        assert_eq!(
                            rx.try_recv(),
                            Ok(TrayCommand::SetThemeMode(ThemeModeSelection::FollowScheme))
                        );
                    }
                    _ => panic!("Expected RadioGroup inside Theme Mode SubMenu"),
                }
            }
            _ => panic!("Expected SubMenu at index 4"),
        }

        // 5: Scheme Variant SubMenu with RadioGroup (9 variants)
        match &mut items[5] {
            MenuItem::SubMenu(sub) => {
                assert_eq!(sub.label, "Scheme Variant");
                assert_eq!(sub.submenu.len(), 1);
                match &mut sub.submenu[0] {
                    MenuItem::RadioGroup(rg) => {
                        assert_eq!(rg.selected, 5); // TonalSpot default
                        assert_eq!(rg.options.len(), 9);
                        assert_eq!(rg.options[0].label, "Content");
                        assert_eq!(rg.options[8].label, "Fruit Salad");

                        for idx in 0..9 {
                            (rg.select)(&mut tray, idx);
                            assert_eq!(tray.config.scheme_variant, idx as u8);
                            assert_eq!(
                                rx.try_recv(),
                                Ok(TrayCommand::SetSchemeVariant(idx as u8))
                            );
                        }
                    }
                    _ => panic!("Expected RadioGroup inside Scheme Variant SubMenu"),
                }
            }
            _ => panic!("Expected SubMenu at index 5"),
        }

        // 6: Separator
        assert!(matches!(items[6], MenuItem::Separator));

        // 7: Quit
        match &mut items[7] {
            MenuItem::Standard(item) => {
                assert_eq!(item.label, "Quit");
                assert_eq!(item.icon_name, "application-exit");
                (item.activate)(&mut tray);
            }
            _ => panic!("Expected StandardItem at index 7"),
        }
        assert_eq!(rx.try_recv(), Ok(TrayCommand::Quit));
    }
}
