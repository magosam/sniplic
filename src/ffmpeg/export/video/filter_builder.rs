use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::project::{Clip, MediaItem};
use crate::ffmpeg::export::keyframes::{
    build_keyframe_expr, generate_shutter_weights, get_track_max_abs, is_track_dynamic,
};
use crate::ffmpeg::export::subtitles::{escape_ffmpeg_filter_path, generate_ass_subtitles_file};
use crate::ffmpeg::export::types::ExportSettings;

use super::transitions::TransitionConfig;

/// Structure for decomposition and normalization of CSS Filter parameters
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCssFilter {
    pub contrast: f64,
    pub brightness: f64,
    pub saturate: f64,
    pub sepia: f64,
    pub hue_rotate_deg: f64,
    pub grayscale: f64,
    pub blur_px: f64,
    pub invert: f64,
}

impl Default for ParsedCssFilter {
    fn default() -> Self {
        Self {
            contrast: 1.0,
            brightness: 1.0,
            saturate: 1.0,
            sepia: 0.0,
            hue_rotate_deg: 0.0,
            grayscale: 0.0,
            blur_px: 0.0,
            invert: 0.0,
        }
    }
}

/// Robustly parses any sequence of CSS filter functions
/// Example: "contrast(1.250) saturate(1.100) brightness(1.020) sepia(0.200) hue-rotate(-15.0deg)"
pub fn parse_css_filter(css: &str) -> ParsedCssFilter {
    let mut parsed = ParsedCssFilter::default();
    let mut remaining = css;

    while let Some(open_paren) = remaining.find('(') {
        let prefix = &remaining[..open_paren];
        let func_name = prefix.split_whitespace().last().unwrap_or("");
        let after_open = &remaining[open_paren + 1..];

        if let Some(close_paren) = after_open.find(')') {
            let arg_str = after_open[..close_paren].trim();
            remaining = &after_open[close_paren + 1..];

            let parse_num = |s: &str| -> Option<f64> {
                let trimmed = s.trim();
                if let Some(deg) = trimmed.strip_suffix("deg") {
                    deg.trim().parse::<f64>().ok()
                } else if let Some(rad) = trimmed.strip_suffix("rad") {
                    rad.trim().parse::<f64>().ok().map(|r| r * 180.0 / std::f64::consts::PI)
                } else if let Some(turn) = trimmed.strip_suffix("turn") {
                    turn.trim().parse::<f64>().ok().map(|t| t * 360.0)
                } else if let Some(px) = trimmed.strip_suffix("px") {
                    px.trim().parse::<f64>().ok()
                } else if let Some(pct) = trimmed.strip_suffix('%') {
                    pct.trim().parse::<f64>().ok().map(|p| p / 100.0)
                } else {
                    trimmed.parse::<f64>().ok()
                }
            };

            match func_name.to_lowercase().as_str() {
                "contrast" => {
                    if let Some(val) = parse_num(arg_str) {
                        parsed.contrast = val;
                    }
                }
                "brightness" => {
                    if let Some(val) = parse_num(arg_str) {
                        parsed.brightness = val;
                    }
                }
                "saturate" => {
                    if let Some(val) = parse_num(arg_str) {
                        parsed.saturate = val;
                    }
                }
                "sepia" => {
                    if let Some(val) = parse_num(arg_str) {
                        parsed.sepia = val;
                    }
                }
                "hue-rotate" | "huerotate" => {
                    if let Some(val) = parse_num(arg_str) {
                        parsed.hue_rotate_deg = val;
                    }
                }
                "grayscale" => {
                    if let Some(val) = parse_num(arg_str) {
                        parsed.grayscale = val;
                    }
                }
                "blur" => {
                    if let Some(val) = parse_num(arg_str) {
                        parsed.blur_px = val;
                    }
                }
                "invert" => {
                    if let Some(val) = parse_num(arg_str) {
                        parsed.invert = val;
                    }
                }
                _ => {}
            }
        } else {
            break;
        }
    }

    parsed
}

/// Parses CSS clip-path inset(top right bottom left) expressions into FFmpeg crop coordinates
pub fn parse_clip_path_inset(clip_path: &str, w: f64, h: f64) -> Option<(f64, f64, f64, f64)> {
    let trimmed = clip_path.trim();
    if !trimmed.starts_with("inset(") || !trimmed.ends_with(')') {
        return None;
    }
    let inner = &trimmed[6..trimmed.len() - 1].trim();
    let parts: Vec<&str> = inner.split_whitespace().collect();
    if parts.len() != 4 {
        return None;
    }
    let parse_pct = |s: &str| -> Option<f64> {
        s.trim_end_matches('%').trim().parse::<f64>().ok()
    };
    let top_pct = parse_pct(parts[0])?;
    let right_pct = parse_pct(parts[1])?;
    let bottom_pct = parse_pct(parts[2])?;
    let left_pct = parse_pct(parts[3])?;

    let left_px = (left_pct / 100.0) * w;
    let right_px = (right_pct / 100.0) * w;
    let top_px = (top_pct / 100.0) * h;
    let bottom_px = (bottom_pct / 100.0) * h;

    let crop_w = (w - left_px - right_px).max(2.0);
    let crop_h = (h - top_px - bottom_px).max(2.0);
    let crop_x = left_px;
    let crop_y = top_px;

    Some((crop_w, crop_h, crop_x, crop_y))
}

/// Native fallback for CSS Filter equations for known Sniplic presets
pub fn get_builtin_filter_css(filter_id: &str, intensity: f64) -> String {
    let t = (intensity / 100.0).max(0.0).min(1.0);
    match filter_id {
        "4k" => format!(
            "contrast({:.3}) saturate({:.3}) brightness({:.3})",
            1.0 + 0.15 * t,
            1.0 + 0.10 * t,
            1.0 + 0.03 * t
        ),
        "enhance" => format!(
            "contrast({:.3}) saturate({:.3}) brightness({:.3})",
            1.0 + 0.10 * t,
            1.0 + 0.30 * t,
            1.0 + 0.04 * t
        ),
        "blueGrey" => format!(
            "contrast({:.3}) saturate({:.3}) hue-rotate({:.1}deg) sepia({:.3})",
            1.0 + 0.18 * t,
            (1.0 - 0.35 * t).max(0.0),
            -15.0 * t,
            0.15 * t
        ),
        "cinematics" => format!(
            "contrast({:.3}) saturate({:.3}) sepia({:.3}) hue-rotate({:.1}deg) brightness({:.3})",
            1.0 + 0.25 * t,
            1.0 + 0.20 * t,
            0.22 * t,
            -18.0 * t,
            1.0 - 0.02 * t
        ),
        "noir" | "blackwhite" | "grayscale" | "filmNoir1940" | "ilfordHp5" | "leicaMonochrome"
        | "kodakTriX400" | "highContrastStreet" => format!(
            "grayscale({:.3}) contrast({:.3}) brightness({:.3})",
            t,
            1.0 + 0.25 * t,
            1.0 - 0.03 * t
        ),
        "vintageSepia" | "sepia" | "vintage" | "vintage70s" => format!(
            "sepia({:.3}) contrast({:.3}) brightness({:.3})",
            0.75 * t,
            1.0 + 0.15 * t,
            1.0 - 0.02 * t
        ),
        "invert" => {
            if t > 0.5 {
                "invert(1)".to_string()
            } else {
                "".to_string()
            }
        }
        "bladeRunnerAmber" => format!(
            "contrast({:.3}) sepia({:.3}) saturate({:.3}) hue-rotate({:.1}deg)",
            1.0 + 0.35 * t,
            0.50 * t,
            1.0 + 0.40 * t,
            -10.0 * t
        ),
        "hollywoodTealOrange" => format!(
            "contrast({:.3}) saturate({:.3}) sepia({:.3}) hue-rotate({:.1}deg)",
            1.0 + 0.28 * t,
            1.0 + 0.25 * t,
            0.25 * t,
            -22.0 * t
        ),
        "coldNordic" => format!(
            "hue-rotate({:.1}deg) saturate({:.3}) contrast({:.3}) sepia({:.3})",
            15.0 * t,
            (1.0 - 0.20 * t).max(0.0),
            1.0 + 0.15 * t,
            0.10 * t
        ),
        "vibrantAnime" => format!(
            "contrast({:.3}) saturate({:.3}) brightness({:.3})",
            1.0 + 0.15 * t,
            1.0 + 0.40 * t,
            1.0 + 0.05 * t
        ),
        "duneDesert" => format!(
            "contrast({:.3}) sepia({:.3}) saturate({:.3}) brightness({:.3})",
            1.0 + 0.25 * t,
            0.40 * t,
            1.0 + 0.20 * t,
            1.0 + 0.03 * t
        ),
        "technicolor3Strip" => format!(
            "contrast({:.3}) saturate({:.3}) brightness({:.3})",
            1.0 + 0.30 * t,
            1.0 + 0.35 * t,
            1.0 + 0.02 * t
        ),
        _ => format!(
            "contrast({:.3}) saturate({:.3}) brightness({:.3})",
            1.0 + 0.10 * t,
            1.0 + 0.15 * t,
            1.0 + 0.02 * t
        ),
    }
}

