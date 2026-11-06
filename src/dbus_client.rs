use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use image::{ImageBuffer, Rgba, RgbaImage};
use zbus::zvariant::{Fd, OwnedValue, Value};
use zbus::Connection;

/// Outcome of checking or acquiring the single-instance daemon lock.
#[derive(Debug)]
pub enum InstanceAction {
    /// No other running instance was found; this process now holds the PID guard.
    Acquired(SingleInstanceGuard),
    /// An existing live instance was found and signaled via the `.reload` trigger file.
    SignaledExisting(u32),
    /// `--stop` was requested; if a live instance was running, it was cleanly terminated.
    StoppedExisting(Option<u32>),
}

/// RAII guard for the daemon PID file. Removes the PID and reload files on drop
/// if the PID file still belongs to the current process.
#[derive(Debug)]
pub struct SingleInstanceGuard {
    pid_file: PathBuf,
    reload_file: PathBuf,
    pid: u32,
}

impl SingleInstanceGuard {
    /// Path to the active PID file.
    pub fn pid_file(&self) -> &Path {
        &self.pid_file
    }

    /// Path to the companion reload trigger file watched by `notify`.
    pub fn reload_file(&self) -> &Path {
        &self.reload_file
    }

    /// Process ID recorded by this guard.
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if let Ok(content) = std::fs::read_to_string(&self.pid_file) {
            if content.trim().parse::<u32>().ok() == Some(self.pid) {
                let _ = std::fs::remove_file(&self.pid_file);
            }
        }
        let _ = std::fs::remove_file(&self.reload_file);
    }
}

/// Return the current username for temporary runtime files.
pub fn current_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".to_string())
}

/// Return the default PID file path `/tmp/kde-material-you-colors-$USER.pid`.
pub fn default_pid_file_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "kde-material-you-colors-{}.pid",
        current_username()
    ))
}

/// Return the default reload trigger file path `/tmp/kde-material-you-colors-$USER.reload`.
pub fn default_reload_trigger_path() -> PathBuf {
    reload_trigger_path_for_pid_file(&default_pid_file_path())
}

/// Derive the companion `.reload` trigger path for a given PID file path.
pub fn reload_trigger_path_for_pid_file(pid_file: &Path) -> PathBuf {
    pid_file.with_extension("reload")
}

/// Check whether `pid` exists in `/proc/<pid>` and is not a zombie (`Z`) process.
pub fn is_pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));
    if !proc_dir.exists() {
        return false;
    }
    if let Ok(status) = std::fs::read_to_string(proc_dir.join("status")) {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("State:") {
                let state_char = rest.trim().chars().next().unwrap_or(' ');
                if state_char == 'Z' || state_char == 'X' {
                    return false;
                }
                break;
            }
        }
    }
    true
}

/// Check if a daemon instance recorded in `pid_file` is alive in `/proc/<pid>`.
///
/// - If `stop_only` is `true`, cleanly terminates the running process (with `SIGTERM`)
///   instead of `SIGKILL`, removes the PID file, and returns `InstanceAction::StoppedExisting`.
/// - If another live instance is already running, writes a timestamp to the companion
///   `.reload` trigger file (watched by `notify`) and returns `InstanceAction::SignaledExisting(pid)`
///   so the caller can exit `0` without killing the active daemon (fixing Bug 5).
/// - Otherwise writes the current PID to `pid_file` and returns `InstanceAction::Acquired`.
pub fn signal_existing_or_acquire(pid_file: &Path, stop_only: bool) -> Result<InstanceAction> {
    let reload_file = reload_trigger_path_for_pid_file(pid_file);
    let current_pid = std::process::id();

    let existing_pid = if pid_file.exists() {
        std::fs::read_to_string(pid_file)
            .ok()
            .and_then(|s| s.lines().next()?.trim().parse::<u32>().ok())
            .filter(|&pid| pid != current_pid && is_pid_alive(pid))
    } else {
        None
    };

    if stop_only {
        if let Some(pid) = existing_pid {
            let _ = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
            let _ = std::fs::remove_file(pid_file);
            let _ = std::fs::remove_file(&reload_file);
            return Ok(InstanceAction::StoppedExisting(Some(pid)));
        }
        let _ = std::fs::remove_file(pid_file);
        let _ = std::fs::remove_file(&reload_file);
        return Ok(InstanceAction::StoppedExisting(None));
    }

    if let Some(pid) = existing_pid {
        if let Some(parent) = reload_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::fs::write(&reload_file, format!("reload:{now_nanos}\n")).with_context(|| {
            format!(
                "Failed to write reload trigger file at {}",
                reload_file.display()
            )
        })?;
        return Ok(InstanceAction::SignaledExisting(pid));
    }

    if let Some(parent) = pid_file.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create PID file directory {}", parent.display())
            })?;
        }
    }

    std::fs::write(pid_file, format!("{current_pid}\n"))
        .with_context(|| format!("Failed to write PID file at {}", pid_file.display()))?;

    Ok(InstanceAction::Acquired(SingleInstanceGuard {
        pid_file: pid_file.to_path_buf(),
        reload_file,
        pid: current_pid,
    }))
}

