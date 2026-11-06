use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::color::hex_to_rgb;

/// Convert a `#rrggbb` hex color string into a comma-separated `r,g,b` string (e.g. `"66,133,244"`).
pub fn hex_to_rgb_csv(hex: &str) -> String {
    let (r, g, b) = hex_to_rgb(hex).unwrap_or((0, 0, 0));
    format!("{r},{g},{b}")
}

/// Order-preserving, case-preserving INI document compatible with KDE configuration files
/// (`kdeglobals`, `konsolerc`, `breezerc`, `sierrabreezeenhancedrc`, `klassyrc`, `kwinrc`, `kwinrulesrc`)
/// and Python `configparser` (`space_around_delimiters=False`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KdeIni {
    sections: Vec<(String, Vec<(String, String)>)>,
}

impl KdeIni {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Parse INI text preserving section and key order and exact casing.
    pub fn parse(content: &str) -> Self {
        let mut ini = Self::new();
        let mut current_section: Option<String> = None;

        for raw_line in content.lines() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
                let sec = trimmed[1..trimmed.len() - 1].to_string();
                ini.ensure_section(&sec);
                current_section = Some(sec);
                continue;
            }
            if let Some(ref sec) = current_section {
                if let Some((k, v)) = trimmed.split_once('=') {
                    ini.set(sec, k.trim(), v.trim());
                } else {
                    ini.set(sec, trimmed, "");
                }
            }
        }

        ini
    }

    /// Load an INI file from `path`, or return an empty `KdeIni` if the file does not exist.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read INI file at {}", path.display()))?;
        Ok(Self::parse(&content))
    }

    /// Return `true` if `section` exists.
    pub fn has_section(&self, section: &str) -> bool {
        self.sections.iter().any(|(s, _)| s == section)
    }

    /// Ensure `section` exists in the document.
    pub fn ensure_section(&mut self, section: &str) {
        if !self.has_section(section) {
            self.sections.push((section.to_string(), Vec::new()));
        }
    }

    /// Get the value of `key` in `section`, if present.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .iter()
            .find(|(s, _)| s == section)
            .and_then(|(_, entries)| {
                entries
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.as_str())
            })
    }

    /// Set `key=value` inside `section`, creating `section` if needed and updating in-place if `key` exists.
    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        self.ensure_section(section);
        if let Some((_, entries)) = self.sections.iter_mut().find(|(s, _)| s == section) {
            if let Some((_, existing_val)) = entries.iter_mut().find(|(k, _)| k == key) {
                *existing_val = value.to_string();
            } else {
                entries.push((key.to_string(), value.to_string()));
            }
        }
    }

    /// Serialize the INI document with `key=value` formatting (no spaces around `=`).
    pub fn to_ini_string(&self) -> String {
        let mut out = String::new();
        for (idx, (section, entries)) in self.sections.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            out.push('[');
            out.push_str(section);
            out.push_str("]\n");
            for (k, v) in entries {
                out.push_str(k);
                out.push('=');
                out.push_str(v);
                out.push('\n');
            }
        }
        out
    }

    /// Write the serialized INI document to `path`, creating parent directories if needed.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create directory {}", parent.display())
                })?;
            }
        }
        fs::write(path, self.to_ini_string())
            .with_context(|| format!("failed to write INI file at {}", path.display()))
    }
}
