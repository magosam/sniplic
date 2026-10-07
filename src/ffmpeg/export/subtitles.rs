use std::path::{Path, PathBuf};
use super::types::{SubtitleExportConfig, SubtitleExportItem};

pub fn generate_ass_subtitles_file(
    subtitles: &[SubtitleExportItem],
    config: &Option<SubtitleExportConfig>,
    width: u32,
    height: u32,
    fps: f64,
    _preview_height: Option<f64>,
) -> std::io::Result<PathBuf> {
    let mut temp_path = std::env::temp_dir();
    temp_path.push(format!("subtitles_{}.ass", uuid::Uuid::new_v4().simple()));

    let pos_x = config.as_ref().map(|c| c.pos_x).unwrap_or(50.0);
    let pos_y = config.as_ref().map(|c| c.pos_y).unwrap_or(82.0);
    let base_font_size = config.as_ref().map(|c| c.font_size).unwrap_or(18.0);
    let box_width_pct = config.as_ref().and_then(|c| c.box_width).unwrap_or(68.0).max(15.0).min(98.0);

    // Fine LibASS metric calibration factor:
    // In CSS, the base is the EM box. In LibASS, Fontsize is distributed over usWinAscent + usWinDescent.
    // The 1.135 factor combined with a smooth outline (4.5%) precisely balances the visual width and height
    // of text at submetric precision without peripheral bloating from thick outlines.
    const LIBASS_METRIC_COMPENSATION: f64 = 1.135;
    const BASE_CANVAS_HEIGHT: f64 = 360.0;

    let scale_factor = ((height as f64) / BASE_CANVAS_HEIGHT) * LIBASS_METRIC_COMPENSATION;
    let final_font_size = ((base_font_size as f64) * scale_factor).round().max(14.0) as u32;

    // Subtle letter spacing equivalent to CSS tracking
    let spacing = ((final_font_size as f64) * 0.015).round().max(0.0) as u32;

    // Horizontal margins so line wrapping matches the exact box width from preview
    let half_box = (box_width_pct / 2.0).min(49.0);
    let left_pct = (pos_x - half_box).max(1.0);
    let right_pct = (100.0 - (pos_x + half_box)).max(1.0);
    let margin_l = ((left_pct / 100.0) * width as f32).round() as u32;
    let margin_r = ((right_pct / 100.0) * width as f32).round() as u32;

    // MarginV from the bottom (Alignment 2 = bottom center)
    // Positions the base of the subtitle block aligning the visual center with pos_y
    let center_dist_from_bottom = ((100.0 - pos_y) / 100.0) * (height as f32);
    let margin_v = (center_dist_from_bottom - (final_font_size as f32 * 0.4)).round().max(8.0) as u32;

    // Helper to convert hexadecimal colors (#RRGGBB) to LibASS format (&HAABBGGRR)
    let hex_to_ass_color = |hex: &str, alpha_pct: f32| -> String {
        let clean = hex.trim().trim_start_matches('#');
        let (r, g, b) = if clean.len() >= 6 {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255);
            (r, g, b)
        } else {
            (255, 255, 255)
        };
        let inv_alpha = ((1.0 - (alpha_pct / 100.0).max(0.0).min(1.0)) * 255.0).round() as u8;
        format!("&H{:02X}{:02X}{:02X}{:02X}", inv_alpha, b, g, r)
    };

    let hex_to_inline_ass_color = |hex: &str| -> String {
        let clean = hex.trim().trim_start_matches('#');
        let (r, g, b) = if clean.len() >= 6 {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255);
            (r, g, b)
        } else {
            (255, 255, 255)
        };
        format!("&H{:02X}{:02X}{:02X}&", b, g, r)
    };

    let get_ass_highlight_color = |base_hex: &str| -> String {
        let clean = base_hex.trim().trim_start_matches('#');
        let (r, g, b) = if clean.len() >= 6 {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255);
            (r, g, b)
        } else {
            (255, 255, 255)
        };

        if r > 190 && g > 170 && b < 100 {
            return "&HFFE500&".to_string();
        }
        let lum = 0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32);
        if lum > 190.0 {
            return "&H00DDFF&".to_string();
        }
        if b > 180 && r < 120 {
            return "&H303BFF&".to_string();
        }
        if r > 190 && g < 130 {
            return "&HFFF000&".to_string();
        }
        if lum < 90.0 {
            return "&H00DDFF&".to_string();
        }
        let inv_r = 255 - r;
        let inv_g = 255 - g;
        let inv_b = 255 - b;
        format!("&H{:02X}{:02X}{:02X}&", inv_b, inv_g, inv_r)
    };

    let font_name = config
        .as_ref()
        .and_then(|c| c.font_family.as_deref())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Arial");

    let primary_color = config
        .as_ref()
        .and_then(|c| c.color.as_deref())
        .map(|hex| hex_to_ass_color(hex, 100.0))
        .unwrap_or_else(|| "&H0032E6FA".to_string());

    let bold_val = if config.as_ref().and_then(|c| c.bold).unwrap_or(true) { -1 } else { 0 };
    let italic_val = if config.as_ref().and_then(|c| c.italic).unwrap_or(false) { -1 } else { 0 };
    let underline_val = if config.as_ref().and_then(|c| c.underline).unwrap_or(false) { -1 } else { 0 };

    let alignment = match config.as_ref().and_then(|c| c.align_x.as_deref()) {
        Some("left") => 1,
        Some("right") => 3,
        _ => 2,
    };

    let has_border = config
        .as_ref()
        .and_then(|c| c.border_style.as_deref())
        .map(|s| s == "solid")
        .unwrap_or(false);
    let border_w = config.as_ref().and_then(|c| c.border_width).unwrap_or(2.0);
    let outline = if has_border {
        ((border_w as f64) * scale_factor * 0.35).round().max(1.0) as u32
    } else {
        ((final_font_size as f64) * 0.045).round().max(1.0) as u32
    };

    let outline_color = config
        .as_ref()
        .and_then(|c| c.border_color.as_deref())
        .map(|hex| hex_to_ass_color(hex, 100.0))
        .unwrap_or_else(|| "&H00000000".to_string());

    let bg_enabled = config.as_ref().and_then(|c| c.bg_enabled).unwrap_or(false);
    let (border_style, back_color) = if bg_enabled {
        let bg_color_hex = config.as_ref().and_then(|c| c.bg_color.as_deref()).unwrap_or("#000000");
        let bg_op = config.as_ref().and_then(|c| c.bg_opacity).unwrap_or(75.0);
        (3, hex_to_ass_color(bg_color_hex, bg_op))
    } else {
        (1, "&H80000000".to_string())
    };

    let shadow = if bg_enabled { 0 } else { ((final_font_size as f64) * 0.03).round().max(1.0) as u32 };

    let mut content = String::new();
    content.push_str("[Script Info]\n");
    content.push_str("ScriptType: v4.00+\n");
    content.push_str(&format!("PlayResX: {}\n", width));
    content.push_str(&format!("PlayResY: {}\n", height));
    content.push_str("WrapStyle: 0\n");
    content.push_str("ScaledBorderAndShadow: yes\n\n");

    content.push_str("[V4+ Styles]\n");
    content.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    content.push_str(&format!(
        "Style: Default,{},{},{},&H000000FF,{},{},{},{},{},0,100,100,{},0,{},{},{},{},{},{},{},1\n\n",
        font_name, final_font_size, primary_color, outline_color, back_color, bold_val, italic_val, underline_val, spacing, border_style, outline, shadow, alignment, margin_l, margin_r, margin_v
    ));

    content.push_str("[Events]\n");
    content.push_str("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");

    let format_ass_time = |frame: u64| -> String {
        let total_sec = frame as f64 / fps;
        let h = (total_sec / 3600.0).floor() as u64;
        let m = ((total_sec % 3600.0) / 60.0).floor() as u64;
        let s = (total_sec % 60.0).floor() as u64;
        let cs = ((total_sec % 1.0) * 100.0).floor() as u64;
        format!("{}:{:02}:{:02}.{:02}", h, m, s, cs)
    };

    let max_chars = config.as_ref().and_then(|c| c.max_chars).unwrap_or(50) as usize;

    for sub in subtitles {
        let start_str = format_ass_time(sub.start_frame);
        let end_str = format_ass_time(sub.end_frame);

        let is_unlocked = sub.is_unlocked.unwrap_or(false);
        let sub_box_w = sub.box_width.unwrap_or(box_width_pct);
        let effective_max_chars = ((sub_box_w / box_width_pct) * (max_chars as f32))
            .round()
            .max(12.0) as usize;

        let wrapped = wrap_subtitle_text(&sub.text, effective_max_chars);
        let text = wrapped.replace("\r\n", "\\N").replace('\n', "\\N");

        let mut sorted_kfs = sub.keyframes.clone().unwrap_or_default();
        sorted_kfs.sort_by_key(|k| k.frame);

        let align_str = sub.align_x.as_deref().unwrap_or("center");
        let an = match align_str {
            "left" => 1,
            "right" => 3,
            _ => 2,
        };

        let sub_pos_x = sub.pos_x.unwrap_or(pos_x);
        let sub_pos_y = sub.pos_y.unwrap_or(pos_y);

        if is_unlocked || !sorted_kfs.is_empty() || sub.animation_in.is_some() || sub.animation_out.is_some() {
            if sorted_kfs.len() >= 2 {
                let total_frames = sub.end_frame.saturating_sub(sub.start_frame);

                // 1. Segment before the first keyframe
                if sorted_kfs[0].frame > 0 {
                    let seg_start = sub.start_frame;
                    let seg_end = (sub.start_frame + sorted_kfs[0].frame).min(sub.end_frame);
                    if seg_end > seg_start {
                        let kf = &sorted_kfs[0];
                        let kf_px = ((kf.pos_x.unwrap_or(sub_pos_x) / 100.0) * (width as f32)).round() as i32;
                        let kf_py = ((kf.pos_y.unwrap_or(sub_pos_y) / 100.0) * (height as f32)).round() as i32;
                        let mut seg_tags = format!("\\an{}\\pos({},{})", an, kf_px, kf_py);
                        if let Some(ref fn_name) = sub.font_family {
                            if !fn_name.trim().is_empty() {
                                seg_tags.push_str(&format!("\\fn{}", fn_name.trim()));
                            }
                        }
                        let kf_fs = kf.font_size.or(sub.font_size);
                        if let Some(fs) = kf_fs {
                            let scaled_fs = ((fs as f64) * scale_factor).round().max(10.0) as u32;
                            seg_tags.push_str(&format!("\\fs{}", scaled_fs));
                        }
                        let kf_col = kf.color.as_deref().or(sub.color.as_deref());
                        if let Some(c) = kf_col {
                            seg_tags.push_str(&format!("\\1c{}", hex_to_inline_ass_color(c)));
                        }
                        if let Some(rot) = kf.rotation.or(sub.rotation) {
                            if rot.abs() > 0.01 {
                                seg_tags.push_str(&format!("\\frz{}", -(rot.round() as i32)));
                            }
                        }
                        if let Some(op) = kf.opacity {
                            let inv_alpha = ((1.0 - (op / 100.0).max(0.0).min(1.0)) * 255.0).round() as u8;
                            seg_tags.push_str(&format!("\\1a&H{:02X}&", inv_alpha));
                        }
                        content.push_str(&format!(
                            "Dialogue: 0,{},{},Default,,0,0,0,,{{{}}}{}\n",
                            format_ass_time(seg_start),
                            format_ass_time(seg_end),
                            seg_tags,
                            text
                        ));
                    }
                }

                // 2. Interpolation between each pair of keyframes
                for i in 0..(sorted_kfs.len() - 1) {
                    let kf1 = &sorted_kfs[i];
                    let kf2 = &sorted_kfs[i + 1];
                    let seg_start = (sub.start_frame + kf1.frame).min(sub.end_frame);
                    let seg_end = (sub.start_frame + kf2.frame).min(sub.end_frame);
                    if seg_end <= seg_start {
                        continue;
                    }

                    let dur_ms = (((seg_end - seg_start) as f64 / fps) * 1000.0).round().max(1.0) as u64;

                    let x1 = ((kf1.pos_x.unwrap_or(sub_pos_x) / 100.0) * (width as f32)).round() as i32;
                    let y1 = ((kf1.pos_y.unwrap_or(sub_pos_y) / 100.0) * (height as f32)).round() as i32;
                    let x2 = ((kf2.pos_x.unwrap_or(sub_pos_x) / 100.0) * (width as f32)).round() as i32;
                    let y2 = ((kf2.pos_y.unwrap_or(sub_pos_y) / 100.0) * (height as f32)).round() as i32;

                    let mut seg_tags = format!("\\an{}\\move({},{},{},{},0,{})", an, x1, y1, x2, y2, dur_ms);

                    if let Some(ref fn_name) = sub.font_family {
                        if !fn_name.trim().is_empty() {
                            seg_tags.push_str(&format!("\\fn{}", fn_name.trim()));
                        }
                    }

                    let fs1 = kf1.font_size.or(sub.font_size);
                    let fs2 = kf2.font_size.or(sub.font_size);
                    if let (Some(f1), Some(f2)) = (fs1, fs2) {
                        let s_f1 = ((f1 as f64) * scale_factor).round().max(10.0) as u32;
                        let s_f2 = ((f2 as f64) * scale_factor).round().max(10.0) as u32;
                        seg_tags.push_str(&format!("\\fs{}\\t(0,{},\\fs{})", s_f1, dur_ms, s_f2));
                    }

                    let r1 = kf1.rotation.or(sub.rotation).unwrap_or(0.0);
                    let r2 = kf2.rotation.or(sub.rotation).unwrap_or(0.0);
                    if r1.abs() > 0.01 || r2.abs() > 0.01 {
                        seg_tags.push_str(&format!("\\frz{}\\t(0,{},\\frz{})", -(r1.round() as i32), dur_ms, -(r2.round() as i32)));
                    }

                    let c1 = kf1.color.as_deref().or(sub.color.as_deref());
                    let c2 = kf2.color.as_deref().or(sub.color.as_deref());
                    if let (Some(col1), Some(col2)) = (c1, c2) {
                        seg_tags.push_str(&format!("\\1c{}\\t(0,{},\\1c{})", hex_to_inline_ass_color(col1), dur_ms, hex_to_inline_ass_color(col2)));
                    }

                    if let (Some(op1), Some(op2)) = (kf1.opacity, kf2.opacity) {
                        let a1 = ((1.0 - (op1 / 100.0).max(0.0).min(1.0)) * 255.0).round() as u8;
                        let a2 = ((1.0 - (op2 / 100.0).max(0.0).min(1.0)) * 255.0).round() as u8;
                        seg_tags.push_str(&format!("\\1a&H{:02X}&\\t(0,{},\\1a&H{:02X}&)", a1, dur_ms, a2));
                    }

                    content.push_str(&format!(
                        "Dialogue: 0,{},{},Default,,0,0,0,,{{{}}}{}\n",
                        format_ass_time(seg_start),
                        format_ass_time(seg_end),
                        seg_tags,
                        text
                    ));
                }

                // 3. Segment after the last keyframe
                let last_kf = sorted_kfs.last().unwrap();
                if last_kf.frame < total_frames {
                    let seg_start = sub.start_frame + last_kf.frame;
                    let seg_end = sub.end_frame;
                    if seg_end > seg_start {
                        let kf_px = ((last_kf.pos_x.unwrap_or(sub_pos_x) / 100.0) * (width as f32)).round() as i32;
                        let kf_py = ((last_kf.pos_y.unwrap_or(sub_pos_y) / 100.0) * (height as f32)).round() as i32;
                        let mut seg_tags = format!("\\an{}\\pos({},{})", an, kf_px, kf_py);
                        if let Some(ref fn_name) = sub.font_family {
                            if !fn_name.trim().is_empty() {
                                seg_tags.push_str(&format!("\\fn{}", fn_name.trim()));
                            }
                        }
                        let kf_fs = last_kf.font_size.or(sub.font_size);
                        if let Some(fs) = kf_fs {
                            let scaled_fs = ((fs as f64) * scale_factor).round().max(10.0) as u32;
                            seg_tags.push_str(&format!("\\fs{}", scaled_fs));
                        }
                        let kf_col = last_kf.color.as_deref().or(sub.color.as_deref());
                        if let Some(c) = kf_col {
                            seg_tags.push_str(&format!("\\1c{}", hex_to_inline_ass_color(c)));
                        }
                        if let Some(rot) = last_kf.rotation.or(sub.rotation) {
                            if rot.abs() > 0.01 {
                                seg_tags.push_str(&format!("\\frz{}", -(rot.round() as i32)));
                            }
                        }
                        if let Some(op) = last_kf.opacity {
                            let inv_alpha = ((1.0 - (op / 100.0).max(0.0).min(1.0)) * 255.0).round() as u8;
                            seg_tags.push_str(&format!("\\1a&H{:02X}&", inv_alpha));
                        }
                        content.push_str(&format!(
                            "Dialogue: 0,{},{},Default,,0,0,0,,{{{}}}{}\n",
                            format_ass_time(seg_start),
                            format_ass_time(seg_end),
                            seg_tags,
                            text
                        ));
                    }
                }
            } else {
                // No multiple keyframes (or 1 fixed keyframe) - apply styling and animations
                let mut tags = String::new();
                tags.push_str(&format!("\\an{}", an));

                let px = ((sub_pos_x / 100.0) * (width as f32)).round() as i32;
                let py = ((sub_pos_y / 100.0) * (height as f32)).round() as i32;

                let dur_ms = ((sub.animation_duration.unwrap_or(0.3) as f64) * 1000.0).round().max(50.0) as u64;
                let in_fade = if sub.animation_in.as_deref() == Some("fade") { dur_ms } else { 0 };
                let out_fade = if sub.animation_out.as_deref() == Some("fade") { dur_ms } else { 0 };

                if in_fade > 0 || out_fade > 0 {
                    tags.push_str(&format!("\\fad({},{})", in_fade, out_fade));
                }

                if sub.animation_in.as_deref() == Some("pop") {
                    tags.push_str(&format!("\\fscx70\\fscy70\\t(0,{},\\fscx100\\fscy100)", dur_ms));
                    if in_fade == 0 {
                        tags.push_str(&format!("\\fad({},{})", dur_ms, out_fade));
                    }
                }

                let slide_offset = ((25.0 / BASE_CANVAS_HEIGHT) * (height as f64)).round() as i32;
                if sub.animation_in.as_deref() == Some("slide_up") {
                    tags.push_str(&format!("\\move({},{},{},{},0,{})", px, py + slide_offset, px, py, dur_ms));
                    if in_fade == 0 && out_fade == 0 {
                        tags.push_str(&format!("\\fad({},0)", dur_ms));
                    }
                } else if sub.animation_in.as_deref() == Some("slide_down") {
                    tags.push_str(&format!("\\move({},{},{},{},0,{})", px, py - slide_offset, px, py, dur_ms));
                    if in_fade == 0 && out_fade == 0 {
                        tags.push_str(&format!("\\fad({},0)", dur_ms));
                    }
                } else {
                    tags.push_str(&format!("\\pos({},{})", px, py));
                }

                // Typography and Size
                if let Some(ref fn_name) = sub.font_family {
                    if !fn_name.trim().is_empty() {
                        tags.push_str(&format!("\\fn{}", fn_name.trim()));
                    }
                }

                if let Some(fs) = sub.font_size {
                    let scaled_fs = ((fs as f64) * scale_factor).round().max(10.0) as u32;
                    tags.push_str(&format!("\\fs{}", scaled_fs));
                }

                // Colors
                if let Some(ref c) = sub.color {
                    tags.push_str(&format!("\\1c{}", hex_to_inline_ass_color(c)));
                }

                // Borders
                let has_custom_border = sub.border_style.as_deref() == Some("solid");
                if has_custom_border {
                    let bw = sub.border_width.unwrap_or(2.0);
                    let outline_px = ((bw as f64) * scale_factor * 0.35).round().max(1.0) as u32;
                    let bc = sub.border_color.as_deref().unwrap_or("#000000");
                    tags.push_str(&format!("\\bord{}\\3c{}", outline_px, hex_to_inline_ass_color(bc)));
                } else if sub.border_style.as_deref() == Some("none") {
                    tags.push_str("\\bord0");
                }

                // Bold, Italic, Underline
                if let Some(b) = sub.bold {
                    tags.push_str(if b { "\\b1" } else { "\\b0" });
                }
                if let Some(i) = sub.italic {
                    tags.push_str(if i { "\\i1" } else { "\\i0" });
                }
                if let Some(u) = sub.underline {
                    tags.push_str(if u { "\\u1" } else { "\\u0" });
                }

                // Rotation (CSS clockwise = ASS counter-clockwise)
                if let Some(rot) = sub.rotation {
                    if rot.abs() > 0.01 {
                        tags.push_str(&format!("\\frz{}", -(rot.round() as i32)));
                    }
                }

                // Letter spacing
                if let Some(sp) = sub.letter_spacing {
                    if sp.abs() > 0.01 {
                        let sp_scaled = ((sp as f64) * scale_factor * 0.2).round() as i32;
                        tags.push_str(&format!("\\fsp{}", sp_scaled));
                    }
                }

                let words_batch_enabled = sub.words_batch_enabled
                    .or_else(|| config.as_ref().and_then(|c| c.words_batch_enabled))
                    .unwrap_or(false);

                let words_per_batch = sub.words_per_batch
                    .or_else(|| config.as_ref().and_then(|c| c.words_per_batch))
                    .unwrap_or(5) as usize;

                let is_single_line = sub.single_line_enabled
                    .or_else(|| config.as_ref().and_then(|c| c.single_line_enabled))
                    .unwrap_or(false);

                if is_single_line {
                    tags.push_str("\\q2");
                }

                let is_anim_cap_active = sub.animated_captions_enabled
                    .or_else(|| config.as_ref().and_then(|c| c.animated_captions_enabled))
                    .unwrap_or(false);
                let anim_cap_style = sub.animated_captions_style.as_deref()
                    .or_else(|| config.as_ref().and_then(|c| c.animated_captions_style.as_deref()))
                    .unwrap_or("scale");

                let raw_words: Vec<&str> = text.split_whitespace().collect();
                let is_ai_sub = sub.words.as_ref().map(|w| !w.is_empty()).unwrap_or(false)
                    || sub.generated_by_ai.unwrap_or(false);
                let should_window_or_animate = (is_anim_cap_active || (is_ai_sub && words_batch_enabled) || is_single_line) && raw_words.len() > 1;

                if should_window_or_animate {
                    let total_dur = sub.end_frame.saturating_sub(sub.start_frame).max(1);
                    let num_w = raw_words.len();
                    let base_hex = sub.color.as_deref()
                        .or_else(|| config.as_ref().and_then(|c| c.color.as_deref()))
                        .unwrap_or("#fae632");
                    let base_ass_color = hex_to_inline_ass_color(base_hex);
                    let hl_color = get_ass_highlight_color(base_hex);
                    let batch_size = if words_batch_enabled { words_per_batch.max(1) } else { num_w };

                    for w_idx in 0..num_w {
                        let (w_start_frame, w_end_frame) = if let Some(ref w_items) = sub.words {
                            if let Some(wi) = w_items.get(w_idx) {
                                (wi.start_frame.max(sub.start_frame), wi.end_frame.min(sub.end_frame).max(wi.start_frame + 1))
                            } else {
                                (
                                    sub.start_frame + ((w_idx as u64) * total_dur) / (num_w as u64),
                                    sub.start_frame + (((w_idx as u64 + 1) * total_dur) / (num_w as u64)),
                                )
                            }
                        } else {
                            (
                                sub.start_frame + ((w_idx as u64) * total_dur) / (num_w as u64),
                                sub.start_frame + (((w_idx as u64 + 1) * total_dur) / (num_w as u64)),
                            )
                        };
                        let w_start_str = format_ass_time(w_start_frame);
                        let w_end_str = format_ass_time(w_end_frame);

                        let (start_i, end_i) = if words_batch_enabled {
                            let batch_idx = w_idx / batch_size;
                            let s = batch_idx * batch_size;
                            let e = (s + batch_size - 1).min(num_w - 1);
                            (s, e)
                        } else {
                            (0, num_w - 1)
                        };

                        let mut animated_text = String::new();
                        for i in start_i..=end_i {
                            let word = raw_words[i];
                            if i > start_i {
                                animated_text.push(' ');
                            }
                            if is_anim_cap_active && i == w_idx {
                                match anim_cap_style {
                                    "scale" => {
                                        animated_text.push_str(&format!("{{\\fscx118\\fscy118}}{}{{\\fscx100\\fscy100}}", word));
                                    }
                                    "color" => {
                                        animated_text.push_str(&format!("{{\\c{}}}{}{{\\c{}}}", hl_color, word, base_ass_color));
                                    }
                                    _ => {
                                        animated_text.push_str(&format!("{{\\fscx118\\fscy118\\c{}}}{}{{\\fscx100\\fscy100\\c{}}}", hl_color, word, base_ass_color));
                                    }
                                }
                            } else {
                                animated_text.push_str(word);
                            }
                        }

                        content.push_str(&format!(
                            "Dialogue: 0,{},{},Default,,0,0,0,,{{{}}}{}\n",
                            w_start_str, w_end_str, tags, animated_text
                        ));
                    }
                } else {
                    content.push_str(&format!(
                        "Dialogue: 0,{},{},Default,,0,0,0,,{{{}}}{}\n",
                        start_str, end_str, tags, text
                    ));
                }
            }
        } else {
            let is_anim_cap_active = sub.animated_captions_enabled
                .or_else(|| config.as_ref().and_then(|c| c.animated_captions_enabled))
                .unwrap_or(false);
            let anim_cap_style = sub.animated_captions_style.as_deref()
                .or_else(|| config.as_ref().and_then(|c| c.animated_captions_style.as_deref()))
                .unwrap_or("scale");

            let raw_words: Vec<&str> = text.split_whitespace().collect();
            if is_anim_cap_active && raw_words.len() > 1 {
                let total_dur = sub.end_frame.saturating_sub(sub.start_frame).max(1);
                let num_w = raw_words.len();
                let base_hex = sub.color.as_deref()
                    .or_else(|| config.as_ref().and_then(|c| c.color.as_deref()))
                    .unwrap_or("#fae632");
                let base_ass_color = hex_to_inline_ass_color(base_hex);
                let hl_color = get_ass_highlight_color(base_hex);

                for w_idx in 0..num_w {
                    let (w_start_frame, w_end_frame) = if let Some(ref w_items) = sub.words {
                        if let Some(wi) = w_items.get(w_idx) {
                            (wi.start_frame.max(sub.start_frame), wi.end_frame.min(sub.end_frame).max(wi.start_frame + 1))
                        } else {
                            (
                                sub.start_frame + ((w_idx as u64) * total_dur) / (num_w as u64),
                                sub.start_frame + (((w_idx as u64 + 1) * total_dur) / (num_w as u64)),
                            )
                        }
                    } else {
                        (
                            sub.start_frame + ((w_idx as u64) * total_dur) / (num_w as u64),
                            sub.start_frame + (((w_idx as u64 + 1) * total_dur) / (num_w as u64)),
                        )
                    };
                    let w_start_str = format_ass_time(w_start_frame);
                    let w_end_str = format_ass_time(w_end_frame);

                    let mut animated_text = String::new();
                    for (i, word) in raw_words.iter().enumerate() {
                        if i > 0 {
                            animated_text.push(' ');
                        }
                        if i == w_idx {
                            match anim_cap_style {
                                "scale" => {
                                    animated_text.push_str(&format!("{{\\fscx118\\fscy118}}{}{{\\fscx100\\fscy100}}", word));
                                }
                                "color" => {
                                    animated_text.push_str(&format!("{{\\c{}}}{}{{\\c{}}}", hl_color, word, base_ass_color));
                                }
                                _ => {
                                    animated_text.push_str(&format!("{{\\fscx118\\fscy118\\c{}}}{}{{\\fscx100\\fscy100\\c{}}}", hl_color, word, base_ass_color));
                                }
                            }
                        } else {
                            animated_text.push_str(word);
                        }
                    }

                    content.push_str(&format!(
                        "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
                        w_start_str, w_end_str, animated_text
                    ));
                }
            } else {
                content.push_str(&format!(
                    "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
                    start_str, end_str, text
                ));
            }
        }
    }

    std::fs::write(&temp_path, content)?;
    Ok(temp_path)
}