/// Convert a `zbus::zvariant::Value` into a clean string representation without panicking.
pub fn zvariant_value_to_string(val: &Value<'_>) -> Option<String> {
    match val {
        Value::Str(s) => Some(s.as_str().to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::U8(n) => Some(n.to_string()),
        Value::I16(n) => Some(n.to_string()),
        Value::U16(n) => Some(n.to_string()),
        Value::I32(n) => Some(n.to_string()),
        Value::U32(n) => Some(n.to_string()),
        Value::I64(n) => Some(n.to_string()),
        Value::U64(n) => Some(n.to_string()),
        Value::F64(n) => Some(n.to_string()),
        Value::Value(inner) => zvariant_value_to_string(inner),
        Value::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .filter_map(zvariant_value_to_string)
                .collect();
            if items.is_empty() {
                None
            } else if items.len() == 1 {
                // Single integer QColor representation (matches Python int(color[0]) -> #rrggbb)
                if let Ok(rgb_int) = items[0].parse::<u32>() {
                    Some(format!("#{:06x}", rgb_int & 0xFF_FFFF))
                } else {
                    Some(items[0].clone())
                }
            } else {
                Some(items.join(","))
            }
        }
        Value::Structure(structure) => {
            let fields: Vec<String> = structure
                .fields()
                .iter()
                .filter_map(zvariant_value_to_string)
                .collect();
            if fields.is_empty() {
                None
            } else {
                Some(fields.join(","))
            }
        }
        _ => None,
    }
}

/// Convert a `zbus::zvariant::OwnedValue` into a string without panicking.
pub fn owned_value_to_string(val: &OwnedValue) -> Option<String> {
    let borrowed: &Value<'_> = val;
    zvariant_value_to_string(borrowed)
}

/// Convert a raw D-Bus map `HashMap<String, OwnedValue>` returned by `org.kde.PlasmaShell.wallpaper`
/// into a flat `HashMap<String, String>`.
pub fn convert_wallpaper_map(raw: HashMap<String, OwnedValue>) -> HashMap<String, String> {
    let mut out = HashMap::with_capacity(raw.len());
    for (k, v) in raw {
        if let Some(s) = owned_value_to_string(&v) {
            if !s.is_empty() {
                out.insert(k, s);
            }
        }
    }
    out
}

/// Query KDE Plasma 6 `org.kde.plasmashell` `/PlasmaShell` `org.kde.PlasmaShell.wallpaper(monitor)`.
///
/// Gracefully returns `Ok(HashMap::new())` when `/PlasmaShell` is unavailable or returns `{}`
/// without spamming tracebacks or panicking (fixing Bug 1 / Issue #346).
pub async fn query_plasma_wallpaper(monitor: u32) -> Result<HashMap<String, String>> {
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("Session D-Bus unavailable while querying wallpaper: {e}");
            return Ok(HashMap::new());
        }
    };

    let reply = match conn
        .call_method(
            Some("org.kde.plasmashell"),
            "/PlasmaShell",
            Some("org.kde.PlasmaShell"),
            "wallpaper",
            &(monitor,),
        )
        .await
    {
        Ok(msg) => msg,
        Err(e) => {
            tracing::debug!(
                "org.kde.PlasmaShell.wallpaper({monitor}) unavailable or returned error: {e}"
            );
            return Ok(HashMap::new());
        }
    };

    let body = reply.body();
    let raw_map: HashMap<String, OwnedValue> = match body.deserialize() {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("Failed to deserialize PlasmaShell.wallpaper reply: {e}");
            return Ok(HashMap::new());
        }
    };

    Ok(convert_wallpaper_map(raw_map))
}