/// Converts filter specifications (CSS Filter or preset ID) into native FFmpeg directives
pub fn build_filter_directives(
    filter_id: &str,
    intensity: f64,
    custom_css: Option<&str>,
    timeline_enable: Option<&str>,
) -> Vec<String> {
    let css = if let Some(c) = custom_css.filter(|s| !s.trim().is_empty()) {
        c.to_string()
    } else {
        get_builtin_filter_css(filter_id, intensity)
    };

    let parsed = parse_css_filter(&css);
    let enable_suffix = timeline_enable
        .map(|expr| format!(":enable='{}'", expr))
        .unwrap_or_default();

    let mut filters = Vec::new();

    let eff_saturate = (parsed.saturate * (1.0 - parsed.grayscale)).max(0.0);
    let eff_contrast = parsed.contrast.max(0.0).min(5.0);
    let eff_brightness = (parsed.brightness - 1.0).max(-1.0).min(1.0);

    if eff_brightness.abs() > 0.001
        || (eff_contrast - 1.0).abs() > 0.001
        || (eff_saturate - 1.0).abs() > 0.001
    {
        filters.push(format!(
            "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}{}",
            eff_brightness, eff_contrast, eff_saturate, enable_suffix
        ));
    }

    let norm_hue = parsed.hue_rotate_deg % 360.0;
    if norm_hue.abs() > 0.1 {
        filters.push(format!("hue=h={:.1}{}", norm_hue, enable_suffix));
    }

    if parsed.sepia > 0.01 {
        let s = parsed.sepia.min(1.0);
        let r1 = 1.0 - s + 0.393 * s;
        let r2 = 0.769 * s;
        let r3 = 0.189 * s;
        let g1 = 0.349 * s;
        let g2 = 1.0 - s + 0.686 * s;
        let g3 = 0.168 * s;
        let b1 = 0.272 * s;
        let b2 = 0.534 * s;
        let b3 = 1.0 - s + 0.131 * s;
        filters.push(format!(
            "colorchannelmixer={:.3}:{:.3}:{:.3}:0:{:.3}:{:.3}:{:.3}:0:{:.3}:{:.3}:{:.3}{}",
            r1, r2, r3, g1, g2, g3, b1, b2, b3, enable_suffix
        ));
    }

    if parsed.invert > 0.5 {
        if enable_suffix.is_empty() {
            filters.push("negate".to_string());
        } else {
            filters.push(format!("negate{}", enable_suffix));
        }
    }

    if parsed.blur_px > 0.1 {
        let sigma = (parsed.blur_px * 0.8).max(0.5);
        filters.push(format!("gblur=sigma={:.1}:steps=2{}", sigma, enable_suffix));
    }

    if filter_id == "4k" {
        let t = (intensity / 100.0).max(0.0).min(1.0);
        if t > 0.05 {
            filters.push(format!(
                "unsharp=5:5:{:.2}:5:5:0.0{}",
                0.8 * t,
                enable_suffix
            ));
        }
    }

    filters
}

