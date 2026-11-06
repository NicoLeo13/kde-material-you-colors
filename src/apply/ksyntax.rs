use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::apply::ThemePaths;
use crate::scheme::MaterialYouScheme;

/// Paths of written KSyntaxHighlighting `.theme` files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KSyntaxArtifacts {
    pub dark_theme_path: PathBuf,
    pub light_theme_path: PathBuf,
}

fn tone_at(palette: &[String], index: usize, fallback: &str) -> String {
    palette
        .get(index)
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

/// Generate the `Material You Dark` KSyntaxHighlighting JSON structure matching `schemeconfigs.py`.
pub fn generate_ksyntax_dark_theme(scheme: &MaterialYouScheme) -> Value {
    let bg_color = scheme
        .pywal
        .dark
        .colors
        .get("color0")
        .cloned()
        .unwrap_or_else(|| "#111111".to_string());
    let on_surface = scheme
        .schemes
        .dark
        .get("onSurface")
        .cloned()
        .unwrap_or_else(|| "#e3e3e3".to_string());
    let mark_error = scheme
        .custom
        .get("negative")
        .map(|c| c.dark.on_color.clone())
        .unwrap_or_else(|| "#da4453".to_string());
    let mark_warning = scheme
        .custom
        .get("neutral")
        .map(|c| c.dark.on_color.clone())
        .unwrap_or_else(|| "#f67400".to_string());

    let sec_20 = tone_at(&scheme.tones.secondary, 20, "#2b2b2b");
    let sec_30 = tone_at(&scheme.tones.secondary, 30, "#3b3b3b");
    let sec_35 = tone_at(&scheme.tones.secondary, 35, "#454545");
    let neu_45 = tone_at(&scheme.tones.neutral, 45, "#6b6b6b");

    json!({
        "metadata": {
            "copyright": [
                "SPDX-FileCopyrightText: 2016 Volker Krause <vkrause@kde.org>",
                "SPDX-FileCopyrightText: 2016 Dominik Haumann <dhaumann@kde.org>"
            ],
            "license": "SPDX-License-Identifier: MIT",
            "name": "Material You Dark",
            "revision": 7
        },
        "editor-colors": {
            "BackgroundColor": bg_color,
            "BracketMatching": sec_35,
            "CodeFolding": "#224e65",
            "CurrentLine": sec_20,
            "CurrentLineNumber": on_surface,
            "IconBorder": bg_color,
            "IndentationLine": sec_20,
            "LineNumbers": neu_45,
            "MarkBookmark": "#0404bf",
            "MarkBreakpointActive": "#8b0607",
            "MarkBreakpointDisabled": "#820683",
            "MarkBreakpointReached": "#6d6e07",
            "MarkError": mark_error,
            "MarkExecution": "#4d4e50",
            "MarkWarning": mark_warning,
            "ModifiedLines": "#c04900",
            "ReplaceHighlight": "#808021",
            "SavedLines": "#1c8042",
            "SearchHighlight": "#218058",
            "Separator": "#3f4347",
            "SpellChecking": "#c0392b",
            "TabMarker": "#4d4d4d",
            "TemplateBackground": sec_20,
            "TemplateFocusedPlaceholder": "#123723",
            "TemplatePlaceholder": "#123723",
            "TemplateReadOnlyPlaceholder": "#4d1f24",
            "TextSelection": sec_30,
            "WordWrapMarker": "#3a3f44"
        },
        "text-styles": {
            "Alert": {
                "background-color": "#4d1f24",
                "bold": "true",
                "selected-text-color": "#95da4c",
                "text-color": "#95da4c"
            },
            "Annotation": {
                "selected-text-color": "#54aa75",
                "text-color": "#3f8058"
            },
            "Attribute": {
                "selected-text-color": "#fdbc4b",
                "text-color": "#2980b9"
            },
            "BaseN": {
                "selected-text-color": "#f67400",
                "text-color": "#f67400"
            },
            "BuiltIn": {
                "selected-text-color": "#bdc3c7",
                "text-color": "#7f8c8d"
            },
            "Char": {
                "selected-text-color": "#3daee9",
                "text-color": "#3daee9"
            },
            "Comment": {
                "selected-text-color": "#808080",
                "text-color": "#7a7c7d"
            },
            "CommentVar": {
                "selected-text-color": "#94a3a4",
                "text-color": "#7f8c8d"
            },
            "Constant": {
                "bold": "true",
                "selected-text-color": "#27aeae",
                "text-color": "#27aeae"
            },
            "ControlFlow": {
                "bold": "true",
                "selected-text-color": "#fdbc4b",
                "text-color": "#fdbc4b"
            },
            "DataType": {
                "selected-text-color": "#fdbc4b",
                "text-color": "#2980b9"
            },
            "DecVal": {
                "selected-text-color": "#f67400",
                "text-color": "#f67400"
            },
            "Documentation": {
                "selected-text-color": "#da4453",
                "text-color": "#a43340"
            },
            "Error": {
                "selected-text-color": "#da4453",
                "text-color": "#da4453",
                "underline": "true"
            },
            "Extension": {
                "bold": "true",
                "selected-text-color": "#bdc3c7",
                "text-color": "#0099ff"
            },
            "Float": {
                "selected-text-color": "#f67400",
                "text-color": "#f67400"
            },
            "Function": {
                "selected-text-color": "#af81ff",
                "text-color": "#8e44ad"
            },
            "Import": {
                "selected-text-color": "#27ae60",
                "text-color": "#27ae60"
            },
            "Information": {
                "selected-text-color": "#e46700",
                "text-color": "#c45b00"
            },
            "Keyword": {
                "bold": "true",
                "selected-text-color": on_surface,
                "text-color": on_surface
            },
            "Normal": {
                "selected-text-color": on_surface,
                "text-color": on_surface
            },
            "Operator": {
                "selected-text-color": "#54aa75",
                "text-color": "#3f8058"
            },
            "Others": {
                "selected-text-color": "#27ae60",
                "text-color": "#27ae60"
            },
            "Preprocessor": {
                "selected-text-color": "#27ae60",
                "text-color": "#27ae60"
            },
            "RegionMarker": {
                "background-color": "#153042",
                "selected-text-color": "#3daee9",
                "text-color": "#2980b9"
            },
            "SpecialChar": {
                "selected-text-color": "#3daee9",
                "text-color": "#3daee9"
            },
            "SpecialString": {
                "selected-text-color": "#da4453",
                "text-color": "#da4453"
            },
            "String": {
                "selected-text-color": "#f44f4f",
                "text-color": "#f44f4f"
            },
            "Variable": {
                "selected-text-color": "#27aeae",
                "text-color": "#27aeae"
            },
            "VerbatimString": {
                "selected-text-color": "#da4453",
                "text-color": "#da4453"
            },
            "Warning": {
                "selected-text-color": "#da4453",
                "text-color": "#da4453"
            }
        },
        "custom-styles": {}
    })
}

/// Generate the `Material You Light` KSyntaxHighlighting JSON structure matching `schemeconfigs.py`.
pub fn generate_ksyntax_light_theme(scheme: &MaterialYouScheme) -> Value {
    let bg_color = scheme
        .pywal
        .light
        .colors
        .get("color0")
        .cloned()
        .unwrap_or_else(|| "#fefefe".to_string());
    let on_surface = scheme
        .schemes
        .light
        .get("onSurface")
        .cloned()
        .unwrap_or_else(|| "#1f1f1f".to_string());
    let mark_error = scheme
        .custom
        .get("negative")
        .map(|c| c.dark.on_color.clone())
        .unwrap_or_else(|| "#da4453".to_string());
    let mark_warning = scheme
        .custom
        .get("neutral")
        .map(|c| c.dark.on_color.clone())
        .unwrap_or_else(|| "#f67400".to_string());

    let sec_65 = tone_at(&scheme.tones.secondary, 65, "#9e9e9e");
    let sec_80 = tone_at(&scheme.tones.secondary, 80, "#c8c8c8");
    let neu_55 = tone_at(&scheme.tones.neutral, 55, "#858585");

    json!({
        "metadata": {
            "copyright": [
                "SPDX-FileCopyrightText: 2016 Volker Krause <vkrause@kde.org>",
                "SPDX-FileCopyrightText: 2016 Dominik Haumann <dhaumann@kde.org>"
            ],
            "license": "SPDX-License-Identifier: MIT",
            "revision": 9,
            "name": "Material You Light"
        },
        "editor-colors": {
            "BackgroundColor": bg_color,
            "CodeFolding": "#94caef",
            "BracketMatching": sec_65,
            "CurrentLine": sec_80,
            "IconBorder": bg_color,
            "IndentationLine": sec_80,
            "LineNumbers": neu_55,
            "CurrentLineNumber": on_surface,
            "MarkBookmark": "#0000ff",
            "MarkBreakpointActive": "#ff0000",
            "MarkBreakpointReached": "#ffff00",
            "MarkBreakpointDisabled": "#ff00ff",
            "MarkExecution": "#a0a0a4",
            "MarkWarning": mark_warning,
            "MarkError": mark_error,
            "ModifiedLines": "#fdbc4b",
            "ReplaceHighlight": "#00ff00",
            "SavedLines": "#2ecc71",
            "SearchHighlight": "#ffff00",
            "TextSelection": sec_80,
            "Separator": "#d5d5d5",
            "SpellChecking": "#bf0303",
            "TabMarker": "#d2d2d2",
            "TemplateBackground": sec_80,
            "TemplatePlaceholder": "#baf8ce",
            "TemplateFocusedPlaceholder": "#76da98",
            "TemplateReadOnlyPlaceholder": "#f6e6e6",
            "WordWrapMarker": "#ededed"
        },
        "text-styles": {
            "Normal": {
                "text-color": on_surface,
                "selected-text-color": on_surface,
                "bold": "false",
                "italic": "false",
                "underline": "false",
                "strike-through": "false"
            },
            "Keyword": {
                "text-color": on_surface,
                "selected-text-color": "#ffffff",
                "bold": "true"
            },
            "Function": {
                "text-color": "#644a9b",
                "selected-text-color": "#452886"
            },
            "Variable": {
                "text-color": "#0057ae",
                "selected-text-color": "#00316e"
            },
            "ControlFlow": {
                "text-color": on_surface,
                "selected-text-color": "#ffffff",
                "bold": "true"
            },
            "Operator": {
                "text-color": "#ca60ca",
                "selected-text-color": "#a44ea4"
            },
            "BuiltIn": {
                "text-color": "#644a9b",
                "selected-text-color": "#452886",
                "bold": "true"
            },
            "Extension": {
                "text-color": "#0095ff",
                "selected-text-color": "#ffffff",
                "bold": "true"
            },
            "Preprocessor": {
                "text-color": "#006e28",
                "selected-text-color": "#006e28"
            },
            "Attribute": {
                "text-color": "#0057ae",
                "selected-text-color": "#00316e"
            },
            "Char": {
                "text-color": "#924c9d",
                "selected-text-color": "#6c2477"
            },
            "SpecialChar": {
                "text-color": "#3daee9",
                "selected-text-color": "#fcfcfc"
            },
            "String": {
                "text-color": "#bf0303",
                "selected-text-color": "#9c0e0e"
            },
            "VerbatimString": {
                "text-color": "#e31616",
                "selected-text-color": "#9c0e0e"
            },
            "SpecialString": {
                "text-color": "#ff5500",
                "selected-text-color": "#ff5500"
            },
            "Import": {
                "text-color": "#ff5500",
                "selected-text-color": "#ff5500"
            },
            "DataType": {
                "text-color": "#0057ae",
                "selected-text-color": "#00316e"
            },
            "DecVal": {
                "text-color": "#b08000",
                "selected-text-color": "#805c00"
            },
            "BaseN": {
                "text-color": "#b08000",
                "selected-text-color": "#805c00"
            },
            "Float": {
                "text-color": "#b08000",
                "selected-text-color": "#805c00"
            },
            "Constant": {
                "text-color": "#aa5500",
                "selected-text-color": "#5e2f00"
            },
            "Comment": {
                "text-color": "#898887",
                "selected-text-color": "#5e5d5d"
            },
            "Documentation": {
                "text-color": "#607880",
                "selected-text-color": "#46585e"
            },
            "Annotation": {
                "text-color": "#ca60ca",
                "selected-text-color": "#a44ea4"
            },
            "CommentVar": {
                "text-color": "#0095ff",
                "selected-text-color": "#ffffff"
            },
            "RegionMarker": {
                "text-color": "#0057ae",
                "selected-text-color": "#00316e",
                "background-color": "#e0e9f8"
            },
            "Information": {
                "text-color": "#b08000",
                "selected-text-color": "#805c00"
            },
            "Warning": {
                "text-color": "#bf0303",
                "selected-text-color": "#9c0e0e"
            },
            "Alert": {
                "text-color": "#bf0303",
                "selected-text-color": "#9c0e0e",
                "background-color": "#f7e6e6",
                "bold": "true"
            },
            "Error": {
                "text-color": "#bf0303",
                "selected-text-color": "#9c0e0e",
                "underline": "true"
            },
            "Others": {
                "text-color": "#006e28",
                "selected-text-color": "#006e28"
            }
        },
        "custom-styles": {}
    })
}

/// Write `material-you-dark.theme` and `material-you-light.theme` into
/// `<data_dir>/org.kde.syntax-highlighting/themes/`.
pub fn write_ksyntax_themes(
    scheme: &MaterialYouScheme,
    paths: &ThemePaths,
) -> Result<KSyntaxArtifacts> {
    let themes_dir = paths.ksyntax_themes_dir();
    fs::create_dir_all(&themes_dir).with_context(|| {
        format!(
            "failed to create KSyntaxHighlighting directory at {}",
            themes_dir.display()
        )
    })?;

    let dark_theme_path = themes_dir.join("material-you-dark.theme");
    let light_theme_path = themes_dir.join("material-you-light.theme");

    let dark_json = serde_json::to_string_pretty(&generate_ksyntax_dark_theme(scheme))
        .context("failed to serialize KSyntaxHighlighting dark theme")?;
    let light_json = serde_json::to_string_pretty(&generate_ksyntax_light_theme(scheme))
        .context("failed to serialize KSyntaxHighlighting light theme")?;

    fs::write(&dark_theme_path, dark_json)
        .with_context(|| format!("failed to write {}", dark_theme_path.display()))?;
    fs::write(&light_theme_path, light_json)
        .with_context(|| format!("failed to write {}", light_theme_path.display()))?;

    Ok(KSyntaxArtifacts {
        dark_theme_path,
        light_theme_path,
    })
}