fn extract_u32_meta(meta: &HashMap<String, OwnedValue>, key: &str) -> Option<u32> {
    let val = meta.get(key)?;
    if let Ok(v) = u32::try_from(val) {
        return Some(v);
    }
    if let Ok(v) = i32::try_from(val) {
        return u32::try_from(v).ok();
    }
    if let Ok(v) = u64::try_from(val) {
        return u32::try_from(v).ok();
    }
    owned_value_to_string(val)?.parse::<u32>().ok()
}

/// Decode bytes received from `org.kde.KWin.ScreenShot2` (either an encoded image like PNG
/// or raw `QImage` pixel data described by `meta`) into an `RgbaImage`.
pub fn decode_kwin_screenshot_bytes(
    bytes: &[u8],
    meta: &HashMap<String, OwnedValue>,
) -> Result<RgbaImage> {
    if bytes.is_empty() {
        return Err(anyhow!("KWin ScreenShot2 pipe returned 0 bytes"));
    }

    // Fast path: if KWin wrote an encoded image (e.g. PNG/JPEG/WebP), decode directly
    if let Ok(dynamic_img) = image::load_from_memory(bytes) {
        return Ok(dynamic_img.into_rgba8());
    }

    let width = extract_u32_meta(meta, "width")
        .ok_or_else(|| anyhow!("Missing 'width' in KWin ScreenShot2 metadata"))?;
    let height = extract_u32_meta(meta, "height")
        .ok_or_else(|| anyhow!("Missing 'height' in KWin ScreenShot2 metadata"))?;
    if width == 0 || height == 0 {
        return Err(anyhow!(
            "Invalid dimensions {width}x{height} in KWin ScreenShot2 metadata"
        ));
    }

    let format = extract_u32_meta(meta, "format").unwrap_or(6);
    let bytes_per_pixel: u32 = match format {
        13 | 29 => 3, // QImage::Format_RGB888 / Format_BGR888
        _ => 4,       // QImage::Format_RGB32 (4), ARGB32 (5), ARGB32_Premultiplied (6), RGBA8888 (16..18)
    };
    let stride = extract_u32_meta(meta, "stride").unwrap_or(width * bytes_per_pixel);
    let required_len = (stride as usize)
        .saturating_mul(height.saturating_sub(1) as usize)
        .saturating_add((width * bytes_per_pixel) as usize);

    if bytes.len() < required_len {
        return Err(anyhow!(
            "KWin ScreenShot2 buffer too short: got {} bytes, expected at least {} ({width}x{height}, stride {stride})",
            bytes.len(),
            required_len
        ));
    }

    let mut img: RgbaImage = ImageBuffer::new(width, height);
    for y in 0..height {
        let row_start = (y as usize) * (stride as usize);
        for x in 0..width {
            let px_start = row_start + (x as usize) * (bytes_per_pixel as usize);
            let rgba = match format {
                // QImage::Format_RGB32 (4), Format_ARGB32 (5), Format_ARGB32_Premultiplied (6)
                // Stored as 0xAARRGGBB in native little-endian -> [B, G, R, A]
                4 => [bytes[px_start + 2], bytes[px_start + 1], bytes[px_start], 255],
                5 | 6 => [
                    bytes[px_start + 2],
                    bytes[px_start + 1],
                    bytes[px_start],
                    bytes[px_start + 3],
                ],
                // QImage::Format_RGB888 (13)
                13 => [bytes[px_start], bytes[px_start + 1], bytes[px_start + 2], 255],
                // QImage::Format_BGR888 (29)
                29 => [bytes[px_start + 2], bytes[px_start + 1], bytes[px_start], 255],
                // QImage::Format_RGBX8888 (16), Format_RGBA8888 (17), Format_RGBA8888_Premultiplied (18)
                16 => [bytes[px_start], bytes[px_start + 1], bytes[px_start + 2], 255],
                _ => [
                    bytes[px_start],
                    bytes[px_start + 1],
                    bytes[px_start + 2],
                    bytes[px_start + 3],
                ],
            };
            img.put_pixel(x, y, Rgba(rgba));
        }
    }

    Ok(img)
}