/// Converts Sniplic visual effects (Blur, Glow, Vignette, Pixelate, ZoomPulse) into FFmpeg directives
pub fn build_effect_filters(
    effect_id: &str,
    config: &serde_json::Value,
    timeline_enable: Option<&str>,
    width: u32,
    height: u32,
    fps: f64,
    start_sec: f64,
    end_sec: f64,
) -> Vec<String> {
    let enable_suffix = timeline_enable
        .map(|expr| format!(":enable='{}'", expr))
        .unwrap_or_default();

    match effect_id {
        "blur" => {
            let desfoque = config.get("desfoque").and_then(|v| v.as_f64()).unwrap_or(15.0);
            let sigma = (desfoque * 0.8).max(0.5);
            vec![format!("gblur=sigma={:.1}:steps=2{}", sigma, enable_suffix)]
        }
        "glow" => {
            let brilho = config.get("brilho").and_then(|v| v.as_f64()).unwrap_or(50.0);
            let b_val = ((brilho / 100.0) * 0.25).max(0.0).min(0.5);
            vec![
                format!("eq=brightness={:.3}:contrast=1.120{}", b_val, enable_suffix),
                format!("unsharp=7:7:1.20:7:7:0.0{}", enable_suffix),
            ]
        }
        "vignette" => {
            let forca = config.get("forca").and_then(|v| v.as_f64()).unwrap_or(60.0);
            let angle = ((forca / 100.0) * (std::f64::consts::PI / 4.0)).max(0.1);
            vec![format!("vignette=angle='{:.3}'{}", angle, enable_suffix)]
        }
        "pixelate" => {
            let tamanho = config.get("tamanho").and_then(|v| v.as_u64()).unwrap_or(16).max(2).min(128);
            vec![format!("pixelize=w={}:h={}{}", tamanho, tamanho, enable_suffix)]
        }
        "zoomPulse" => {
            let velocidade = config.get("velocidade").and_then(|v| v.as_f64()).unwrap_or(5.0);
            let forca = config.get("forca").and_then(|v| v.as_f64()).unwrap_or(40.0) / 100.0;
            if timeline_enable.is_some() {
                vec![format!(
                    "zoompan=z='if(between(time,{start_sec:.4},{end_sec:.4}),1+max(0,sin(PI*{velocidade:.2}/2*(time-{start_sec:.4})))*{forca:.4}*0.08,1)':d=1:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':s={width}x{height}:fps={fps:.4}"
                )]
            } else {
                vec![format!(
                    "zoompan=z='1+max(0,sin(PI*{velocidade:.2}/2*time))*{forca:.4}*0.08':d=1:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':s={width}x{height}:fps={fps:.4}"
                )]
            }
        }
        "cameraShake" => {
            let forca = config.get("forca").and_then(|v| v.as_f64()).unwrap_or(50.0) / 100.0;
            let velocidade = config.get("velocidade").and_then(|v| v.as_f64()).unwrap_or(5.0);
            let amp_x = (forca * 24.0).max(2.0);
            let amp_y = (forca * 18.0).max(2.0);
            let freq = (velocidade * 3.5).max(1.0);
            if let Some(expr) = timeline_enable {
                vec![format!(
                    "crop=w='iw*0.92':h='ih*0.92':x='if({expr},(in_w-out_w)/2+(sin(t*{freq:.2}*3.1)+sin(t*{freq:.2}*5.3)*0.5)*{amp_x:.2},(in_w-out_w)/2)':y='if({expr},(in_h-out_h)/2+(cos(t*{freq:.2}*3.7)+cos(t*{freq:.2}*6.1)*0.5)*{amp_y:.2},(in_h-out_h)/2)',scale={width}:{height}"
                )]
            } else {
                vec![format!(
                    "crop=w='iw*0.92':h='ih*0.92':x='(in_w-out_w)/2+(sin(t*{freq:.2}*3.1)+sin(t*{freq:.2}*5.3)*0.5)*{amp_x:.2}':y='(in_h-out_h)/2+(cos(t*{freq:.2}*3.7)+cos(t*{freq:.2}*6.1)*0.5)*{amp_y:.2}',scale={width}:{height}"
                )]
            }
        }
        "gentleSway" => {
            let forca = config.get("forca").and_then(|v| v.as_f64()).unwrap_or(40.0) / 100.0;
            let velocidade = config.get("velocidade").and_then(|v| v.as_f64()).unwrap_or(5.0);
            let angle_rad = ((forca * 7.0).to_radians()).max(0.01);
            let speed = (velocidade * 0.8).max(0.5);
            if let Some(expr) = timeline_enable {
                vec![format!(
                    "rotate=a='if({expr},sin(t*{speed:.2})*{angle_rad:.4},0)':ow={width}:oh={height}:c=black"
                )]
            } else {
                vec![format!(
                    "rotate=a='sin(t*{speed:.2})*{angle_rad:.4}':ow={width}:oh={height}:c=black"
                )]
            }
        }
        "quickSpin" => {
            let velocidade = config.get("velocidade").and_then(|v| v.as_f64()).unwrap_or(5.0);
            let voltas = config.get("voltas").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let rot_speed = velocidade * 1.25 * voltas;
            if let Some(expr) = timeline_enable {
                vec![format!(
                    "rotate=a='if({expr},t*{rot_speed:.2}*2*PI,0)':ow={width}:oh={height}:c=black"
                )]
            } else {
                vec![format!(
                    "rotate=a='t*{rot_speed:.2}*2*PI':ow={width}:oh={height}:c=black"
                )]
            }
        }
        "verticalBounce" => {
            let forca = config.get("forca").and_then(|v| v.as_f64()).unwrap_or(50.0) / 100.0;
            let velocidade = config.get("velocidade").and_then(|v| v.as_f64()).unwrap_or(5.0);
            let amp = (forca * 30.0).max(2.0);
            let speed = (velocidade / 1.5).max(0.5);
            if let Some(expr) = timeline_enable {
                vec![format!(
                    "crop=w='iw':h='ih*0.92':x=0:y='if({expr},(in_h-out_h)/2-abs(sin(t*{speed:.2}*PI))*{amp:.2},(in_h-out_h)/2)',scale={width}:{height}"
                )]
            } else {
                vec![format!(
                    "crop=w='iw':h='ih*0.92':x=0:y='(in_h-out_h)/2-abs(sin(t*{speed:.2}*PI))*{amp:.2}',scale={width}:{height}"
                )]
            }
        }
        "digitalGlitch" => {
            let intensidade = config.get("intensidade").and_then(|v| v.as_f64()).unwrap_or(50.0) / 100.0;
            let shift = ((intensidade * 12.0).round() as i32).max(2);
            vec![
                format!("rgbashift=rh={shift}:bh=-{shift}:edge=smear{enable_suffix}"),
                format!("eq=contrast=1.15:brightness=0.04{enable_suffix}"),
            ]
        }
        "chromaticAberration" => {
            let desloc = config.get("deslocamento").and_then(|v| v.as_f64()).unwrap_or(8.0);
            let angulo_deg = config.get("angulo").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let rad = angulo_deg.to_radians();
            let rh = (desloc * rad.cos()).round() as i32;
            let rv = (desloc * rad.sin()).round() as i32;
            let bh = -rh;
            let bv = -rv;
            vec![format!("rgbashift=rh={rh}:rv={rv}:bh={bh}:bv={bv}:edge=smear{enable_suffix}")]
        }
        "strobeFlash" => {
            let freq = config.get("frequencia").and_then(|v| v.as_f64()).unwrap_or(4.0);
            let intensidade = config.get("intensidade").and_then(|v| v.as_f64()).unwrap_or(80.0) / 100.0;
            let flash_val = (intensidade * 0.75).max(0.1);
            if let Some(expr) = timeline_enable {
                vec![format!(
                    "eq=brightness='if({expr}*gt(sin(t*{freq:.2}*PI),0.6),{flash_val:.3},0)'"
                )]
            } else {
                vec![format!(
                    "eq=brightness='if(gt(sin(t*{freq:.2}*PI),0.6),{flash_val:.3},0)'"
                )]
            }
        }
        "lightLeak" => {
            let intensidade = config.get("intensidade").and_then(|v| v.as_f64()).unwrap_or(60.0) / 100.0;
            let rs = (intensidade * 0.28).min(0.5);
            let gs = (intensidade * 0.12).min(0.3);
            let bs = -(intensidade * 0.18).min(0.4);
            let br = (intensidade * 0.08).min(0.2);
            vec![
                format!("colorbalance=rs={rs:.3}:gs={gs:.3}:bs={bs:.3}{enable_suffix}"),
                format!("eq=brightness={br:.3}:contrast=1.06:saturation=1.18{enable_suffix}"),
            ]
        }
        "edgeGlow" => {
            let intensidade = config.get("intensidade").and_then(|v| v.as_f64()).unwrap_or(60.0) / 100.0;
            let raio = config.get("raio").and_then(|v| v.as_f64()).unwrap_or(5.0).max(1.0);
            let lsize = (raio.round() as i32 * 2 + 1).max(3).min(13);
            let amount = (1.0 + intensidade * 0.8).min(2.5);
            let sat = 1.0 + intensidade * 0.35;
            vec![
                format!("unsharp={lsize}:{lsize}:{amount:.2}:{lsize}:{lsize}:0.0{enable_suffix}"),
                format!("eq=contrast=1.18:saturation={sat:.2}:brightness=0.04{enable_suffix}"),
            ]
        }
        "vintageFilm" => {
            let grao = config.get("grao").and_then(|v| v.as_f64()).unwrap_or(40.0) / 100.0;
            let temp = config.get("temperatura").and_then(|v| v.as_f64()).unwrap_or(60.0) / 100.0;
            let noise_amt = (grao * 35.0).max(5.0).min(60.0).round() as i32;
            let rs = (temp * 0.18).min(0.35);
            let bs = -(temp * 0.16).min(0.35);
            vec![
                format!("colorbalance=rs={rs:.3}:gs=0.04:bs={bs:.3}{enable_suffix}"),
                format!("eq=contrast=1.12:saturation=0.75:brightness=0.02{enable_suffix}"),
                format!("noise=alls={noise_amt}:allf=t+u{enable_suffix}"),
            ]
        }
        "crtScanlines" => {
            let densidade = config.get("densidade").and_then(|v| v.as_u64()).unwrap_or(5).max(2).min(10);
            let contraste = config.get("contraste").and_then(|v| v.as_f64()).unwrap_or(50.0) / 100.0;
            let step = (12 - densidade).max(3);
            let opacity = (contraste * 0.65).max(0.15).min(0.85);
            vec![
                format!("drawgrid=w=iw:h={step}:t=1:c=black@{opacity:.2}{enable_suffix}"),
                format!("eq=contrast=1.14:saturation=1.20{enable_suffix}"),
            ]
        }
        "waveWarp" => {
            let forca = config.get("forca").and_then(|v| v.as_f64()).unwrap_or(50.0) / 100.0;
            let velocidade = config.get("velocidade").and_then(|v| v.as_f64()).unwrap_or(4.0);
            let max_angle = ((forca * 2.5).to_radians()).max(0.005);
            let freq = (velocidade * 0.7).max(0.2);
            let scale_margin = 1.0 + (forca * 0.05);
            if let Some(expr) = timeline_enable {
                vec![
                    format!("rotate=a='if({expr},sin(t*{freq:.2}*PI)*{max_angle:.4},0)':ow={width}:oh={height}:c=black"),
                    format!("scale='trunc(iw*{scale_margin:.3}/2)*2':'trunc(ih*{scale_margin:.3}/2)*2',crop={width}:{height}"),
                ]
            } else {
                vec![
                    format!("rotate=a='sin(t*{freq:.2}*PI)*{max_angle:.4}':ow={width}:oh={height}:c=black"),
                    format!("scale='trunc(iw*{scale_margin:.3}/2)*2':'trunc(ih*{scale_margin:.3}/2)*2',crop={width}:{height}"),
                ]
            }
        }
        "zoomBlur" => {
            let intensidade = config.get("intensidade").and_then(|v| v.as_f64()).unwrap_or(50.0) / 100.0;
            let raio = config.get("raio").and_then(|v| v.as_f64()).unwrap_or(12.0);
            let sigma = ((raio / 2.0) * intensidade).max(0.8);
            let zoom_amt = (intensidade * 0.08).max(0.02);
            if timeline_enable.is_some() {
                vec![
                    format!("gblur=sigma={sigma:.1}:steps=2{enable_suffix}"),
                    format!(
                        "zoompan=z='if(between(time,{start_sec:.4},{end_sec:.4}),1+abs(sin(PI*2*(time-{start_sec:.4})))*{zoom_amt:.4},1)':d=1:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':s={width}x{height}:fps={fps:.4}"
                    ),
                ]
            } else {
                vec![
                    format!("gblur=sigma={sigma:.1}:steps=2{enable_suffix}"),
                    format!(
                        "zoompan=z='1+abs(sin(PI*2*time))*{zoom_amt:.4}':d=1:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':s={width}x{height}:fps={fps:.4}"
                    ),
                ]
            }
        }
        "splitMirror" => {
            let orientacao = config.get("orientacao").and_then(|v| v.as_u64()).unwrap_or(0);
            if orientacao == 0 {
                vec![format!("hflip{enable_suffix}")]
            } else {
                vec![format!("vflip{enable_suffix}")]
            }
        }
        _ => Vec::new(),
    }
}

