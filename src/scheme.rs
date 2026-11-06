use crate::color::{
    argb_to_hex, blend_to_contrast, hex_to_argb, lighten_color, material_argb_to_hex,
    multiply_lightness, multiply_saturation, oklab_blend, scale_saturation,
    sort_colors_luminance,
};
use anyhow::{Context, Result};
use image::imageops::FilterType;
use material_colors::blend::harmonize;
use material_colors::color::Argb;
use material_colors::dynamic_color::{DynamicColor, DynamicScheme, MaterialDynamicColors, Variant};
use material_colors::hct::Hct;
use material_colors::image::{AsPixels, Image as MaterialImage};
use material_colors::palette::TonalPalette;
use material_colors::quantize::{Quantizer, QuantizerCelebi};
use material_colors::score::Score;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Material You dynamic color scheme variants (`0..=8`) matching Python `schemes` list order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SchemeVariant {
    Content = 0,
    Expressive = 1,
    Fidelity = 2,
    Monochrome = 3,
    Neutral = 4,
    TonalSpot = 5,
    Vibrant = 6,
    Rainbow = 7,
    FruitSalad = 8,
}

impl Default for SchemeVariant {
    fn default() -> Self {
        Self::TonalSpot
    }
}

impl SchemeVariant {
    /// Convert a numeric index (`0..=8`) into a `SchemeVariant`, defaulting to `TonalSpot` (`5`) if out of range.
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Content,
            1 => Self::Expressive,
            2 => Self::Fidelity,
            3 => Self::Monochrome,
            4 => Self::Neutral,
            5 => Self::TonalSpot,
            6 => Self::Vibrant,
            7 => Self::Rainbow,
            8 => Self::FruitSalad,
            _ => Self::TonalSpot,
        }
    }

    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    pub const fn to_material_variant(self) -> Variant {
        match self {
            Self::Content => Variant::Content,
            Self::Expressive => Variant::Expressive,
            Self::Fidelity => Variant::Fidelity,
            Self::Monochrome => Variant::Monochrome,
            Self::Neutral => Variant::Neutral,
            Self::TonalSpot => Variant::TonalSpot,
            Self::Vibrant => Variant::Vibrant,
            Self::Rainbow => Variant::Rainbow,
            Self::FruitSalad => Variant::FruitSalad,
        }
    }
}

/// Input parameters for generating a complete Material You scheme and Pywal/Konsole palettes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemeRequest {
    pub source_color_hex: Option<String>,
    pub image_path: Option<PathBuf>,
    pub ncolor: usize,
    pub variant: SchemeVariant,
    pub chroma_multiplier: f64,
    pub tone_multiplier: f64,
    pub contrast_level: f64,
    pub frame_contrast: f64,
    pub light: bool,
    pub pywal_light: bool,
    pub light_saturation_multiplier: f64,
    pub dark_saturation_multiplier: f64,
    pub light_brightness_multiplier: f64,
    pub dark_brightness_multiplier: f64,
    pub custom_colors_list: Vec<String>,
}

impl Default for SchemeRequest {
    fn default() -> Self {
        Self {
            source_color_hex: None,
            image_path: None,
            ncolor: 0,
            variant: SchemeVariant::TonalSpot,
            chroma_multiplier: 1.0,
            tone_multiplier: 1.0,
            contrast_level: 0.0,
            frame_contrast: 0.2,
            light: false,
            pywal_light: false,
            light_saturation_multiplier: 1.0,
            dark_saturation_multiplier: 1.0,
            light_brightness_multiplier: 1.0,
            dark_brightness_multiplier: 1.0,
            custom_colors_list: Vec::new(),
        }
    }
}

/// Selected seed color metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedColor {
    pub index: usize,
    pub color: String,
}

/// Light and Dark Material 3 dynamic color role maps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemesPair {
    pub light: BTreeMap<String, String>,
    pub dark: BTreeMap<String, String>,
}

/// Tonal palettes (`0..100` hex colors each) for Material 3 roles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TonalPalettes {
    pub primary: Vec<String>,
    pub secondary: Vec<String>,
    pub tertiary: Vec<String>,
    pub neutral: Vec<String>,
    #[serde(rename = "neutralVariant")]
    pub neutral_variant: Vec<String>,
    pub error: Vec<String>,
}

/// Static harmonized role quartet (`color`, `onColor`, `container`, `onContainer`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizedRoleSet {
    pub color: String,
    #[serde(rename = "onColor")]
    pub on_color: String,
    pub container: String,
    #[serde(rename = "onContainer")]
    pub on_container: String,
}

/// Harmonized Breeze text state entry (`link`, `visited`, `negative`, `neutral`, `positive`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomRoleEntry {
    pub source: String,
    pub value: String,
    pub light: HarmonizedRoleSet,
    pub dark: HarmonizedRoleSet,
}

/// Pywal special terminal role colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PywalSpecial {
    pub background: String,
    #[serde(rename = "backgroundIntense")]
    pub background_intense: String,
    #[serde(rename = "backgroundFaint")]
    pub background_faint: String,
    pub foreground: String,
    #[serde(rename = "foregroundIntense")]
    pub foreground_intense: String,
    #[serde(rename = "foregroundFaint")]
    pub foreground_faint: String,
    pub cursor: String,
}