async fn call_kwin_screenshot_method(
    conn: &Connection,
    method: &str,
) -> Result<(Vec<u8>, HashMap<String, OwnedValue>)> {
    let (mut reader, writer) =
        UnixStream::pair().context("Failed to create UnixStream pipe pair for KWin ScreenShot2")?;

    let read_handle = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(buf)
    });

    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("include-decoration", Value::from(false));
    options.insert("include-cursor", Value::from(false));
    options.insert("native-resolution", Value::from(false));

    let reply_res = {
        let fd = Fd::from(&writer);
        conn.call_method(
            Some("org.kde.KWin"),
            "/org/kde/KWin/ScreenShot2",
            Some("org.kde.KWin.ScreenShot2"),
            method,
            &(options, fd),
        )
        .await
    };
    // Drop our local write end immediately so `reader.read_to_end` finishes when KWin closes its FD
    drop(writer);

    let reply = reply_res.with_context(|| format!("KWin ScreenShot2.{method} D-Bus call failed"))?;
    let meta: HashMap<String, OwnedValue> = reply
        .body()
        .deserialize()
        .with_context(|| format!("Failed to deserialize KWin ScreenShot2.{method} metadata"))?;

    let raw_bytes = read_handle
        .await
        .context("KWin screenshot pipe reader task panicked")?
        .context("Failed reading bytes from KWin screenshot pipe")?;

    Ok((raw_bytes, meta))
}

/// Capture a workspace or active-screen screenshot natively via `org.kde.KWin.ScreenShot2`
/// using a pipe file descriptor and save it as a PNG at `output_path` (fixing Bug 4).
pub async fn capture_workspace_screenshot(output_path: &Path) -> Result<bool> {
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("Session D-Bus unavailable for KWin screenshot: {e}");
            return Ok(false);
        }
    };

    let (raw_bytes, meta) = match call_kwin_screenshot_method(&conn, "CaptureWorkspace").await {
        Ok(res) => res,
        Err(e) => {
            tracing::debug!("CaptureWorkspace failed ({e}), trying CaptureActiveScreen");
            match call_kwin_screenshot_method(&conn, "CaptureActiveScreen").await {
                Ok(res) => res,
                Err(e2) => {
                    tracing::debug!("CaptureActiveScreen also failed: {e2}");
                    return Ok(false);
                }
            }
        }
    };

    let rgba_img = match decode_kwin_screenshot_bytes(&raw_bytes, &meta) {
        Ok(img) => img,
        Err(e) => {
            tracing::warn!("Failed to decode KWin screenshot buffer: {e}");
            return Ok(false);
        }
    };

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create screenshot parent directory {}",
                    parent.display()
                )
            })?;
        }
    }

    rgba_img
        .save_with_format(output_path, image::ImageFormat::Png)
        .with_context(|| format!("Failed to save KWin screenshot to {}", output_path.display()))?;

    Ok(output_path.exists())
}

/// Reconfigure KWin via `org.kde.KWin` `/KWin` `org.kde.KWin.reconfigure`.
pub async fn reconfigure_kwin() -> Result<()> {
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("Session D-Bus unavailable for KWin.reconfigure: {e}");
            return Ok(());
        }
    };

    if let Err(e) = conn
        .call_method(
            Some("org.kde.KWin"),
            "/KWin",
            Some("org.kde.KWin"),
            "reconfigure",
            &(),
        )
        .await
    {
        tracing::debug!("Could not call org.kde.KWin.reconfigure: {e}");
    }

    Ok(())
}

/// Trigger KWin's smooth theme transition (`org.kde.KWin.BlendChanges.start`) if supported.
pub async fn start_kwin_blend_changes() -> Result<()> {
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let _ = conn
        .call_method(
            Some("org.kde.KWin"),
            "/org/kde/KWin/BlendChanges",
            Some("org.kde.KWin.BlendChanges"),
            "start",
            &(),
        )
        .await;

    Ok(())
}

/// Reload the KDE Rounded Corners KWin effect (`kwin4_effect_shapecorners`) if loaded.
pub async fn reload_kde_rounded_corners() -> Result<()> {
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let effect_name = "kwin4_effect_shapecorners";
    let is_loaded_reply = conn
        .call_method(
            Some("org.kde.KWin"),
            "/Effects",
            Some("org.kde.kwin.Effects"),
            "isEffectLoaded",
            &(effect_name,),
        )
        .await;

    if let Ok(msg) = is_loaded_reply {
        if msg.body().deserialize::<bool>().unwrap_or(false) {
            let _ = conn
                .call_method(
                    Some("org.kde.KWin"),
                    "/Effects",
                    Some("org.kde.kwin.Effects"),
                    "reconfigureEffect",
                    &(effect_name,),
                )
                .await;
        }
    }

    Ok(())
}