fn wrap_subtitle_text(text: &str, max_chars: usize) -> String {
    if text.is_empty() || max_chars == 0 || text.contains('\n') || text.contains("\\N") {
        return text.to_string();
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= 1 {
        return text.to_string();
    }

    let total_len = text.chars().count();
    if total_len <= max_chars {
        return text.to_string();
    }

    // Try balancing across 2 lines if it fits
    let target_lines = (total_len as f64 / max_chars as f64).ceil().max(2.0) as usize;
    if target_lines == 2 {
        let mid = total_len / 2;
        let mut best_split = None;
        let mut best_diff = usize::MAX;
        let mut cur_len = 0;

        for i in 0..(words.len() - 1) {
            cur_len += words[i].chars().count();
            let l1 = words[..=i].join(" ");
            let l2 = words[(i + 1)..].join(" ");

            if l1.chars().count() <= max_chars && l2.chars().count() <= max_chars {
                let diff = if cur_len > mid { cur_len - mid } else { mid - cur_len };
                if diff < best_diff {
                    best_diff = diff;
                    best_split = Some(i + 1);
                }
            }
            cur_len += 1;
        }

        if let Some(split_idx) = best_split {
            let l1 = words[..split_idx].join(" ");
            let l2 = words[split_idx..].join(" ");
            return format!("{}\n{}", l1, l2);
        }
    }

    // Progressive word wrapping
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in words {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.chars().count() + 1 + word.chars().count() <= max_chars {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines.join("\n")
}

pub fn escape_ffmpeg_filter_path(path: &Path) -> String {
    let path_str = path.to_string_lossy().replace('\\', "/");
    path_str.replace(':', "\\:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_ass_with_custom_styles() {
        let items = vec![SubtitleExportItem {
            start_frame: 0,
            end_frame: 30,
            text: "Teste de Legenda".to_string(),
            ..Default::default()
        }];
        let config = SubtitleExportConfig {
            pos_x: 50.0,
            pos_y: 80.0,
            font_size: 20.0,
            box_width: Some(70.0),
            max_chars: Some(50),
            style: Some("standard".to_string()),
            font_family: Some("Roboto".to_string()),
            color: Some("#ffffff".to_string()),
            bold: Some(false),
            italic: Some(true),
            underline: Some(false),
            align_x: Some("left".to_string()),
            border_style: Some("solid".to_string()),
            border_color: Some("#ff0000".to_string()),
            border_width: Some(3.0),
            bg_enabled: Some(true),
            bg_color: Some("#000000".to_string()),
            bg_opacity: Some(80.0),
            ..Default::default()
        };

        let res = generate_ass_subtitles_file(&items, &Some(config), 1920, 1080, 30.0, None);
        assert!(res.is_ok());
        let path = res.unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(path);

        assert!(content.contains("Roboto"));
        assert!(content.contains("&H00FFFFFF")); // Pure white
        assert!(content.contains("&H000000FF")); // Red BGR: &H000000FF
        assert!(content.contains("Dialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,Teste de Legenda"));
    }

    #[test]
    fn test_generate_ass_with_animated_captions() {
        let items = vec![SubtitleExportItem {
            start_frame: 0,
            end_frame: 60,
            text: "Palavra Um Dois Tres".to_string(),
            animated_captions_enabled: Some(true),
            animated_captions_style: Some("scale".to_string()),
            ..Default::default()
        }];
        let config = SubtitleExportConfig {
            animated_captions_enabled: Some(true),
            animated_captions_style: Some("scale".to_string()),
            ..Default::default()
        };

        let res = generate_ass_subtitles_file(&items, &Some(config), 1920, 1080, 30.0, None);
        assert!(res.is_ok());
        let path = res.unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(path);

        // Must contain multiple Dialogue lines sliced per word with \\fscx118
        assert!(content.contains("\\fscx118\\fscy118"));
    }
}
