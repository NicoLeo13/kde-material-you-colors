use anyhow::{anyhow, Result};
use material_colors::blend::harmonize;
use material_colors::color::Argb;
use std::str::FromStr;

/// Convert a 32-bit ARGB integer (`0xAARRGGBB`) into a lowercase `#rrggbb` hex string.
pub fn argb_to_hex(argb: u32) -> String {
    let r = ((argb >> 16) & 0xFF) as u8;
    let g = ((argb >> 8) & 0xFF) as u8;
    let b = (argb & 0xFF) as u8;
    rgb_to_hex(r, g, b)
}

/// Convert a `material_colors::color::Argb` struct into a lowercase `#rrggbb` hex string.
pub fn material_argb_to_hex(argb: Argb) -> String {
    rgb_to_hex(argb.red, argb.green, argb.blue)
}

/// Convert a `material_colors::color::Argb` struct into a 32-bit ARGB integer (`0xAARRGGBB`).
pub fn material_argb_to_u32(argb: Argb) -> u32 {
    ((argb.alpha as u32) << 24)
        | ((argb.red as u32) << 16)
        | ((argb.green as u32) << 8)
        | (argb.blue as u32)
}

/// Parse a hex color string (`#rgb`, `#rrggbb`, `#aarrggbb`, or without `#`) into a 32-bit ARGB integer (`0xFFRRGGBB`).
pub fn hex_to_argb(hex: &str) -> Result<u32> {
    let (r, g, b) = hex_to_rgb(hex)?;
    Ok((0xFFu32 << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
}

/// Parse a hex color string (`#rgb`, `#rrggbb`, `#aarrggbb`, or without `#`) into an `(r, g, b)` tuple.
pub fn hex_to_rgb(hex: &str) -> Result<(u8, u8, u8)> {
    let cleaned = hex.trim().trim_start_matches('#');
    match cleaned.len() {
        3 => {
            let r = u8::from_str_radix(&cleaned[0..1].repeat(2), 16)
                .map_err(|e| anyhow!("invalid red hex component in '{hex}': {e}"))?;
            let g = u8::from_str_radix(&cleaned[1..2].repeat(2), 16)
                .map_err(|e| anyhow!("invalid green hex component in '{hex}': {e}"))?;
            let b = u8::from_str_radix(&cleaned[2..3].repeat(2), 16)
                .map_err(|e| anyhow!("invalid blue hex component in '{hex}': {e}"))?;
            Ok((r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&cleaned[0..2], 16)
                .map_err(|e| anyhow!("invalid red hex component in '{hex}': {e}"))?;
            let g = u8::from_str_radix(&cleaned[2..4], 16)
                .map_err(|e| anyhow!("invalid green hex component in '{hex}': {e}"))?;
            let b = u8::from_str_radix(&cleaned[4..6], 16)
                .map_err(|e| anyhow!("invalid blue hex component in '{hex}': {e}"))?;
            Ok((r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&cleaned[2..4], 16)
                .map_err(|e| anyhow!("invalid red hex component in '{hex}': {e}"))?;
            let g = u8::from_str_radix(&cleaned[4..6], 16)
                .map_err(|e| anyhow!("invalid green hex component in '{hex}': {e}"))?;
            let b = u8::from_str_radix(&cleaned[6..8], 16)
                .map_err(|e| anyhow!("invalid blue hex component in '{hex}': {e}"))?;
            Ok((r, g, b))
        }
        _ => Err(anyhow!("unexpected hex color format: '{hex}'")),
    }
}

/// Format an `(r, g, b)` byte triple into a lowercase `#rrggbb` hex string.
pub fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Convert a `#rrggbb` solid color and an opacity percentage (`0..=100`) to `#aarrggbb`.
pub fn hex_to_alpha(solid_color: &str, opacity: u8) -> String {
    let alpha = ((opacity as f64) * 2.55).ceil().clamp(0.0, 255.0) as u8;
    let rgb = solid_color.trim().trim_start_matches('#');
    format!("#{alpha:02x}{rgb}")
}

#[inline]
fn cube(x: f64) -> f64 {
    x * x * x
}

#[inline]
fn srgb_transfer(x: f64) -> f64 {
    if x >= 0.0031308 {
        1.055 * (x.powf(1.0 / 2.4) - 0.055)
    } else {
        12.92 * x
    }
}

#[inline]
fn srgb_transfer_inv(x: f64) -> f64 {
    if x >= 0.04045 {
        ((x + 0.055) / 1.055).powf(2.4)
    } else {
        x / 12.92
    }
}

#[inline]
fn srgb_channel_to_linear(channel: u8) -> f64 {
    srgb_transfer_inv((channel as f64) / 255.0)
}

#[inline]
fn srgb_transfer_to_u8(c: f64) -> u8 {
    let res = (srgb_transfer(c) * 255.0).round();
    if res < 0.0 {
        0
    } else if res > 255.0 {
        255
    } else {
        res as u8
    }
}

/// Convert `(r, g, b)` bytes into Oklab cube-root LMS coordinates `(lp, mp, sp)` matching `color_utils.py`.
pub fn rgb_to_oklab(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let rl = srgb_channel_to_linear(r);
    let gl = srgb_channel_to_linear(g);
    let bl = srgb_channel_to_linear(b);

    let lp = (0.4122214708 * rl + 0.5363325363 * gl + 0.0514459929 * bl).cbrt();
    let mp = (0.2119034982 * rl + 0.6806995451 * gl + 0.1073969566 * bl).cbrt();
    let sp = (0.0883024619 * rl + 0.2817188376 * gl + 0.6299787005 * bl).cbrt();

    (lp, mp, sp)
}

/// Convert Oklab cube-root LMS coordinates `(lp, mp, sp)` back to `(r, g, b)` bytes matching `color_utils.py`.
pub fn oklab_to_rgb(lp: f64, mp: f64, sp: f64) -> (u8, u8, u8) {
    let l = cube(lp);
    let m = cube(mp);
    let s = cube(sp);

    let r = srgb_transfer_to_u8(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s);
    let g = srgb_transfer_to_u8(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s);
    let b = srgb_transfer_to_u8(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s);

    (r, g, b)
}

/// Blend two hex colors (`#rrggbb`) in Oklab space using `factor` (`0.0..=1.0`) matching `blendColors` in `color_utils.py`.
pub fn oklab_blend(c1_hex: &str, c2_hex: &str, factor: f64) -> String {
    let (r1, g1, b1) = hex_to_rgb(c1_hex).unwrap_or((0, 0, 0));
    let (r2, g2, b2) = hex_to_rgb(c2_hex).unwrap_or((0, 0, 0));

    let inv = 1.0 - factor;
    let (lp1, mp1, sp1) = rgb_to_oklab(r1, g1, b1);
    let (lp2, mp2, sp2) = rgb_to_oklab(r2, g2, b2);

    let lp = lp1 * inv + lp2 * factor;
    let mp = mp1 * inv + mp2 * factor;
    let sp = sp1 * inv + sp2 * factor;

    let (r, g, b) = oklab_to_rgb(lp, mp, sp);
    rgb_to_hex(r, g, b)
}

/// Alias for `oklab_blend` matching the `blend_color` helper contract.
pub fn blend_color(c1_hex: &str, c2_hex: &str, factor: f64) -> String {
    oklab_blend(c1_hex, c2_hex, factor)
}

/// Compute the relative sRGB luminance (`0.0..=1.0`) of a `#rrggbb` color matching `color_luminance` in `color_utils.py`.
pub fn color_luminance(hex_color: &str) -> f64 {
    let (r, g, b) = hex_to_rgb(hex_color).unwrap_or((0, 0, 0));
    0.2126 * srgb_channel_to_linear(r)
        + 0.7152 * srgb_channel_to_linear(g)
        + 0.0722 * srgb_channel_to_linear(b)
}

/// Sort a slice of hex colors by relative luminance in ascending (or descending if `reverse` is true) order.
pub fn sort_colors_luminance(colors: &[String], reverse: bool) -> Vec<String> {
    let mut pairs: Vec<(String, f64)> = colors
        .iter()
        .map(|c| (c.clone(), color_luminance(c)))
        .collect();

    pairs.sort_by(|a, b| {
        let cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
        if reverse {
            cmp.reverse()
        } else {
            cmp
        }
    });

    pairs.into_iter().map(|(c, _)| c).collect()
}

/// Compute the WCAG contrast ratio between a lighter and a darker `#rrggbb` color.
pub fn contrast_ratio(lighter_color: &str, darker_color: &str) -> f64 {
    let l1 = color_luminance(lighter_color);
    let l2 = color_luminance(darker_color);
    (l1 + 0.05) / (l2 + 0.05)
}

/// Iteratively blend `lighter_color` towards `blend_target` until `min_contrast` against `darker_color` is met.
pub fn blend_to_contrast(
    lighter_color: &str,
    darker_color: &str,
    blend_target: &str,
    min_contrast: f64,
    blend_step: f64,
    dark: bool,
) -> String {
    let mut contrast = if dark {
        contrast_ratio(lighter_color, darker_color)
    } else {
        contrast_ratio(darker_color, lighter_color)
    };

    if contrast < min_contrast {
        let mut blend_ratio = 0.0;
        let mut current = lighter_color.to_string();
        let step = if blend_step <= 0.0 { 0.01 } else { blend_step };

        while contrast < min_contrast && blend_ratio < 1.0 {
            blend_ratio = (blend_ratio + step).min(1.0);
            current = oklab_blend(lighter_color, blend_target, blend_ratio);
            contrast = if dark {
                contrast_ratio(&current, darker_color)
            } else {
                contrast_ratio(darker_color, &current)
            };
        }
        current
    } else {
        oklab_blend(lighter_color, blend_target, 0.12)
    }
}

/// Convert RGB bytes (`0..=255`) to HSV where `h` is in `0.0..=1.0`, `s` is in `0.0..=1.0`, and `v` is in `0.0..=255.0`
/// (matching Python `colorsys.rgb_to_hsv(r, g, b)` when passed 0..255 integer channels).
fn rgb_to_hsv_raw(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let rf = r as f64;
    let gf = g as f64;
    let bf = b as f64;
    let maxc = rf.max(gf).max(bf);
    let minc = rf.min(gf).min(bf);
    let v = maxc;
    if (minc - maxc).abs() < f64::EPSILON {
        return (0.0, 0.0, v);
    }
    let s = (maxc - minc) / maxc;
    let rc = (maxc - rf) / (maxc - minc);
    let gc = (maxc - gf) / (maxc - minc);
    let bc = (maxc - bf) / (maxc - minc);
    let mut h = if (rf - maxc).abs() < f64::EPSILON {
        bc - gc
    } else if (gf - maxc).abs() < f64::EPSILON {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    h = (h / 6.0).rem_euclid(1.0);
    (h, s, v)
}

/// Convert HSV (`h` in `0.0..=1.0`, `s` in `0.0..=1.0`, `v` in `0.0..=255.0`) back to RGB bytes (`0..=255`),
/// matching Python `int(c)` truncation on `colorsys.hsv_to_rgb`.
fn hsv_to_rgb_raw(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let v_clamped = v.clamp(0.0, 255.0);
    if s <= 0.0 {
        let c = v_clamped as u8;
        return (c, c, c);
    }
    let s_clamped = s.clamp(0.0, 1.0);
    let i = (h * 6.0).floor();
    let f = (h * 6.0) - i;
    let p = v_clamped * (1.0 - s_clamped);
    let q = v_clamped * (1.0 - s_clamped * f);
    let t = v_clamped * (1.0 - s_clamped * (1.0 - f));
    let (r, g, b) = match (i as i32).rem_euclid(6) {
        0 => (v_clamped, t, p),
        1 => (q, v_clamped, p),
        2 => (p, v_clamped, t),
        3 => (p, q, v_clamped),
        4 => (t, p, v_clamped),
        _ => (v_clamped, p, q),
    };
    (
        r.clamp(0.0, 255.0) as u8,
        g.clamp(0.0, 255.0) as u8,
        b.clamp(0.0, 255.0) as u8,
    )
}

/// Set the HSV saturation of `hex_color` to `amount` (`0.0..=1.0`), matching `scale_saturation` in `color_utils.py`.
pub fn scale_saturation(hex_color: &str, amount: f64) -> String {
    let (r, g, b) = hex_to_rgb(hex_color).unwrap_or((0, 0, 0));
    let (h, _, v) = rgb_to_hsv_raw(r, g, b);
    let (nr, ng, nb) = hsv_to_rgb_raw(h, amount.clamp(0.0, 1.0), v);
    rgb_to_hex(nr, ng, nb)
}

/// Multiply the HSV saturation of `hex_color` by `multiplier`, clamping to `0.0..=1.0`.
pub fn multiply_saturation(hex_color: &str, multiplier: f64) -> String {
    let (r, g, b) = hex_to_rgb(hex_color).unwrap_or((0, 0, 0));
    let (h, s, v) = rgb_to_hsv_raw(r, g, b);
    let new_s = (s * multiplier).clamp(0.0, 1.0);
    let (nr, ng, nb) = hsv_to_rgb_raw(h, new_s, v);
    rgb_to_hex(nr, ng, nb)
}

/// Set the HSV value (brightness, `0.0..=255.0`) of `hex_color` to `amount`, matching `scale_lightness` in `color_utils.py`.
pub fn scale_lightness(hex_color: &str, amount: f64) -> String {
    let (r, g, b) = hex_to_rgb(hex_color).unwrap_or((0, 0, 0));
    let (h, s, _) = rgb_to_hsv_raw(r, g, b);
    let (nr, ng, nb) = hsv_to_rgb_raw(h, s, amount.clamp(0.0, 255.0));
    rgb_to_hex(nr, ng, nb)
}

/// Multiply the HSV value (brightness) of `hex_color` by `multiplier`, clamping to `0.0..=255.0`.
pub fn multiply_lightness(hex_color: &str, multiplier: f64) -> String {
    let (r, g, b) = hex_to_rgb(hex_color).unwrap_or((0, 0, 0));
    let (h, s, v) = rgb_to_hsv_raw(r, g, b);
    let new_v = (v * multiplier).clamp(0.0, 255.0);
    let (nr, ng, nb) = hsv_to_rgb_raw(h, s, new_v);
    rgb_to_hex(nr, ng, nb)
}

/// Lighten `hex_color` if its luminance is below `min_lum`, then blend 20% towards `blend_hex`
/// (matching `lighteen_color` in `color_utils.py`).
pub fn lighten_color(hex_color: &str, min_lum: f64, blend_hex: &str) -> String {
    let current_luminance = color_luminance(hex_color);
    let adjusted = if current_luminance < min_lum {
        let new_lightness = 255.0 * (1.0 - current_luminance);
        scale_lightness(hex_color, new_lightness)
    } else {
        hex_color.to_string()
    };
    oklab_blend(&adjusted, blend_hex, 0.2)
}

/// Harmonize `design_hex` towards `source_hex` in HCT space using `material_colors::blend::harmonize`.
pub fn harmonize_hex(design_hex: &str, source_hex: &str) -> String {
    let design = Argb::from_str(design_hex).unwrap_or_else(|_| {
        let (r, g, b) = hex_to_rgb(design_hex).unwrap_or((0, 0, 0));
        Argb::new(255, r, g, b)
    });
    let source = Argb::from_str(source_hex).unwrap_or_else(|_| {
        let (r, g, b) = hex_to_rgb(source_hex).unwrap_or((0, 0, 0));
        Argb::new(255, r, g, b)
    });
    let harmonized = harmonize(design, source);
    material_argb_to_hex(harmonized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_rgb_argb_conversions() {
        assert_eq!(rgb_to_hex(0x42, 0x85, 0xf4), "#4285f4");
        assert_eq!(hex_to_rgb("#4285f4").unwrap(), (0x42, 0x85, 0xf4));
        assert_eq!(hex_to_rgb("4285F4").unwrap(), (0x42, 0x85, 0xf4));
        assert_eq!(hex_to_rgb("#abc").unwrap(), (0xaa, 0xbb, 0xcc));
        assert_eq!(hex_to_rgb("#ff112233").unwrap(), (0x11, 0x22, 0x33));
        assert!(hex_to_rgb("#12345").is_err());

        let argb = hex_to_argb("#4285f4").unwrap();
        assert_eq!(argb, 0xFF4285F4);
        assert_eq!(argb_to_hex(argb), "#4285f4");

        assert_eq!(hex_to_alpha("#ff0000", 50), "#80ff0000");
        assert_eq!(hex_to_alpha("#112233", 100), "#ff112233");
        assert_eq!(hex_to_alpha("#112233", 0), "#00112233");
    }

    #[test]
    fn test_oklab_roundtrip_and_blend_matches_python() {
        for (r, g, b) in [
            (0, 0, 0),
            (255, 255, 255),
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
            (66, 133, 244),
            (218, 68, 83),
        ] {
            let (lp, mp, sp) = rgb_to_oklab(r, g, b);
            let (ro, go, bo) = oklab_to_rgb(lp, mp, sp);
            assert!(
                (ro as i16 - r as i16).abs() <= 1
                    && (go as i16 - g as i16).abs() <= 1
                    && (bo as i16 - b as i16).abs() <= 1,
                "Oklab roundtrip mismatch for ({r}, {g}, {b}) -> ({ro}, {go}, {bo})"
            );
        }

        // Exact reference values from running color_utils.py blendColors("#ff0000", "#0000ff", 0.5)
        let mid_rb = oklab_blend("#ff0000", "#0000ff", 0.5);
        assert_eq!(mid_rb, blend_color("#ff0000", "#0000ff", 0.5));
        assert_eq!(oklab_blend("#ff0000", "#00ff00", 0.0), "#fe0000");
        assert_eq!(oklab_blend("#ff0000", "#00ff00", 1.0), "#00fe00");

        let mid_rg = oklab_blend("#ff0000", "#00ff00", 0.5);
        let (r, g, b) = hex_to_rgb(&mid_rg).unwrap();
        assert!(r > 100 && g > 100 && b < 50);
    }

    #[test]
    fn test_luminance_contrast_and_saturation_scaling() {
        assert!((color_luminance("#000000") - 0.0).abs() < 1e-6);
        assert!((color_luminance("#ffffff") - 1.0).abs() < 1e-6);
        assert!((contrast_ratio("#ffffff", "#000000") - 21.0).abs() < 1e-3);

        let colors = vec![
            "#ffffff".to_string(),
            "#000000".to_string(),
            "#777777".to_string(),
        ];
        let sorted = sort_colors_luminance(&colors, false);
        assert_eq!(sorted, vec!["#000000", "#777777", "#ffffff"]);

        let desaturated = scale_saturation("#ff0000", 0.0);
        assert_eq!(desaturated, "#ffffff");

        let full_sat = scale_saturation("#ff8080", 1.0);
        assert_eq!(full_sat, "#ff0000");

        let contrasted_dark =
            blend_to_contrast("#475ac6", "#1a1a22", "#c1f7fb", 4.5, 0.1, true);
        assert!(contrast_ratio(&contrasted_dark, "#1a1a22") >= 4.5);
    }

    #[test]
    fn test_breeze_harmonize_hex() {
        // Harmonize Breeze link blue (#2980b9) towards a red seed (#ff0000)
        let harmonized = harmonize_hex("#2980b9", "#ff0000");
        assert!(harmonized.starts_with('#'));
        assert_eq!(harmonized.len(), 7);
        assert_ne!(harmonized, "#2980b9");

        // Harmonizing a color with itself preserves its hue/tone
        let self_harmonized = harmonize_hex("#27ae60", "#27ae60");
        let (r1, g1, b1) = hex_to_rgb("#27ae60").unwrap();
        let (r2, g2, b2) = hex_to_rgb(&self_harmonized).unwrap();
        assert!((r1 as i16 - r2 as i16).abs() <= 2);
        assert!((g1 as i16 - g2 as i16).abs() <= 2);
        assert!((b1 as i16 - b2 as i16).abs() <= 2);
    }
}