/// Emit `/KlassyDecoration` `org.kde.Klassy.Style.updateDecorationColorCache` signal on the session bus.
pub async fn notify_klassy_decoration() -> Result<()> {
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("Session D-Bus unavailable for Klassy notification: {e}");
            return Ok(());
        }
    };

    if let Err(e) = conn
        .emit_signal(
            None::<&str>,
            "/KlassyDecoration",
            "org.kde.Klassy.Style",
            "updateDecorationColorCache",
            &(),
        )
        .await
    {
        tracing::debug!("Failed to emit Klassy updateDecorationColorCache signal: {e}");
    }

    Ok(())
}

/// Extract child node names from a D-Bus introspection XML payload.
pub fn parse_introspect_child_nodes(xml: &str) -> Vec<String> {
    let mut nodes = Vec::new();
    for line in xml.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("<node") {
            if let Some(name_idx) = rest.find("name=\"") {
                let after_quote = &rest[name_idx + 6..];
                if let Some(end_quote) = after_quote.find('"') {
                    let name = &after_quote[..end_quote];
                    if !name.is_empty() && !name.starts_with('/') {
                        nodes.push(name.to_string());
                    }
                }
            }
        }
    }
    nodes
}

/// Reload all running `org.kde.konsole-*` `/Sessions/*` by toggling between `profile_name`
/// and `"TempMyou"` via `org.kde.konsole.Session.setProfile`.
pub async fn reload_konsole_sessions(profile_name: &str) -> Result<()> {
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("Session D-Bus unavailable for Konsole reload: {e}");
            return Ok(());
        }
    };

    let names_reply = match conn
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "ListNames",
            &(),
        )
        .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("Could not list D-Bus names for Konsole reload: {e}");
            return Ok(());
        }
    };

    let all_names: Vec<String> = names_reply.body().deserialize().unwrap_or_default();
    let konsole_services: Vec<String> = all_names
        .into_iter()
        .filter(|name| name.contains("org.kde.konsole"))
        .collect();

    if konsole_services.is_empty() {
        return Ok(());
    }

    let active_profile_file = std::env::temp_dir().join(format!(
        "kde-material-you-colors-konsole-profile-{}",
        current_username()
    ));

    for service in konsole_services {
        let introspect_reply = match conn
            .call_method(
                Some(service.as_str()),
                "/Sessions",
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await
        {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!("Failed to introspect {service} /Sessions: {e}");
                continue;
            }
        };

        let xml: String = introspect_reply.body().deserialize().unwrap_or_default();
        for node in parse_introspect_child_nodes(&xml) {
            let session_path = format!("/Sessions/{node}");
            let current_profile = match conn
                .call_method(
                    Some(service.as_str()),
                    session_path.as_str(),
                    Some("org.kde.konsole.Session"),
                    "profile",
                    &(),
                )
                .await
            {
                Ok(msg) => msg.body().deserialize::<String>().unwrap_or_default(),
                Err(_) => continue,
            };

            let new_profile = if current_profile == profile_name {
                "TempMyou"
            } else {
                profile_name
            };

            if conn
                .call_method(
                    Some(service.as_str()),
                    session_path.as_str(),
                    Some("org.kde.konsole.Session"),
                    "setProfile",
                    &(new_profile,),
                )
                .await
                .is_ok()
            {
                let _ = std::fs::write(&active_profile_file, new_profile);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Array;

    #[test]
    fn test_single_instance_pid_acquisition_and_reload_signaling() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pid_file = dir.path().join("kde-material-you-colors-test.pid");
        let reload_file = reload_trigger_path_for_pid_file(&pid_file);

        // 1. Stale non-existent PID in pid_file is ignored and overwritten by current process
        std::fs::write(&pid_file, "99999999\n").unwrap();
        let action = signal_existing_or_acquire(&pid_file, false).unwrap();
        let guard = match action {
            InstanceAction::Acquired(g) => g,
            other => panic!("Expected Acquired, got {other:?}"),
        };
        assert_eq!(guard.pid(), std::process::id());
        assert_eq!(
            std::fs::read_to_string(&pid_file).unwrap().trim(),
            std::process::id().to_string()
        );
        drop(guard);
        assert!(!pid_file.exists());

        // 2. When a live external PID (PID 1 / init or parent PID) is recorded in pid_file,
        // signal_existing_or_acquire writes the .reload trigger file without killing the process!
        let live_other_pid = std::os::unix::process::parent_id();
        assert!(is_pid_alive(live_other_pid));
        std::fs::write(&pid_file, format!("{live_other_pid}\n")).unwrap();

        let action2 = signal_existing_or_acquire(&pid_file, false).unwrap();
        match action2 {
            InstanceAction::SignaledExisting(pid) => assert_eq!(pid, live_other_pid),
            other => panic!("Expected SignaledExisting({live_other_pid}), got {other:?}"),
        }
        assert!(reload_file.exists());
        let reload_content = std::fs::read_to_string(&reload_file).unwrap();
        assert!(reload_content.starts_with("reload:"));
        // Verify the external live process is still alive
        assert!(is_pid_alive(live_other_pid));
    }

    #[test]
    fn test_single_instance_stop_only_with_no_live_process() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pid_file = dir.path().join("kde-material-you-colors-stop.pid");
        std::fs::write(&pid_file, "99999998\n").unwrap();

        let action = signal_existing_or_acquire(&pid_file, true).unwrap();
        match action {
            InstanceAction::StoppedExisting(None) => {}
            other => panic!("Expected StoppedExisting(None), got {other:?}"),
        }
        assert!(!pid_file.exists());
    }

    #[test]
    fn test_convert_wallpaper_map_handles_strings_and_qcolor_arrays_gracefully() {
        let mut raw: HashMap<String, OwnedValue> = HashMap::new();
        raw.insert(
            "wallpaperPlugin".to_string(),
            OwnedValue::try_from(Value::from("org.kde.image")).unwrap(),
        );
        raw.insert(
            "Image".to_string(),
            OwnedValue::try_from(Value::from("file:///usr/share/wallpapers/Next/contents/images/1920x1080.png")).unwrap(),
        );

        let mut color_arr = Array::new(&zbus::zvariant::Signature::U32);
        color_arr.append(Value::U32(0x4285F4)).unwrap();
        raw.insert(
            "Color".to_string(),
            OwnedValue::try_from(Value::Array(color_arr)).unwrap(),
        );

        let converted = convert_wallpaper_map(raw);
        assert_eq!(
            converted.get("wallpaperPlugin").map(String::as_str),
            Some("org.kde.image")
        );
        assert_eq!(
            converted.get("Image").map(String::as_str),
            Some("file:///usr/share/wallpapers/Next/contents/images/1920x1080.png")
        );
        assert_eq!(converted.get("Color").map(String::as_str), Some("#4285f4"));

        // Empty map returns empty map without panic
        assert!(convert_wallpaper_map(HashMap::new()).is_empty());
    }

    #[test]
    fn test_decode_kwin_screenshot_bytes_raw_bgra_and_png() {
        // Test raw QImage::Format_ARGB32_Premultiplied (6) [B, G, R, A] 2x1 pixels
        let raw_bgra: [u8; 8] = [
            0xF4, 0x85, 0x42, 0xFF, // Pixel (0, 0): R=0x42, G=0x85, B=0xF4, A=0xFF
            0x00, 0xFF, 0x00, 0xFF, // Pixel (1, 0): R=0x00, G=0xFF, B=0x00, A=0xFF
        ];
        let mut meta: HashMap<String, OwnedValue> = HashMap::new();
        meta.insert("width".into(), OwnedValue::from(2u32));
        meta.insert("height".into(), OwnedValue::from(1u32));
        meta.insert("stride".into(), OwnedValue::from(8u32));
        meta.insert("format".into(), OwnedValue::from(6u32));

        let decoded = decode_kwin_screenshot_bytes(&raw_bgra, &meta).unwrap();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.get_pixel(0, 0), &Rgba([0x42, 0x85, 0xF4, 0xFF]));
        assert_eq!(decoded.get_pixel(1, 0), &Rgba([0x00, 0xFF, 0x00, 0xFF]));
    }

    #[test]
    fn test_parse_introspect_child_nodes() {
        let xml = r#"
<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-Bus Object Introspection 1.0//EN"
"http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <node name="1"/>
  <node name="2"/>
  <node name="14"/>
</node>
"#;
        assert_eq!(parse_introspect_child_nodes(xml), vec!["1", "2", "14"]);
    }
}