/// Single mode (light or dark) Pywal / Konsole 24-color scheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PywalScheme {
    pub wallpaper: String,
    pub checksum: String,
    pub alpha: String,
    pub special: PywalSpecial,
    pub colors: BTreeMap<String, String>,
}

/// Light and Dark Pywal / Konsole 24-color palettes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PywalPalettes {
    pub light: PywalScheme,
    pub dark: PywalScheme,
}

/// Wallpaper source metadata for runtime JSON export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperInfo {
    #[serde(rename = "type")]
    pub source_type: String,
    pub data: String,
}

/// Complete generated Material You scheme, tonal palettes, harmonized Breeze text states, and Pywal/Konsole palettes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialYouScheme {
    /// Ranked seed colors indexed by `"0".."N"` (also serialized as an array in `to_runtime_json` where appropriate).
    pub best: BTreeMap<String, String>,
    /// Ordered list of best seed colors.
    pub best_list: Vec<String>,
    /// Selected seed color (`best[ncolor]`).
    pub seed: SeedColor,
    /// Light and dark Material 3 dynamic role maps.
    pub schemes: SchemesPair,
    /// Tonal palettes (`primary`, `secondary`, `tertiary`, `neutral`, `neutralVariant`, `error`).
    pub tones: TonalPalettes,
    /// Harmonized Breeze text state roles (`link`, `visited`, `negative`, `neutral`, `positive`).
    pub custom: BTreeMap<String, CustomRoleEntry>,
    /// Pywal/Konsole 24-color palettes for `light` and `dark`.
    pub pywal: PywalPalettes,
    /// Wallpaper source metadata.
    pub wallpaper: WallpaperInfo,
    /// Active light mode flag.
    pub light: bool,
    /// Active pywal light mode flag.
    pub pywal_light: bool,
    /// Frame contrast value (`0.0..=1.0`).
    pub frame_contrast: f64,
}