pub fn build_video_filter_complex(
    visual_clips: &[(Clip, MediaItem)],
    fx_clips: &[Clip],
    clip_factors: &[u32],
    incoming_trans: &HashMap<String, TransitionConfig>,
    outgoing_trans: &HashMap<String, TransitionConfig>,
    cut_transitions: &[(String, String, TransitionConfig)],
    clip_id_to_index: &HashMap<String, usize>,
    width: u32,
    height: u32,
    export_fps: f64,
    project_fps: f64,
    total_duration_sec: f64,
    scale_factor_x: f64,
    scale_factor_y: f64,
    settings: &ExportSettings,
) -> (String, String, Option<PathBuf>, Vec<PathBuf>) {
    let mut filter_complex = String::new();
    let mut temp_lut_files = Vec::new();

    // Base canvas with continuous black color at target export frame rate (e.g. 24 FPS)
    filter_complex.push_str(&format!(
        "color=c=black:s={}x{}:r={:.4}:d={:.6}[vbase0];",
        width, height, export_fps, total_duration_sec
    ));

    // Conformance and Composition of each visual clip with Full Transformation and Transitions
    for i in 0..visual_clips.len() {
        let (clip, _) = &visual_clips[i];
        let in_tc = incoming_trans.get(&clip.id);
        let out_tc = outgoing_trans.get(&clip.id);

        let in_trans_dur = in_tc.map(|t| t.duration_frames as f64 / project_fps).unwrap_or(0.0);
        let out_trans_dur = out_tc.map(|t| t.duration_frames as f64 / project_fps).unwrap_or(0.0);
        let in_half_dur = in_trans_dur / 2.0;
        let out_half_dur = out_trans_dur / 2.0;

        let clip_start_sec = clip.start_frame as f64 / project_fps;
        let clip_dur_sec = clip.duration_frames as f64 / project_fps;
        let clip_end_sec = clip_start_sec + clip_dur_sec;

        let eff_start_sec = (clip_start_sec - in_half_dur).max(0.0);
        let eff_end_sec = clip_end_sec + out_half_dur;
        let eff_dur_sec = eff_end_sec - eff_start_sec;
        let factor = clip_factors[i];
        let clip_internal_fps = export_fps * (factor as f64);

        let mut clip_filter_chain = Vec::new();

        // 1. Internal composition frame rate, pts, and duration (clip local time t = 0 .. eff_dur_sec)
        let speed = clip.speed.unwrap_or(1.0);
        if (speed - 1.0).abs() > 0.0001 && speed > 0.0 {
            clip_filter_chain.push(format!("setpts=(1/{:.6})*(PTS-STARTPTS)", speed));
        } else {
            clip_filter_chain.push("setpts=PTS-STARTPTS".to_string());
        }
        clip_filter_chain.push(format!("fps=fps={clip_internal_fps:.4}:round=near"));
        clip_filter_chain.push(format!("trim=duration={eff_dur_sec:.6}"));
        if in_half_dur > 0.0 {
            clip_filter_chain.push(format!("tpad=start_mode=clone:start_duration={:.4}", in_half_dur + 0.1));
        }
        clip_filter_chain.push(format!("tpad=stop_mode=clone:stop_duration={:.4}", out_half_dur + 0.1));

        if clip.reversed.unwrap_or(false) {
            clip_filter_chain.push("reverse".to_string());
        }

        // 2. Mirroring (Flip)
        if clip.transform.scale_x < 0.0 {
            clip_filter_chain.push("hflip".to_string());
        }
        if clip.transform.scale_y < 0.0 {
            clip_filter_chain.push("vflip".to_string());
        }

        let local_time_var = if in_half_dur > 0.0 {
            format!("(t-{:.6})", in_half_dur)
        } else {
            "t".to_string()
        };

        // 3. Direct Source Unified Scale (4K/1080p Sharpness Preservation + Lanczos)
        let is_clipped = clip.clip_path.is_some();
        let fit_mode = if is_clipped { "max" } else { "min" };

        let is_scale_dynamic = is_track_dynamic(clip.transform_plugin_data.as_ref(), "scaleXTrack")
            || is_track_dynamic(clip.transform_plugin_data.as_ref(), "scaleYTrack");

        if is_scale_dynamic {
            let scale_x_expr = build_keyframe_expr(
                clip.transform_plugin_data.as_ref(),
                "scaleXTrack",
                clip.transform.scale_x.abs() as f64,
                1.0,
                project_fps,
                &local_time_var,
            );
            let scale_y_expr = build_keyframe_expr(
                clip.transform_plugin_data.as_ref(),
                "scaleYTrack",
                clip.transform.scale_y.abs() as f64,
                1.0,
                project_fps,
                &local_time_var,
            );
            clip_filter_chain.push(format!(
                "scale=eval=frame:w='max(2,round(iw*{fit_mode}({width}/iw,{height}/ih)*({scale_x_expr})/2)*2)':h='max(2,round(ih*{fit_mode}({width}/iw,{height}/ih)*({scale_y_expr})/2)*2)':flags=lanczos+accurate_rnd"
            ));
        } else {
            let scale_x_val = build_keyframe_expr(
                clip.transform_plugin_data.as_ref(),
                "scaleXTrack",
                clip.transform.scale_x.abs() as f64,
                1.0,
                project_fps,
                &local_time_var,
            ).parse::<f64>().unwrap_or(clip.transform.scale_x.abs() as f64);

            let scale_y_val = build_keyframe_expr(
                clip.transform_plugin_data.as_ref(),
                "scaleYTrack",
                clip.transform.scale_y.abs() as f64,
                1.0,
                project_fps,
                &local_time_var,
            ).parse::<f64>().unwrap_or(clip.transform.scale_y.abs() as f64);

            clip_filter_chain.push(format!(
                "scale=w='max(2,round(iw*{fit_mode}({width}/iw,{height}/ih)*{scale_x_val:.4}/2)*2)':h='max(2,round(ih*{fit_mode}({width}/iw,{height}/ih)*{scale_y_val:.4}/2)*2)':flags=lanczos+accurate_rnd"
            ));
        }

        if is_clipped {
            clip_filter_chain.push(format!("crop={width}:{height}:(in_w-{width})/2:(in_h-{height})/2"));
        }

        // 4. Rotation on the already scaled high-definition buffer
        let is_rot_dynamic = is_track_dynamic(clip.transform_plugin_data.as_ref(), "rotationTrack");
        let rot_val_static = build_keyframe_expr(
            clip.transform_plugin_data.as_ref(),
            "rotationTrack",
            clip.transform.rotation_degrees as f64,
            1.0,
            project_fps,
            &local_time_var,
        ).parse::<f64>().unwrap_or(clip.transform.rotation_degrees as f64);

        if is_rot_dynamic || rot_val_static.abs() > 0.01 {
            clip_filter_chain.push("format=rgba".to_string());

            let max_scale_x = get_track_max_abs(clip.transform_plugin_data.as_ref(), "scaleXTrack", clip.transform.scale_x.abs() as f64);
            let max_scale_y = get_track_max_abs(clip.transform_plugin_data.as_ref(), "scaleYTrack", clip.transform.scale_y.abs() as f64);
            let max_scale = max_scale_x.max(max_scale_y).max(1.0);
            let max_dim = ((width as f64).hypot(height as f64) * max_scale).ceil() as u32;
            let fixed_ow = (((max_dim / 2) + 1) * 2).max(width).max(height);
            let fixed_oh = fixed_ow;

            if is_rot_dynamic {
                let rot_expr = build_keyframe_expr(
                    clip.transform_plugin_data.as_ref(),
                    "rotationTrack",
                    clip.transform.rotation_degrees as f64,
                    1.0,
                    project_fps,
                    &local_time_var,
                );
                clip_filter_chain.push(format!(
                    "rotate='({rot_expr})*PI/180':ow={fixed_ow}:oh={fixed_oh}:c=black@0:bilinear=1"
                ));
            } else {
                let rot_rad = rot_val_static * std::f64::consts::PI / 180.0;
                clip_filter_chain.push(format!(
                    "rotate={rot:.4}:ow={fixed_ow}:oh={fixed_oh}:c=black@0:bilinear=1",
                    rot = rot_rad
                ));
            }
        }

        // 6. Clip Opacity
        if clip.transform.opacity < 0.999 {
            if !is_rot_dynamic && rot_val_static.abs() <= 0.01 {
                clip_filter_chain.push("format=rgba".to_string());
            }
            clip_filter_chain.push(format!(
                "colorchannelmixer=aa={:.3}",
                clip.transform.opacity.max(0.0).min(1.0)
            ));
        }

        // 7. Color and Image Adjustments (Adjustments & Enhancement Plugins)
        let mut adj_b = 0.0;
        let mut adj_c = 1.0;
        let mut adj_s = 1.0;
        let mut adj_hue = 0.0;
        let mut adj_sharpen = 0.0;
        let mut adj_vignette = 0.0;
        let mut has_color_mod = false;

        if let Some(ref adj_val) = clip.adjustments_plugin_data {
            has_color_mod = true;
            let exposure = adj_val.get("exposure").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let lightness = adj_val.get("lightness").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let brilliance = adj_val.get("brilliance").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let highlight = adj_val.get("highlight").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let shadow = adj_val.get("shadow").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let whites = adj_val.get("whites").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let blacks = adj_val.get("blacks").and_then(|v| v.as_f64()).unwrap_or(0.0);

            let contrast = adj_val.get("contrast").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let clarity = adj_val.get("clarity").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let effects = adj_val.get("effects").and_then(|v| v.as_f64()).unwrap_or(0.0);

            let saturation = adj_val.get("saturation").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let temp = adj_val.get("temp").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let tint = adj_val.get("tint").and_then(|v| v.as_f64()).unwrap_or(0.0);
            adj_sharpen += adj_val.get("sharpen").and_then(|v| v.as_f64()).unwrap_or(0.0);
            adj_vignette += adj_val.get("vignette").and_then(|v| v.as_f64()).unwrap_or(0.0);

            adj_b += exposure * 0.008 + lightness * 0.005 + brilliance * 0.004 + highlight * 0.003 + shadow * 0.003 + whites * 0.0025 + blacks * 0.0025;
            adj_c += contrast * 0.008 + whites * 0.004 - blacks * 0.004 + clarity * 0.004 + effects * 0.003;
            adj_s += saturation * 0.008 + brilliance * 0.0025 + effects * 0.0025;
            adj_hue += temp * 0.5 + tint * 0.45;
        }

        if let Some(ref enh_val) = clip.enhancement_plugin_data {
            has_color_mod = true;
            if let Some(auto_adj) = enh_val.get("autoAdjust") {
                if auto_adj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let mult = auto_adj.get("intensity").and_then(|v| v.as_f64()).unwrap_or(80.0) / 100.0;
                    adj_b += 0.04 * mult;
                    adj_c += 0.08 * mult;
                    adj_s += 0.06 * mult;
                }
            }
            if let Some(color_match) = enh_val.get("colorMatch") {
                if color_match.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let mult = color_match.get("intensity").and_then(|v| v.as_f64()).unwrap_or(75.0) / 100.0;
                    adj_s += 0.05 * mult;
                    adj_b += 0.02 * mult;
                }
            }
            if let Some(cc) = enh_val.get("colorCorrection") {
                if cc.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let mult = cc.get("intensity").and_then(|v| v.as_f64()).unwrap_or(70.0) / 100.0;
                    let mode = cc.get("mode").and_then(|v| v.as_str()).unwrap_or("cinema");
                    match mode {
                        "cinema" => {
                            adj_c += 0.10 * mult;
                            adj_hue -= 12.0 * mult;
                        }
                        "vivid" => {
                            adj_s += 0.22 * mult;
                            adj_c += 0.06 * mult;
                        }
                        "neutral" => {
                            adj_c += 0.03 * mult;
                            adj_s += 0.01 * mult;
                        }
                        "auto" => {
                            adj_c += 0.07 * mult;
                            adj_b += 0.03 * mult;
                        }
                        _ => {}
                    }
                }
            }
            if let Some(enh_img) = enh_val.get("enhanceImage") {
                if enh_img.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let mult = enh_img.get("intensity").and_then(|v| v.as_f64()).unwrap_or(65.0) / 100.0;
                    adj_sharpen += 55.0 * mult;
                    adj_c += 0.05 * mult;
                }
            }
            if let Some(noise) = enh_val.get("imageNoise") {
                if noise.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let mult = noise.get("reduction").and_then(|v| v.as_f64()).unwrap_or(50.0) / 100.0;
                    clip_filter_chain.push(format!("gblur=sigma={:.1}:steps=1", (0.5 * mult).max(0.2)));
                }
            }
        }

        if has_color_mod {
            let total_b = adj_b.max(-1.0).min(1.0);
            let total_c = adj_c.max(0.1).min(3.0);
            let total_s = adj_s.max(0.0).min(3.0);

            if total_b.abs() > 0.001 || (total_c - 1.0).abs() > 0.001 || (total_s - 1.0).abs() > 0.001 {
                clip_filter_chain.push(format!(
                    "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}",
                    total_b, total_c, total_s
                ));
            }

            let total_hue = adj_hue.max(-90.0).min(90.0);
            if total_hue.abs() > 0.5 {
                clip_filter_chain.push(format!("hue=h={:.1}", total_hue));
            }

            if adj_sharpen > 1.0 {
                let sh_val = (adj_sharpen / 100.0 * 1.5).max(0.1).min(2.0);
                clip_filter_chain.push(format!("unsharp=5:5:{:.2}:5:5:0.0", sh_val));
            }

            if adj_vignette > 1.0 {
                let vig_angle = (adj_vignette / 100.0 * (std::f64::consts::PI / 4.0)).max(0.1);
                clip_filter_chain.push(format!("vignette='{:.3}'", vig_angle));
            }
        }

        // 8. Style Filters on the clip itself
        let filter_data = clip.filter_plugin_data.as_ref().or_else(|| {
            clip.transform_plugin_data.as_ref().and_then(|t| t.get("core.video.filters"))
        });
        if let Some(filt_val) = filter_data {
            let filter_id = filt_val.get("filterId").and_then(|v| v.as_str()).unwrap_or("4k");
            let intensity = filt_val.get("intensity").and_then(|v| v.as_f64()).unwrap_or(100.0);
            let custom_css = filt_val.get("cssFilter").and_then(|v| v.as_str());

            // Support for custom 3D LUT file if provided in payload
            if let Some(lut_content) = filt_val
                .get("customLutContent")
                .or_else(|| filt_val.get("lutRawCube"))
                .and_then(|v| v.as_str())
            {
                if !lut_content.trim().is_empty() {
                    let temp_cube = std::env::temp_dir().join(format!("sniplic_lut_{}.cube", uuid::Uuid::new_v4().simple()));
                    if std::fs::write(&temp_cube, lut_content).is_ok() {
                        let escaped = escape_ffmpeg_filter_path(&temp_cube);
                        clip_filter_chain.push(format!("lut3d=file='{}'", escaped));
                        temp_lut_files.push(temp_cube);
                    }
                }
            } else {
                let filt_list = build_filter_directives(filter_id, intensity, custom_css, None);
                clip_filter_chain.extend(filt_list);
            }
        }

        // 8.5 Visual Effects on the clip itself (e.g. blur, glow, vignette, pixelate, zoomPulse)
        let effect_data = clip.effect_plugin_data.as_ref().or_else(|| {
            clip.transform_plugin_data.as_ref().and_then(|t| t.get("core.video.effects"))
        });
        if let Some(eff_val) = effect_data {
            let effect_id = eff_val.get("effectId").and_then(|v| v.as_str()).unwrap_or("blur");
            let empty_cfg = serde_json::json!({});
            let config = eff_val.get("config").unwrap_or(&empty_cfg);
            let eff_list = build_effect_filters(
                effect_id,
                config,
                None,
                width,
                height,
                export_fps,
                0.0,
                eff_dur_sec,
            );
            clip_filter_chain.extend(eff_list);
        }

        // 8.5. Support for Video Fade In and Fade Out
        let v_fade_in = clip.fade_in_frames.unwrap_or(0);
        let v_fade_out = clip.fade_out_frames.unwrap_or(0);
        if v_fade_in > 0 {
            let in_sec = (v_fade_in as f64 / project_fps).min(eff_dur_sec / 2.0);
            clip_filter_chain.push(format!("fade=t=in:st=0:d={in_sec:.4}:alpha=1"));
        }
        if v_fade_out > 0 {
            let out_sec = (v_fade_out as f64 / project_fps).min(eff_dur_sec / 2.0);
            let out_start = (eff_dur_sec - out_sec).max(0.0);
            clip_filter_chain.push(format!("fade=t=out:st={out_start:.4}:d={out_sec:.4}:alpha=1"));
        }

        // 8.6 Support for Chroma Key (Green Screen and Black Screen Removal)
        let chroma_data = clip.chroma_key_plugin_data.as_ref().or_else(|| {
            clip.transform_plugin_data.as_ref().and_then(|t| t.get("core.video.chroma"))
        });
        if let Some(ch_val) = chroma_data {
            let enabled = ch_val.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            if enabled {
                let mode = ch_val.get("mode").and_then(|v| v.as_str()).unwrap_or("none");
                let blend = ch_val.get("blend").and_then(|v| v.as_f64()).unwrap_or(0.10);
                match mode {
                    "green" => {
                        let similarity = ch_val.get("similarity").and_then(|v| v.as_f64()).unwrap_or(0.35);
                        clip_filter_chain.push(format!("colorkey=color=0x00FF00:similarity={similarity:.3}:blend={blend:.3}"));
                    }
                    "black" => {
                        let similarity = ch_val.get("similarity").and_then(|v| v.as_f64()).unwrap_or(0.15);
                        clip_filter_chain.push(format!("colorkey=color=0x000000:similarity={similarity:.3}:blend={blend:.3}"));
                    }
                    _ => {}
                }
            }
        }

        // 9. Unit SAR and RGBA format
        clip_filter_chain.push("setsar=1".to_string());

        // Support for Canvas plugin (Solid Background or Media Blur)
        let canvas_mode = clip
            .canvas_plugin_data
            .as_ref()
            .and_then(|v| v.get("mode"))
            .and_then(|m| m.as_str())
            .unwrap_or("none");

        let current_bg_label = match canvas_mode {
            "color" => {
                let hex_color = clip
                    .canvas_plugin_data
                    .as_ref()
                    .and_then(|v| v.get("color"))
                    .and_then(|c| c.as_str())
                    .map(|s| s.trim_start_matches('#'))
                    .unwrap_or("000000");

                let safe_hex = if hex_color.chars().all(|c| c.is_ascii_hexdigit())
                    && (hex_color.len() == 6 || hex_color.len() == 8)
                {
                    hex_color
                } else {
                    "000000"
                };

                filter_complex.push_str(&format!(
                    "color=c=0x{safe_hex}:s={width}x{height}:r={export_fps:.4}:d={eff_dur_sec:.6},format=rgba[vbg{i}];"
                ));
                format!("[vbg{i}]")
            }
            "blur" => {
                let blur_sigma = clip
                    .canvas_plugin_data
                    .as_ref()
                    .and_then(|v| v.get("blurLevel"))
                    .and_then(|b| b.as_f64())
                    .unwrap_or(6.0)
                    .max(1.0);

                let bg_w = (((width as f64 * 1.1).round() as u32) / 2) * 2;
                let bg_h = (((height as f64 * 1.1).round() as u32) / 2) * 2;

                filter_complex.push_str(&format!(
                    "[{i}:v]split=2[v_in_fg{i}][v_in_bg{i}];"
                ));
                let bg_setpts = if (speed - 1.0).abs() > 0.0001 && speed > 0.0 {
                    format!("setpts=(1/{:.6})*(PTS-STARTPTS)", speed)
                } else {
                    "setpts=PTS-STARTPTS".to_string()
                };
                filter_complex.push_str(&format!(
                    "[v_in_bg{i}]{bg_setpts},fps=fps={export_fps:.4}:round=near,trim=duration={eff_dur_sec:.6},tpad=stop_mode=clone:stop_duration={:.4},scale={bg_w}:{bg_h}:force_original_aspect_ratio=increase,crop={width}:{height},gblur=sigma={blur_sigma:.1}:steps=2,format=rgba,colorchannelmixer=aa=0.75,setsar=1[vbg{i}];",
                    out_half_dur + 0.1
                ));
                format!("[vbg{i}]")
            }
            _ => {
                filter_complex.push_str(&format!(
                    "color=c=black@0:s={width}x{height}:r={export_fps:.4}:d={eff_dur_sec:.6},format=rgba[vbg{i}];"
                ));
                format!("[vbg{i}]")
            }
        };

        let fg_input = if canvas_mode == "blur" {
            format!("[v_in_fg{i}]")
        } else {
            format!("[{i}:v]")
        };

        let pos_x_expr = build_keyframe_expr(
            clip.transform_plugin_data.as_ref(),
            "positionXTrack",
            clip.transform.position_x as f64,
            scale_factor_x,
            project_fps,
            &local_time_var,
        );
        let pos_y_expr = build_keyframe_expr(
            clip.transform_plugin_data.as_ref(),
            "positionYTrack",
            clip.transform.position_y as f64,
            scale_factor_y,
            project_fps,
            &local_time_var,
        );

        let crop_slice_info = clip.clip_path.as_deref().and_then(|cp| {
            parse_clip_path_inset(cp, width as f64, height as f64)
        });

        if let Some((cw, ch, cx, cy)) = crop_slice_info {
            clip_filter_chain.push(format!("crop={:.0}:{:.0}:{:.0}:{:.0}", cw, ch, cx, cy));
        }

        let (base_overlay_x, base_overlay_y) = if let Some((_, _, cx, cy)) = crop_slice_info {
            (format!("{:.0}", cx), format!("{:.0}", cy))
        } else {
            ("(W-w)/2".to_string(), "(H-h)/2".to_string())
        };

        let filter_str = clip_filter_chain.join(",");

        if factor <= 1 {
            filter_complex.push_str(&format!(
                "{fg_input}{filter_str}[v_scaled{i}];"
            ));
            filter_complex.push_str(&format!(
                "{current_bg_label}[v_scaled{i}]overlay=eval=frame:x='{base_overlay_x}+({pos_x_expr})':y='{base_overlay_y}+({pos_y_expr})':format=auto:alpha=straight,format=rgba[v_comp{i}];"
            ));
        } else {
            let shutter_weights = generate_shutter_weights(factor);
            filter_complex.push_str(&format!(
                "{fg_input}{filter_str}[v_prep{i}];"
            ));
            filter_complex.push_str(&format!(
                "color=c=black@0:s={width}x{height}:r={clip_internal_fps:.4}:d={eff_dur_sec:.6},format=rgba[vclip_subcanvas{i}];"
            ));
            filter_complex.push_str(&format!(
                "[vclip_subcanvas{i}][v_prep{i}]overlay=eval=frame:x='{base_overlay_x}+({pos_x_expr})':y='{base_overlay_y}+({pos_y_expr})':format=auto:alpha=straight[v_animated{i}];"
            ));
            filter_complex.push_str(&format!(
                "[v_animated{i}]tmix=frames={factor}:weights='{shutter_weights}',fps=fps={export_fps:.4}:round=near,setpts=PTS-STARTPTS[v_integrated{i}];"
            ));
            filter_complex.push_str(&format!(
                "{current_bg_label}[v_integrated{i}]overlay=0:0:format=auto:alpha=straight,format=rgba[v_comp{i}];"
            ));
        }

        let has_in = in_tc.is_some();
        let has_out = out_tc.is_some();
        let in_dur = in_trans_dur;
        let out_dur = out_trans_dur;
        let out_start = (eff_dur_sec - out_dur).max(0.0);
        let body_start = if has_in { in_dur } else { 0.0 };
        let body_end = if has_out { out_start } else { eff_dur_sec };
        let body_dur = (body_end - body_start).max(0.0);
        let body_timeline_start = eff_start_sec + body_start;

        let has_body = body_dur > 0.001;
        match (has_in, has_out, has_body) {
            (true, true, true) => {
                filter_complex.push_str(&format!(
                    "[v_comp{i}]split=3[v_head_raw{i}][v_body_raw{i}][v_tail_raw{i}];"
                ));
                filter_complex.push_str(&format!(
                    "[v_head_raw{i}]trim=start=0:duration={in_dur:.6},setpts=PTS-STARTPTS,format=rgba[v_head{i}];"
                ));
                filter_complex.push_str(&format!(
                    "[v_tail_raw{i}]trim=start={out_start:.6}:duration={out_dur:.6},setpts=PTS-STARTPTS,format=rgba[v_tail{i}];"
                ));
                filter_complex.push_str(&format!(
                    "[v_body_raw{i}]trim=start={body_start:.6}:duration={body_dur:.6},setpts=PTS-STARTPTS+({body_timeline_start:.6}/TB)[v_body{i}];"
                ));
            }
            (true, true, false) => {
                filter_complex.push_str(&format!(
                    "[v_comp{i}]split=2[v_head_raw{i}][v_tail_raw{i}];"
                ));
                filter_complex.push_str(&format!(
                    "[v_head_raw{i}]trim=start=0:duration={in_dur:.6},setpts=PTS-STARTPTS,format=rgba[v_head{i}];"
                ));
                filter_complex.push_str(&format!(
                    "[v_tail_raw{i}]trim=start={out_start:.6}:duration={out_dur:.6},setpts=PTS-STARTPTS,format=rgba[v_tail{i}];"
                ));
            }
            (true, false, true) => {
                filter_complex.push_str(&format!(
                    "[v_comp{i}]split=2[v_head_raw{i}][v_body_raw{i}];"
                ));
                filter_complex.push_str(&format!(
                    "[v_head_raw{i}]trim=start=0:duration={in_dur:.6},setpts=PTS-STARTPTS,format=rgba[v_head{i}];"
                ));
                filter_complex.push_str(&format!(
                    "[v_body_raw{i}]trim=start={body_start:.6}:duration={body_dur:.6},setpts=PTS-STARTPTS+({body_timeline_start:.6}/TB)[v_body{i}];"
                ));
            }
            (true, false, false) => {
                filter_complex.push_str(&format!(
                    "[v_comp{i}]trim=start=0:duration={in_dur:.6},setpts=PTS-STARTPTS,format=rgba[v_head{i}];"
                ));
            }
            (false, true, true) => {
                filter_complex.push_str(&format!(
                    "[v_comp{i}]split=2[v_body_raw{i}][v_tail_raw{i}];"
                ));
                filter_complex.push_str(&format!(
                    "[v_tail_raw{i}]trim=start={out_start:.6}:duration={out_dur:.6},setpts=PTS-STARTPTS,format=rgba[v_tail{i}];"
                ));
                filter_complex.push_str(&format!(
                    "[v_body_raw{i}]trim=start={body_start:.6}:duration={body_dur:.6},setpts=PTS-STARTPTS+({body_timeline_start:.6}/TB)[v_body{i}];"
                ));
            }
            (false, true, false) => {
                filter_complex.push_str(&format!(
                    "[v_comp{i}]trim=start={out_start:.6}:duration={out_dur:.6},setpts=PTS-STARTPTS,format=rgba[v_tail{i}];"
                ));
            }
            (false, false, _) => {
                filter_complex.push_str(&format!(
                    "[v_comp{i}]trim=start={body_start:.6}:duration={body_dur:.6},setpts=PTS-STARTPTS+({body_timeline_start:.6}/TB)[v_body{i}];"
                ));
            }
        }
    }

    // 10.2 Generates xfade transition segments for each cut between adjacent clips
    for (cut_idx, (left_id, right_id, cfg)) in cut_transitions.iter().enumerate() {
        if let (Some(&left_idx), Some(&right_idx)) = (clip_id_to_index.get(left_id), clip_id_to_index.get(right_id)) {
            let trans_dur = cfg.duration_frames as f64 / project_fps;
            let half_dur = trans_dur / 2.0;
            let right_clip = &visual_clips[right_idx].0;
            let cut_sec = right_clip.start_frame as f64 / project_fps;
            let trans_start = (cut_sec - half_dur).max(0.0);
            let ffmpeg_type = &cfg.ffmpeg_type;

            filter_complex.push_str(&format!(
                "[v_tail{left_idx}][v_head{right_idx}]xfade=transition={ffmpeg_type}:duration={trans_dur:.6}:offset=0,setpts=PTS-STARTPTS+({trans_start:.6}/TB),format=rgba[v_trans{cut_idx}];"
            ));
        }
    }

    // 10.3 Sequential overlay of bodies and transitions on base canvas
    let mut layer_idx = 0;

    for (i, (clip, _)) in visual_clips.iter().enumerate() {
        let in_tc = incoming_trans.get(&clip.id);
        let out_tc = outgoing_trans.get(&clip.id);
        let in_trans_dur = in_tc.map(|t| t.duration_frames as f64 / project_fps).unwrap_or(0.0);
        let out_trans_dur = out_tc.map(|t| t.duration_frames as f64 / project_fps).unwrap_or(0.0);
        let in_half_dur = in_trans_dur / 2.0;
        let out_half_dur = out_trans_dur / 2.0;

        let clip_start_sec = clip.start_frame as f64 / project_fps;
        let clip_dur_sec = clip.duration_frames as f64 / project_fps;
        let clip_end_sec = clip_start_sec + clip_dur_sec;

        let eff_start_sec = (clip_start_sec - in_half_dur).max(0.0);
        let eff_end_sec = clip_end_sec + out_half_dur;
        let eff_dur_sec = eff_end_sec - eff_start_sec;

        let _has_in = in_tc.is_some();
        let _has_out = out_tc.is_some();
        let in_dur = in_trans_dur;
        let out_dur = out_trans_dur;
        let out_start = (eff_dur_sec - out_dur).max(0.0);
        let body_start = if in_tc.is_some() { in_dur } else { 0.0 };
        let body_end = if out_tc.is_some() { out_start } else { eff_dur_sec };
        let body_dur = (body_end - body_start).max(0.0);
        let body_timeline_start = eff_start_sec + body_start;
        let body_timeline_end = body_timeline_start + body_dur;

        if body_dur > 0.001 {
            filter_complex.push_str(&format!(
                "[vbase{layer_idx}][v_body{i}]overlay=0:0:format=auto:alpha=straight:eof_action=pass:enable='between(t,{body_timeline_start:.6},{body_timeline_end:.6})'[vbase{next}];",
                next = layer_idx + 1
            ));
            layer_idx += 1;
        }
    }

    for (cut_idx, (left_id, right_id, cfg)) in cut_transitions.iter().enumerate() {
        if let (Some(&_left_idx), Some(&right_idx)) = (clip_id_to_index.get(left_id), clip_id_to_index.get(right_id)) {
            let trans_dur = cfg.duration_frames as f64 / project_fps;
            let half_dur = trans_dur / 2.0;
            let right_clip = &visual_clips[right_idx].0;
            let cut_sec = right_clip.start_frame as f64 / project_fps;
            let trans_start = (cut_sec - half_dur).max(0.0);
            let trans_end = trans_start + trans_dur;

            filter_complex.push_str(&format!(
                "[vbase{layer_idx}][v_trans{cut_idx}]overlay=0:0:format=auto:alpha=straight:eof_action=pass:enable='between(t,{trans_start:.6},{trans_end:.6})'[vbase{next}];",
                next = layer_idx + 1
            ));
            layer_idx += 1;
        }
    }

    // 10.4 Application of Filter and Effect Tracks (FX Tracks)
    for fx_clip in fx_clips {
        let start_sec = fx_clip.start_frame as f64 / project_fps;
        let dur_sec = fx_clip.duration_frames as f64 / project_fps;
        let end_sec = start_sec + dur_sec;
        if dur_sec <= 0.001 {
            continue;
        }

        let timeline_enable = format!("between(t,{:.6},{:.6})", start_sec, end_sec);
        let mut fx_filters = Vec::new();

        if fx_clip.is_effect_clip() {
            let effect_data = fx_clip.effect_plugin_data.as_ref().or_else(|| {
                fx_clip
                    .transform_plugin_data
                    .as_ref()
                    .and_then(|t| t.get("core.video.effects"))
            });
            let mut effect_id = effect_data
                .and_then(|d| d.get("effectId"))
                .and_then(|v| v.as_str());
            if effect_id.is_none() && fx_clip.media_id.starts_with("fx_effect_") {
                effect_id = Some(fx_clip.media_id.trim_start_matches("fx_effect_"));
            }
            let effect_id = effect_id.unwrap_or("blur");
            let empty_cfg = serde_json::json!({});
            let config = effect_data
                .and_then(|d| d.get("config"))
                .unwrap_or(&empty_cfg);

            let eff_list = build_effect_filters(
                effect_id,
                config,
                Some(&timeline_enable),
                width,
                height,
                export_fps,
                start_sec,
                end_sec,
            );
            fx_filters.extend(eff_list);
        } else {
            let filter_data = fx_clip.filter_plugin_data.as_ref().or_else(|| {
                fx_clip
                    .transform_plugin_data
                    .as_ref()
                    .and_then(|t| t.get("core.video.filters"))
            });
            let mut filter_id = filter_data
                .and_then(|d| d.get("filterId"))
                .and_then(|v| v.as_str());
            if filter_id.is_none() && fx_clip.media_id.starts_with("fx_") {
                filter_id = Some(fx_clip.media_id.trim_start_matches("fx_"));
            }
            let filter_id = filter_id.unwrap_or("4k");
            let intensity = filter_data
                .and_then(|d| d.get("intensity"))
                .and_then(|v| v.as_f64())
                .unwrap_or(100.0);
            let custom_css = filter_data
                .and_then(|d| d.get("cssFilter"))
                .and_then(|v| v.as_str());

            // Support for custom 3D LUT file if provided in payload
            if let Some(lut_content) = filter_data
                .and_then(|d| d.get("customLutContent").or_else(|| d.get("lutRawCube")))
                .and_then(|v| v.as_str())
            {
                if !lut_content.trim().is_empty() {
                    let temp_cube = std::env::temp_dir().join(format!(
                        "sniplic_lut_{}.cube",
                        uuid::Uuid::new_v4().simple()
                    ));
                    if std::fs::write(&temp_cube, lut_content).is_ok() {
                        let escaped = escape_ffmpeg_filter_path(&temp_cube);
                        fx_filters.push(format!("lut3d=file='{}':enable='{}'", escaped, timeline_enable));
                        temp_lut_files.push(temp_cube);
                    }
                }
            } else {
                let filt_list = build_filter_directives(
                    filter_id,
                    intensity,
                    custom_css,
                    Some(&timeline_enable),
                );
                fx_filters.extend(filt_list);
            }
        }

        if !fx_filters.is_empty() {
            let filter_chain = fx_filters.join(",");
            filter_complex.push_str(&format!(
                "[vbase{layer_idx}]{filter_chain}[vbase{next}];",
                next = layer_idx + 1
            ));
            layer_idx += 1;
        }
    }

    let mut final_v_label = format!("[vbase{}]", layer_idx);
    let mut temp_ass_file: Option<PathBuf> = None;

    // 11. Hardcoded Subtitles Burn-in
    if let Some(ref subs) = settings.subtitles {
        if !subs.is_empty() {
            match generate_ass_subtitles_file(
                subs,
                &settings.subtitle_config,
                width,
                height,
                project_fps,
                settings.preview_height,
            ) {
                Ok(ass_path) => {
                    let escaped_ass = escape_ffmpeg_filter_path(&ass_path);
                    let fonts_dir = crate::core::paths::get_app_fonts_dir();
                    let fonts_opt = if fonts_dir.exists() {
                        let escaped_fonts = escape_ffmpeg_filter_path(&fonts_dir);
                        format!(":fontsdir='{escaped_fonts}'")
                    } else {
                        String::new()
                    };
                    filter_complex.push_str(&format!(
                        "{final_v_label}subtitles=filename='{escaped_ass}'{fonts_opt}[v_with_subs];"
                    ));
                    final_v_label = "[v_with_subs]".to_string();
                    temp_ass_file = Some(ass_path);
                }
                Err(err) => {
                    tracing::warn!("Could not generate subtitles file for export: {}", err);
                }
            }
        }
    }

    (filter_complex, final_v_label, temp_ass_file, temp_lut_files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_css_filter_complex() {
        let css = "contrast(1.350) sepia(0.500) saturate(1.400) hue-rotate(-10.0deg) blur(5px)";
        let parsed = parse_css_filter(css);
        assert!((parsed.contrast - 1.35).abs() < 0.001);
        assert!((parsed.sepia - 0.5).abs() < 0.001);
        assert!((parsed.saturate - 1.4).abs() < 0.001);
        assert!((parsed.hue_rotate_deg - (-10.0)).abs() < 0.001);
        assert!((parsed.blur_px - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_css_filter_units() {
        let css = "grayscale(80%) invert(100%)";
        let parsed = parse_css_filter(css);
        assert!((parsed.grayscale - 0.8).abs() < 0.001);
        assert!((parsed.invert - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_build_filter_directives_with_timeline() {
        let directives = build_filter_directives(
            "bladeRunnerAmber",
            100.0,
            Some("contrast(1.350) sepia(0.500) saturate(1.400) hue-rotate(-10.0deg)"),
            Some("between(t,2.000000,5.000000)"),
        );
        assert!(!directives.is_empty());
        let joined = directives.join(",");
        assert!(joined.contains("eq=brightness="));
        assert!(joined.contains("contrast=1.350"));
        assert!(joined.contains("hue=h=-10.0"));
        assert!(joined.contains("colorchannelmixer="));
        assert!(joined.contains(":enable='between(t,2.000000,5.000000)'"));
    }

    #[test]
    fn test_build_effect_filters_blur_and_vignette() {
        let cfg_blur = json!({ "desfoque": 20 });
        let blur_dirs = build_effect_filters("blur", &cfg_blur, Some("between(t,1.0,3.0)"), 1920, 1080, 30.0, 1.0, 3.0);
        assert_eq!(blur_dirs.len(), 1);
        assert!(blur_dirs[0].contains("gblur=sigma=16.0:steps=2:enable='between(t,1.0,3.0)'"));

        let cfg_vig = json!({ "forca": 80 });
        let vig_dirs = build_effect_filters("vignette", &cfg_vig, None, 1920, 1080, 30.0, 0.0, 5.0);
        assert_eq!(vig_dirs.len(), 1);
        assert!(vig_dirs[0].starts_with("vignette=angle="));
    }

    #[test]
    fn test_build_effect_filters_pixelate() {
        let cfg = json!({ "tamanho": 32 });
        let dirs = build_effect_filters("pixelate", &cfg, None, 1920, 1080, 30.0, 0.0, 5.0);
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0], "pixelize=w=32:h=32");
    }

    #[test]
    fn test_build_effect_filters_motion_group() {
        let cfg_shake = json!({ "forca": 60, "velocidade": 6 });
        let shake = build_effect_filters("cameraShake", &cfg_shake, Some("between(t,0,2)"), 1920, 1080, 30.0, 0.0, 2.0);
        assert_eq!(shake.len(), 1);
        assert!(shake[0].contains("crop=w='iw*0.92'"));
        assert!(shake[0].contains("scale=1920:1080"));

        let cfg_sway = json!({ "forca": 40, "velocidade": 4 });
        let sway = build_effect_filters("gentleSway", &cfg_sway, None, 1920, 1080, 30.0, 0.0, 2.0);
        assert_eq!(sway.len(), 1);
        assert!(sway[0].starts_with("rotate=a='sin(t*"));

        let cfg_spin = json!({ "velocidade": 5, "voltas": 1 });
        let spin = build_effect_filters("quickSpin", &cfg_spin, None, 1920, 1080, 30.0, 0.0, 2.0);
        assert_eq!(spin.len(), 1);
        assert!(spin[0].contains("rotate=a='t*"));

        let cfg_bounce = json!({ "forca": 50, "velocidade": 5 });
        let bounce = build_effect_filters("verticalBounce", &cfg_bounce, None, 1920, 1080, 30.0, 0.0, 2.0);
        assert_eq!(bounce.len(), 1);
        assert!(bounce[0].contains("abs(sin("));
    }

    #[test]
    fn test_build_effect_filters_new_suite() {
        let enable = Some("between(t,1.0,4.0)");

        // 1. digitalGlitch
        let glitch = build_effect_filters("digitalGlitch", &json!({ "intensidade": 60 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(glitch.len(), 2);
        assert!(glitch[0].contains("rgbashift=rh="));
        assert!(glitch[0].contains(":enable='between(t,1.0,4.0)'"));

        // 2. chromaticAberration
        let chrom = build_effect_filters("chromaticAberration", &json!({ "deslocamento": 10, "angulo": 0 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(chrom.len(), 1);
        assert!(chrom[0].contains("rgbashift=rh=10:rv=0:bh=-10:bv=0"));

        // 3. strobeFlash
        let strobe = build_effect_filters("strobeFlash", &json!({ "frequencia": 4, "intensidade": 80 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(strobe.len(), 1);
        assert!(strobe[0].contains("eq=brightness='if("));

        // 4. lightLeak
        let leak = build_effect_filters("lightLeak", &json!({ "intensidade": 70 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(leak.len(), 2);
        assert!(leak[0].contains("colorbalance=rs="));

        // 5. edgeGlow
        let glow = build_effect_filters("edgeGlow", &json!({ "intensidade": 50, "raio": 4 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(glow.len(), 2);
        assert!(glow[0].contains("unsharp="));

        // 6. vintageFilm
        let film = build_effect_filters("vintageFilm", &json!({ "grao": 40, "temperatura": 60 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(film.len(), 3);
        assert!(film[2].contains("noise=alls="));

        // 7. crtScanlines
        let crt = build_effect_filters("crtScanlines", &json!({ "densidade": 5, "contraste": 50 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(crt.len(), 2);
        assert!(crt[0].contains("drawgrid="));

        // 8. waveWarp
        let wave = build_effect_filters("waveWarp", &json!({ "velocidade": 4, "forca": 50 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(wave.len(), 2);
        assert!(wave[0].contains("rotate=a='if("));

        // 9. zoomBlur
        let zoom_b = build_effect_filters("zoomBlur", &json!({ "intensidade": 50, "raio": 12 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(zoom_b.len(), 2);
        assert!(zoom_b[0].contains("gblur=sigma="));

        // 10. splitMirror
        let split_h = build_effect_filters("splitMirror", &json!({ "orientacao": 0 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(split_h.len(), 1);
        assert!(split_h[0].contains("hflip"));

        let split_v = build_effect_filters("splitMirror", &json!({ "orientacao": 1 }), enable, 1920, 1080, 30.0, 1.0, 4.0);
        assert_eq!(split_v.len(), 1);
        assert!(split_v[0].contains("vflip"));
    }

    #[test]
    fn test_parse_clip_path_inset_vertical_slices() {
        let w = 1920.0;
        let h = 1080.0;
        // Strip 0 of 6 (left = 0%, right = 83.3333%)
        let res0 = parse_clip_path_inset("inset(0% 83.3333% 0% 0%)", w, h);
        assert!(res0.is_some());
        let (cw0, ch0, cx0, cy0) = res0.unwrap();
        assert!((cw0 - 320.0).abs() < 0.1);
        assert_eq!(ch0, 1080.0);
        assert_eq!(cx0, 0.0);
        assert_eq!(cy0, 0.0);

        // Strip 1 of 6 (left = 16.6667%, right = 66.6667%)
        let res1 = parse_clip_path_inset("inset(0% 66.6667% 0% 16.6667%)", w, h);
        assert!(res1.is_some());
        let (cw1, ch1, cx1, cy1) = res1.unwrap();
        assert!((cw1 - 320.0).abs() < 0.1);
        assert_eq!(ch1, 1080.0);
        assert!((cx1 - 320.0).abs() < 0.1);
        assert_eq!(cy1, 0.0);

        // Strip 5 of 6 (left = 83.3333%, right = 0%)
        let res5 = parse_clip_path_inset("inset(0% 0% 0% 83.3333%)", w, h);
        assert!(res5.is_some());
        let (cw5, ch5, cx5, cy5) = res5.unwrap();
        assert!((cw5 - 320.0).abs() < 0.1);
        assert_eq!(ch5, 1080.0);
        assert!((cx5 - 1600.0).abs() < 0.1);
        assert_eq!(cy5, 0.0);
    }
}