/// Resize the image at `path` to width `128` proportionally with `FilterType::Lanczos3`,
/// quantize with `QuantizerCelebi` (max 128 clusters), and score up to 7 ranked seed colors
/// with fallback `#4285f4` (Google Blue), matching `extra_image_utils.py` and `m3_scheme_utils.py`.
pub fn extract_seed_colors_from_image(path: &Path) -> Result<Vec<String>> {
    let reader = image::ImageReader::open(path)
        .with_context(|| format!("failed to open wallpaper image at {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("failed to guess image format for {}", path.display()))?;

    let decoded = reader
        .decode()
        .with_context(|| format!("failed to decode wallpaper image at {}", path.display()))?;

    let rgba = decoded.into_rgba8();
    let orig_w = rgba.width().max(1);
    let orig_h = rgba.height().max(1);
    let base_width: u32 = 128;
    let w_percent = (base_width as f64) / (orig_w as f64);
    let h_size = ((orig_h as f64) * w_percent).trunc().max(1.0) as u32;

    let mut mat_img = MaterialImage::new(rgba);
    mat_img.resize(base_width, h_size, FilterType::Lanczos3);

    let pixels = mat_img.as_pixels();
    let quantized = QuantizerCelebi::quantize(&pixels, 128);
    let fallback = Argb::from_u32(0xFF4285F4);
    let ranked = Score::score(
        &quantized.color_to_count,
        Some(7),
        Some(fallback),
        Some(true),
    );

    if ranked.is_empty() {
        Ok(vec!["#4285f4".to_string()])
    } else {
        Ok(ranked.into_iter().map(material_argb_to_hex).collect())
    }
}

fn clip(number: f64, min: f64, max: f64, fallback: f64) -> f64 {
    if number.is_nan() {
        fallback
    } else {
        number.clamp(min, max)
    }
}

fn palette_to_hex_vec(palette: &TonalPalette) -> Vec<String> {
    (0..100)
        .map(|tone| material_argb_to_hex(palette.tone(tone)))
        .collect()
}

fn resolve_role_hex(
    mut color: DynamicColor,
    is_bg: bool,
    scheme: &DynamicScheme,
    chroma_mult: f64,
    tone_mult: f64,
    is_dark: bool,
) -> String {
    let mut hct = color.get_hct(scheme);
    if is_bg {
        let min_tone_mult = if is_dark { 0.0 } else { 0.5 };
        let new_tone = (hct.get_tone() * clip(tone_mult, min_tone_mult, 1.5, 1.0)).trunc();
        hct.set_tone(new_tone);
    }
    let new_chroma = hct.get_chroma() * clip(chroma_mult, 0.0, 10.0, 1.0);
    hct.set_chroma(new_chroma);
    let argb: Argb = hct.into();
    material_argb_to_hex(argb)
}

fn resolve_dim_role_hex(
    mut base_color: DynamicColor,
    tone_offset: f64,
    is_bg: bool,
    scheme: &DynamicScheme,
    chroma_mult: f64,
    tone_mult: f64,
    is_dark: bool,
) -> String {
    let mut hct = base_color.get_hct(scheme);
    let shifted_tone = (hct.get_tone() + tone_offset).clamp(0.0, 100.0);
    hct.set_tone(shifted_tone);
    if is_bg {
        let min_tone_mult = if is_dark { 0.0 } else { 0.5 };
        let new_tone = (hct.get_tone() * clip(tone_mult, min_tone_mult, 1.5, 1.0)).trunc();
        hct.set_tone(new_tone);
    }
    let new_chroma = hct.get_chroma() * clip(chroma_mult, 0.0, 10.0, 1.0);
    hct.set_chroma(new_chroma);
    let argb: Argb = hct.into();
    material_argb_to_hex(argb)
}

fn build_dynamic_colors_map(
    scheme: &DynamicScheme,
    chroma_mult: f64,
    tone_mult: f64,
    is_dark: bool,
) -> BTreeMap<String, String> {
    let roles: Vec<(&str, DynamicColor, bool)> = vec![
        ("background", MaterialDynamicColors::background(), true),
        ("onBackground", MaterialDynamicColors::on_background(), false),
        ("surface", MaterialDynamicColors::surface(), true),
        ("surfaceDim", MaterialDynamicColors::surface_dim(), true),
        ("surfaceBright", MaterialDynamicColors::surface_bright(), true),
        (
            "surfaceContainerLowest",
            MaterialDynamicColors::surface_container_lowest(),
            true,
        ),
        (
            "surfaceContainerLow",
            MaterialDynamicColors::surface_container_low(),
            true,
        ),
        (
            "surfaceContainer",
            MaterialDynamicColors::surface_container(),
            true,
        ),
        (
            "surfaceContainerHigh",
            MaterialDynamicColors::surface_container_high(),
            true,
        ),
        (
            "surfaceContainerHighest",
            MaterialDynamicColors::surface_container_highest(),
            true,
        ),
        ("onSurface", MaterialDynamicColors::on_surface(), false),
        ("surfaceVariant", MaterialDynamicColors::surface_variant(), true),
        (
            "onSurfaceVariant",
            MaterialDynamicColors::on_surface_variant(),
            false,
        ),
        ("inverseSurface", MaterialDynamicColors::inverse_surface(), false),
        (
            "inverseOnSurface",
            MaterialDynamicColors::inverse_on_surface(),
            false,
        ),
        ("outline", MaterialDynamicColors::outline(), false),
        (
            "outlineVariant",
            MaterialDynamicColors::outline_variant(),
            false,
        ),
        ("shadow", MaterialDynamicColors::shadow(), false),
        ("scrim", MaterialDynamicColors::scrim(), false),
        ("surfaceTint", MaterialDynamicColors::surface_tint(), true),
        ("primary", MaterialDynamicColors::primary(), true),
        ("onPrimary", MaterialDynamicColors::on_primary(), false),
        (
            "primaryContainer",
            MaterialDynamicColors::primary_container(),
            true,
        ),
        (
            "onPrimaryContainer",
            MaterialDynamicColors::on_primary_container(),
            false,
        ),
        ("inversePrimary", MaterialDynamicColors::inverse_primary(), false),
        ("secondary", MaterialDynamicColors::secondary(), true),
        ("onSecondary", MaterialDynamicColors::on_secondary(), false),
        (
            "secondaryContainer",
            MaterialDynamicColors::secondary_container(),
            true,
        ),
        (
            "onSecondaryContainer",
            MaterialDynamicColors::on_secondary_container(),
            false,
        ),
        ("tertiary", MaterialDynamicColors::tertiary(), true),
        ("onTertiary", MaterialDynamicColors::on_tertiary(), false),
        (
            "tertiaryContainer",
            MaterialDynamicColors::tertiary_container(),
            true,
        ),
        (
            "onTertiaryContainer",
            MaterialDynamicColors::on_tertiary_container(),
            false,
        ),
        ("error", MaterialDynamicColors::error(), true),
        ("onError", MaterialDynamicColors::on_error(), false),
        (
            "errorContainer",
            MaterialDynamicColors::error_container(),
            true,
        ),
        (
            "onErrorContainer",
            MaterialDynamicColors::on_error_container(),
            false,
        ),
        ("primaryFixed", MaterialDynamicColors::primary_fixed(), true),
        (
            "primaryFixedDim",
            MaterialDynamicColors::primary_fixed_dim(),
            true,
        ),
        ("onPrimaryFixed", MaterialDynamicColors::on_primary_fixed(), false),
        (
            "onPrimaryFixedVariant",
            MaterialDynamicColors::on_primary_fixed_variant(),
            false,
        ),
        ("secondaryFixed", MaterialDynamicColors::secondary_fixed(), true),
        (
            "secondaryFixedDim",
            MaterialDynamicColors::secondary_fixed_dim(),
            true,
        ),
        (
            "onSecondaryFixed",
            MaterialDynamicColors::on_secondary_fixed(),
            false,
        ),
        (
            "onSecondaryFixedVariant",
            MaterialDynamicColors::on_secondary_fixed_variant(),
            false,
        ),
        ("tertiaryFixed", MaterialDynamicColors::tertiary_fixed(), true),
        (
            "tertiaryFixedDim",
            MaterialDynamicColors::tertiary_fixed_dim(),
            true,
        ),
        (
            "onTertiaryFixed",
            MaterialDynamicColors::on_tertiary_fixed(),
            false,
        ),
        (
            "onTertiaryFixedVariant",
            MaterialDynamicColors::on_tertiary_fixed_variant(),
            false,
        ),
        (
            "primaryPaletteKeyColor",
            MaterialDynamicColors::primary_palette_key_color(),
            false,
        ),
        (
            "secondaryPaletteKeyColor",
            MaterialDynamicColors::secondary_palette_key_color(),
            false,
        ),
        (
            "tertiaryPaletteKeyColor",
            MaterialDynamicColors::tertiary_palette_key_color(),
            false,
        ),
        (
            "neutralPaletteKeyColor",
            MaterialDynamicColors::neutral_palette_key_color(),
            false,
        ),
        (
            "neutralVariantPaletteKeyColor",
            MaterialDynamicColors::neutral_variant_palette_key_color(),
            false,
        ),
    ];

    let mut out = BTreeMap::new();
    for (name, dc, is_bg) in roles {
        out.insert(
            name.to_string(),
            resolve_role_hex(dc, is_bg, scheme, chroma_mult, tone_mult, is_dark),
        );
    }

    // Include 2025 spec dim/key roles referenced by ColorsPreview.qml
    let dim_offset = if is_dark { -6.0 } else { -6.0 };
    out.insert(
        "primaryDim".to_string(),
        resolve_dim_role_hex(
            MaterialDynamicColors::primary(),
            dim_offset,
            true,
            scheme,
            chroma_mult,
            tone_mult,
            is_dark,
        ),
    );
    out.insert(
        "secondaryDim".to_string(),
        resolve_dim_role_hex(
            MaterialDynamicColors::secondary(),
            dim_offset,
            true,
            scheme,
            chroma_mult,
            tone_mult,
            is_dark,
        ),
    );
    out.insert(
        "tertiaryDim".to_string(),
        resolve_dim_role_hex(
            MaterialDynamicColors::tertiary(),
            dim_offset,
            true,
            scheme,
            chroma_mult,
            tone_mult,
            is_dark,
        ),
    );
    out.insert(
        "errorDim".to_string(),
        resolve_dim_role_hex(
            MaterialDynamicColors::error(),
            dim_offset,
            true,
            scheme,
            chroma_mult,
            tone_mult,
            is_dark,
        ),
    );
    let error_key_argb = scheme.error_palette.key_color().into();
    out.insert(
        "errorPaletteKeyColor".to_string(),
        material_argb_to_hex(error_key_argb),
    );

    out
}

/// Generate static harmonized role quartet (`color`, `onColor`, `container`, `onContainer`)
/// matching `static_color` in `color_utils.py`.
fn static_color_roles(scheme: &DynamicScheme, design_argb: u32, blend: bool) -> HarmonizedRoleSet {
    let mut argb = Argb::from_u32(design_argb);
    if blend {
        argb = harmonize(argb, scheme.source_color_argb);
    }
    let mut modified_scheme = scheme.clone();
    let hct: Hct = argb.into();
    modified_scheme.error_palette = TonalPalette::from_hct(hct);

    let color = material_argb_to_hex(MaterialDynamicColors::error().get_argb(&modified_scheme));
    let on_color =
        material_argb_to_hex(MaterialDynamicColors::on_error().get_argb(&modified_scheme));
    let container =
        material_argb_to_hex(MaterialDynamicColors::error_container().get_argb(&modified_scheme));
    let on_container = material_argb_to_hex(
        MaterialDynamicColors::on_error_container().get_argb(&modified_scheme),
    );

    HarmonizedRoleSet {
        color,
        on_color,
        container,
        on_container,
    }
}

fn apply_pywal_multipliers(hex_color: &str, sat_mult: f64, bright_mult: f64) -> String {
    let mut out = hex_color.to_string();
    if (sat_mult - 1.0).abs() > f64::EPSILON {
        out = multiply_saturation(&out, sat_mult);
    }
    if (bright_mult - 1.0).abs() > f64::EPSILON {
        out = multiply_lightness(&out, bright_mult);
    }
    out
}

/// Build the 24-color Pywal/Konsole dark and light palettes matching `ThemeConfig` in `schemeconfigs.py`.
#[allow(clippy::too_many_arguments)]
fn build_pywal_palettes(
    colors_best: &[String],
    custom_colors_list: &[String],
    tones: &TonalPalettes,
    colors_light: &BTreeMap<String, String>,
    colors_dark: &BTreeMap<String, String>,
    wallpaper_source: &str,
    light_sat_mult: f64,
    dark_sat_mult: f64,
    light_bright_mult: f64,
    dark_bright_mult: f64,
) -> PywalPalettes {
    let use_custom = !custom_colors_list.is_empty();
    let active_best: &[String] = if use_custom {
        custom_colors_list
    } else {
        colors_best
    };
    let best_colors_count = active_best.len();

    // --- DARK PYWAL PALETTE ---
    let dark_surface = colors_dark
        .get("surface")
        .cloned()
        .unwrap_or_else(|| "#111111".to_string());
    let mut pywal_dark_raw: Vec<String> = vec![dark_surface.clone()];
    let mut pywal_dark_intense: Vec<String> =
        vec![oklab_blend(&dark_surface, &tones.secondary[90], 0.8)];
    let mut pywal_dark_faint: Vec<String> =
        vec![oklab_blend(&dark_surface, &tones.secondary[90], 0.7)];

    let mut tone: usize = 50;
    for x in 0..7 {
        if pywal_dark_raw.len() <= 7 {
            if x < best_colors_count {
                let c = lighten_color(&active_best[x], 0.2, &tones.neutral[99]);
                pywal_dark_raw.push(blend_to_contrast(
                    &c,
                    &pywal_dark_raw[0],
                    &tones.neutral[99],
                    2.5,
                    0.01,
                    true,
                ));
            } else {
                if pywal_dark_raw.len() <= 7 {
                    let c = lighten_color(&tones.primary[tone], 0.2, &tones.neutral[99]);
                    pywal_dark_raw.push(blend_to_contrast(
                        &c,
                        &pywal_dark_raw[0],
                        &tones.neutral[99],
                        2.5,
                        0.01,
                        true,
                    ));
                }
                if pywal_dark_raw.len() <= 7 {
                    let c = lighten_color(&tones.tertiary[tone], 0.2, &tones.neutral[99]);
                    pywal_dark_raw.push(blend_to_contrast(
                        &c,
                        &pywal_dark_raw[0],
                        &tones.neutral[99],
                        2.5,
                        0.01,
                        true,
                    ));
                }
                if tone < 91 {
                    tone += 8;
                }
            }
        } else {
            break;
        }
    }

    let dark_tail: Vec<String> = pywal_dark_raw
        .iter()
        .skip(pywal_dark_raw.len().saturating_sub(7))
        .cloned()
        .collect();
    let all_dark_colors = if !use_custom {
        sort_colors_luminance(&dark_tail, false)
    } else {
        dark_tail
    };

    let mut pywal_colors_dark = vec![pywal_dark_raw[0].clone()];
    for c_base in all_dark_colors.iter().take(7) {
        let c = apply_pywal_multipliers(c_base, dark_sat_mult, dark_bright_mult);
        pywal_colors_dark.push(oklab_blend(&tones.neutral[99], &c, 0.95));
        pywal_dark_intense.push(oklab_blend(&tones.neutral[99], &c, 0.82));
        pywal_dark_faint.push(oklab_blend(&pywal_colors_dark[0], &c, 0.7));
    }

    // --- LIGHT PYWAL PALETTE ---
    let light_surface = colors_light
        .get("surface")
        .cloned()
        .unwrap_or_else(|| "#fefefe".to_string());
    let mut pywal_light_raw: Vec<String> = vec![light_surface.clone()];
    let mut pywal_light_intense: Vec<String> =
        vec![oklab_blend(&light_surface, &tones.secondary[25], 0.8)];
    let mut pywal_light_faint: Vec<String> =
        vec![oklab_blend(&light_surface, &tones.secondary[25], 0.7)];

    tone = 50;
    for x in 0..7 {
        if pywal_light_raw.len() <= 7 {
            if x < best_colors_count {
                let c = scale_saturation(&active_best[x], 1.0);
                let c = lighten_color(&c, 0.2, &tones.neutral[99]);
                pywal_light_raw.push(blend_to_contrast(
                    &c,
                    &pywal_light_raw[0],
                    &tones.neutral[10],
                    2.0,
                    0.01,
                    false,
                ));
            } else {
                if pywal_light_raw.len() <= 7 {
                    let c = scale_saturation(&tones.primary[tone], 1.0);
                    pywal_light_raw.push(blend_to_contrast(
                        &c,
                        &pywal_light_raw[0],
                        &tones.neutral[10],
                        2.0,
                        0.01,
                        false,
                    ));
                }
                if pywal_light_raw.len() <= 7 {
                    let c = scale_saturation(&tones.tertiary[tone], 1.0);
                    pywal_light_raw.push(blend_to_contrast(
                        &c,
                        &pywal_light_raw[0],
                        &tones.neutral[10],
                        2.0,
                        0.01,
                        false,
                    ));
                }
                if tone < 91 {
                    tone += 8;
                }
            }
        } else {
            break;
        }
    }

    let light_tail: Vec<String> = pywal_light_raw
        .iter()
        .skip(pywal_light_raw.len().saturating_sub(7))
        .cloned()
        .collect();
    let all_light_colors = if !use_custom {
        sort_colors_luminance(&light_tail, false)
    } else {
        light_tail
    };

    let mut pywal_colors_light = vec![pywal_light_raw[0].clone()];
    for c_base in all_light_colors.iter().take(7) {
        let c = apply_pywal_multipliers(c_base, light_sat_mult, light_bright_mult);
        pywal_colors_light.push(oklab_blend(&tones.neutral[1], &c, 0.95));
        pywal_light_intense.push(oklab_blend(&tones.neutral[1], &c, 0.82));
        pywal_light_faint.push(oklab_blend(&pywal_colors_light[0], &c, 0.7));
    }

    let mut light_colors_map = BTreeMap::new();
    let mut dark_colors_map = BTreeMap::new();
    for i in 0..8 {
        light_colors_map.insert(format!("color{i}"), pywal_colors_light[i].clone());
        light_colors_map.insert(format!("color{}", i + 8), pywal_light_intense[i].clone());
        light_colors_map.insert(format!("color{}", i + 16), pywal_light_faint[i].clone());

        dark_colors_map.insert(format!("color{i}"), pywal_colors_dark[i].clone());
        dark_colors_map.insert(format!("color{}", i + 8), pywal_dark_intense[i].clone());
        dark_colors_map.insert(format!("color{}", i + 16), pywal_dark_faint[i].clone());
    }

    let light_primary = colors_light
        .get("primary")
        .cloned()
        .unwrap_or_else(|| "#4285f4".to_string());
    let dark_primary = colors_dark
        .get("primary")
        .cloned()
        .unwrap_or_else(|| "#4285f4".to_string());
    let dark_on_surface = colors_dark
        .get("onSurface")
        .cloned()
        .unwrap_or_else(|| "#ffffff".to_string());

    let wal_light = PywalScheme {
        wallpaper: wallpaper_source.to_string(),
        checksum: String::new(),
        alpha: "100".to_string(),
        special: PywalSpecial {
            background: pywal_colors_light[0].clone(),
            background_intense: oklab_blend(&tones.neutral[8], &light_primary, 0.0),
            background_faint: oklab_blend(&tones.neutral[8], &light_primary, 0.35),
            foreground: oklab_blend(&pywal_colors_light[0], &tones.secondary[25], 0.98),
            foreground_intense: tones.secondary[25].clone(),
            foreground_faint: oklab_blend(&pywal_colors_light[0], &tones.secondary[25], 0.88),
            cursor: dark_on_surface.clone(),
        },
        colors: light_colors_map,
    };

    let wal_dark = PywalScheme {
        wallpaper: wallpaper_source.to_string(),
        checksum: String::new(),
        alpha: "100".to_string(),
        special: PywalSpecial {
            background: pywal_colors_dark[0].clone(),
            background_intense: oklab_blend(&tones.neutral[8], &dark_primary, 0.0),
            background_faint: oklab_blend(&tones.neutral[8], &dark_primary, 0.35),
            foreground: oklab_blend(&pywal_colors_dark[0], &tones.secondary[90], 0.98),
            foreground_intense: tones.secondary[90].clone(),
            foreground_faint: oklab_blend(&pywal_colors_dark[0], &tones.secondary[90], 0.88),
            cursor: dark_on_surface,
        },
        colors: dark_colors_map,
    };

    PywalPalettes {
        light: wal_light,
        dark: wal_dark,
    }
}

impl MaterialYouScheme {
    /// Generate a complete Material You scheme from a `SchemeRequest`.
    pub fn generate(req: &SchemeRequest) -> Result<Self> {
        let (best_list, wallpaper_info) = if let Some(ref hex) = req.source_color_hex {
            let argb = hex_to_argb(hex)?;
            let normalized_hex = argb_to_hex(argb);
            (
                vec![normalized_hex.clone()],
                WallpaperInfo {
                    source_type: "color".to_string(),
                    data: normalized_hex,
                },
            )
        } else if let Some(ref path) = req.image_path {
            let extracted = extract_seed_colors_from_image(path)?;
            (
                extracted,
                WallpaperInfo {
                    source_type: "image".to_string(),
                    data: path.to_string_lossy().into_owned(),
                },
            )
        } else {
            let fallback = "#4285f4".to_string();
            (
                vec![fallback.clone()],
                WallpaperInfo {
                    source_type: "color".to_string(),
                    data: fallback,
                },
            )
        };

        let mut best_map = BTreeMap::new();
        for (idx, c) in best_list.iter().enumerate() {
            best_map.insert(idx.to_string(), c.clone());
        }

        let seed_index = if req.ncolor < best_list.len() {
            req.ncolor
        } else {
            0
        };
        let seed_hex = best_list
            .get(seed_index)
            .cloned()
            .unwrap_or_else(|| "#4285f4".to_string());
        let seed_argb_u32 = hex_to_argb(&seed_hex).unwrap_or(0xFF4285F4);
        let seed_argb = Argb::from_u32(seed_argb_u32);

        let mat_variant = req.variant.to_material_variant();
        let contrast = Some(req.contrast_level.clamp(-1.0, 1.0));

        let scheme_light = DynamicScheme::by_variant(seed_argb, &mat_variant, false, contrast);
        let scheme_dark = DynamicScheme::by_variant(seed_argb, &mat_variant, true, contrast);

        let colors_light = build_dynamic_colors_map(
            &scheme_light,
            req.chroma_multiplier,
            req.tone_multiplier,
            false,
        );
        let colors_dark = build_dynamic_colors_map(
            &scheme_dark,
            req.chroma_multiplier,
            req.tone_multiplier,
            true,
        );

        let tones = TonalPalettes {
            primary: palette_to_hex_vec(&scheme_light.primary_palette),
            secondary: palette_to_hex_vec(&scheme_light.secondary_palette),
            tertiary: palette_to_hex_vec(&scheme_light.tertiary_palette),
            neutral: palette_to_hex_vec(&scheme_light.neutral_palette),
            neutral_variant: palette_to_hex_vec(&scheme_light.neutral_variant_palette),
            error: palette_to_hex_vec(&scheme_light.error_palette),
        };

        // Base text states taken from Breeze Color Scheme (matching m3_scheme_utils.py)
        let base_text_states: [(&str, u32, &str); 5] = [
            ("link", 4280910009, "#2980b9"),
            ("visited", 4288371126, "#9b59b6"),
            ("negative", 4292494419, "#da4453"),
            ("neutral", 4294341632, "#f67400"),
            ("positive", 4280790624, "#27ae60"),
        ];

        let mut custom = BTreeMap::new();
        for (name, val_u32, hex_val) in base_text_states {
            custom.insert(
                name.to_string(),
                CustomRoleEntry {
                    source: seed_hex.clone(),
                    value: hex_val.to_string(),
                    light: static_color_roles(&scheme_light, val_u32, true),
                    dark: static_color_roles(&scheme_dark, val_u32, true),
                },
            );
        }

        let pywal = build_pywal_palettes(
            &best_list,
            &req.custom_colors_list,
            &tones,
            &colors_light,
            &colors_dark,
            &wallpaper_info.data,
            req.light_saturation_multiplier,
            req.dark_saturation_multiplier,
            req.light_brightness_multiplier,
            req.dark_brightness_multiplier,
        );

        Ok(Self {
            best: best_map,
            best_list,
            seed: SeedColor {
                index: seed_index,
                color: seed_hex,
            },
            schemes: SchemesPair {
                light: colors_light,
                dark: colors_dark,
            },
            tones,
            custom,
            pywal,
            wallpaper: wallpaper_info,
            light: req.light,
            pywal_light: req.pywal_light,
            frame_contrast: req.frame_contrast,
        })
    }

    /// Serialize the generated scheme into the runtime JSON format expected by the QML plasmoid
    /// (`/tmp/kde-material-you-colors-$USER.json`).
    pub fn to_runtime_json(&self) -> Value {
        json!({
            "best": self.best_list,
            "seed": {
                "index": self.seed.index,
                "color": self.seed.color,
            },
            "schemes": {
                "light": self.schemes.light,
                "dark": self.schemes.dark,
            },
            "palettes": {
                "primary": self.tones.primary,
                "secondary": self.tones.secondary,
                "tertiary": self.tones.tertiary,
                "neutral": self.tones.neutral,
                "neutralVariant": self.tones.neutral_variant,
                "error": self.tones.error,
            },
            "tones": {
                "primary": self.tones.primary,
                "secondary": self.tones.secondary,
                "tertiary": self.tones.tertiary,
                "neutral": self.tones.neutral,
                "neutralVariant": self.tones.neutral_variant,
                "error": self.tones.error,
            },
            "custom": self.custom,
            "pywal": {
                "light": self.pywal.light,
                "dark": self.pywal.dark,
            },
            "wallpaper": {
                "type": self.wallpaper.source_type,
                "data": self.wallpaper.data,
            },
            "light": self.light,
        })
    }

    /// Write the formatted runtime JSON to `path` (e.g. `/tmp/kde-material-you-colors-$USER.json`).
    pub fn write_runtime_json(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create parent directory for runtime JSON at {}",
                    parent.display()
                )
            })?;
        }
        let val = self.to_runtime_json();
        let serialized = serde_json::to_string_pretty(&val)
            .context("failed to serialize runtime JSON")?;
        std::fs::write(path, serialized).with_context(|| {
            format!("failed to write runtime JSON to {}", path.display())
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use tempfile::tempdir;

    #[test]
    fn test_all_nine_scheme_variants() {
        for variant_idx in 0u8..=8u8 {
            let variant = SchemeVariant::from_u8(variant_idx);
            assert_eq!(variant.as_u8(), variant_idx);

            let req = SchemeRequest {
                source_color_hex: Some("#4285f4".to_string()),
                variant,
                chroma_multiplier: 1.2,
                tone_multiplier: 1.0,
                contrast_level: 0.0,
                ..Default::default()
            };

            let scheme = MaterialYouScheme::generate(&req)
                .unwrap_or_else(|e| panic!("failed to generate variant {variant_idx}: {e}"));

            assert_eq!(scheme.seed.index, 0);
            assert_eq!(scheme.seed.color, "#4285f4");
            assert_eq!(scheme.best.get("0").map(String::as_str), Some("#4285f4"));

            // Check essential Material 3 dynamic roles exist in both light and dark
            for role in [
                "primary",
                "onPrimary",
                "primaryContainer",
                "onPrimaryContainer",
                "secondary",
                "onSecondary",
                "secondaryContainer",
                "onSecondaryContainer",
                "tertiary",
                "onTertiary",
                "tertiaryContainer",
                "onTertiaryContainer",
                "error",
                "onError",
                "errorContainer",
                "onErrorContainer",
                "background",
                "onBackground",
                "surface",
                "onSurface",
                "surfaceVariant",
                "onSurfaceVariant",
                "surfaceDim",
                "surfaceBright",
                "surfaceContainerLowest",
                "surfaceContainerLow",
                "surfaceContainer",
                "surfaceContainerHigh",
                "surfaceContainerHighest",
                "outline",
                "outlineVariant",
                "shadow",
                "scrim",
                "inverseSurface",
                "inverseOnSurface",
                "inversePrimary",
                "primaryFixed",
                "primaryFixedDim",
                "onPrimaryFixed",
                "onPrimaryFixedVariant",
                "secondaryFixed",
                "secondaryFixedDim",
                "onSecondaryFixed",
                "onSecondaryFixedVariant",
                "tertiaryFixed",
                "tertiaryFixedDim",
                "onTertiaryFixed",
                "onTertiaryFixedVariant",
            ] {
                let l_val = scheme.schemes.light.get(role).unwrap_or_else(|| {
                    panic!("missing light role {role} in variant {variant_idx}")
                });
                let d_val = scheme.schemes.dark.get(role).unwrap_or_else(|| {
                    panic!("missing dark role {role} in variant {variant_idx}")
                });
                assert!(l_val.starts_with('#') && l_val.len() == 7);
                assert!(d_val.starts_with('#') && d_val.len() == 7);
            }

            // Check tonal palettes length (100 tones: 0..100)
            assert_eq!(scheme.tones.primary.len(), 100);
            assert_eq!(scheme.tones.secondary.len(), 100);
            assert_eq!(scheme.tones.tertiary.len(), 100);
            assert_eq!(scheme.tones.neutral.len(), 100);
            assert_eq!(scheme.tones.neutral_variant.len(), 100);
            assert_eq!(scheme.tones.error.len(), 100);

            // Check custom Breeze text state roles
            for state in ["link", "visited", "negative", "neutral", "positive"] {
                let entry = scheme
                    .custom
                    .get(state)
                    .unwrap_or_else(|| panic!("missing custom state {state}"));
                assert_eq!(entry.source, "#4285f4");
                assert!(entry.light.color.starts_with('#'));
                assert!(entry.light.on_color.starts_with('#'));
                assert!(entry.light.container.starts_with('#'));
                assert!(entry.light.on_container.starts_with('#'));
                assert!(entry.dark.color.starts_with('#'));
                assert!(entry.dark.on_color.starts_with('#'));
                assert!(entry.dark.container.starts_with('#'));
                assert!(entry.dark.on_container.starts_with('#'));
            }

            // Check 24-color Pywal palettes
            assert_eq!(scheme.pywal.light.colors.len(), 24);
            assert_eq!(scheme.pywal.dark.colors.len(), 24);
            for i in 0..24 {
                let key = format!("color{i}");
                assert!(scheme.pywal.light.colors.contains_key(&key));
                assert!(scheme.pywal.dark.colors.contains_key(&key));
            }
        }
    }

    #[test]
    fn test_image_seed_extraction_and_runtime_json_export() {
        let dir = tempdir().expect("failed to create tempdir");
        let img_path = dir.path().join("wallpaper.png");

        // Create a 256x128 synthetic wallpaper with dominant crimson/blue regions
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(256, 128, |x, y| {
            if x < 180 {
                Rgba([210, 35, 45, 255])
            } else if y < 64 {
                Rgba([35, 110, 220, 255])
            } else {
                Rgba([40, 180, 90, 255])
            }
        });
        img.save(&img_path).expect("failed to save test PNG");

        let seeds = extract_seed_colors_from_image(&img_path)
            .expect("failed to extract seed colors from PNG");
        assert!(!seeds.is_empty());
        assert!(seeds.len() <= 7);
        for s in &seeds {
            assert!(s.starts_with('#') && s.len() == 7);
        }

        let req = SchemeRequest {
            image_path: Some(img_path.clone()),
            ncolor: 0,
            variant: SchemeVariant::Vibrant,
            light: true,
            ..Default::default()
        };
        let scheme = MaterialYouScheme::generate(&req).expect("failed to generate scheme from PNG");
        assert_eq!(scheme.seed.color, seeds[0]);
        assert_eq!(scheme.wallpaper.source_type, "image");

        let json_path = dir.path().join("kde-material-you-colors-test.json");
        scheme
            .write_runtime_json(&json_path)
            .expect("failed to write runtime JSON");

        let raw = std::fs::read_to_string(&json_path).expect("failed to read runtime JSON");
        let parsed: Value = serde_json::from_str(&raw).expect("invalid JSON written");

        assert!(parsed["best"].is_array());
        assert_eq!(parsed["seed"]["index"], 0);
        assert_eq!(parsed["seed"]["color"], seeds[0]);
        assert!(parsed["schemes"]["light"]["primary"].is_string());
        assert!(parsed["schemes"]["dark"]["primary"].is_string());
        assert!(parsed["palettes"]["primary"].is_array());
        assert_eq!(parsed["palettes"]["primary"].as_array().unwrap().len(), 100);
        assert!(parsed["custom"]["link"]["light"]["color"].is_string());
        assert!(parsed["pywal"]["dark"]["colors"]["color0"].is_string());
        assert!(parsed["pywal"]["dark"]["colors"]["color23"].is_string());
        assert_eq!(parsed["light"], true);
    }
}
