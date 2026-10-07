use serde_json::Value;

use super::track_reader::get_keyframe_track;

pub fn build_keyframe_expr(
    plugin_data: Option<&Value>,
    track_name: &str,
    default_val: f64,
    scale_multiplier: f64,
    fps: f64,
    time_var: &str,
) -> String {
    let Some(track) = get_keyframe_track(plugin_data, track_name) else {
        return format!("{:.4}", default_val * scale_multiplier);
    };

    let enabled = track.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    if !enabled {
        return format!("{:.4}", default_val * scale_multiplier);
    }

    let Some(raw_keyframes) = track.get("keyframes").and_then(|v| v.as_array()) else {
        return format!("{:.4}", default_val * scale_multiplier);
    };

    if raw_keyframes.is_empty() {
        return format!("{:.4}", default_val * scale_multiplier);
    }

    struct Kf {
        time_sec: f64,
        value: f64,
        easing: String,
    }

    let mut keyframes: Vec<Kf> = raw_keyframes
        .iter()
        .filter_map(|k| {
            let frame = k.get("frame").and_then(|v| v.as_i64())?;
            let value = k.get("value").and_then(|v| v.as_f64())?;
            let easing = k
                .get("easing")
                .and_then(|v| v.as_str())
                .unwrap_or("linear")
                .to_string();
            Some(Kf {
                time_sec: (frame as f64) / fps,
                value: value * scale_multiplier,
                easing,
            })
        })
        .collect();

    if keyframes.is_empty() {
        return format!("{:.4}", default_val * scale_multiplier);
    }

    keyframes.sort_by(|a, b| a.time_sec.partial_cmp(&b.time_sec).unwrap_or(std::cmp::Ordering::Equal));

    if keyframes.len() == 1 {
        return format!("{:.4}", keyframes[0].value);
    }

    let n = keyframes.len();
    let v_first = keyframes[0].value;
    let t_first = keyframes[0].time_sec;
    let v_last = keyframes[n - 1].value;

    let mut current_expr = format!("{:.4}", v_last);

    for i in (0..n - 1).rev() {
        let kf_a = &keyframes[i];
        let kf_b = &keyframes[i + 1];
        let span = kf_b.time_sec - kf_a.time_sec;

        let seg_val = if span <= 0.000001 {
            format!("{:.4}", kf_a.value)
        } else {
            let raw_u = format!("clip(({time_var}-{:.6})/{:.6},0.0,1.0)", kf_a.time_sec, span);
            let diff = kf_b.value - kf_a.value;
            match kf_a.easing.as_str() {
                "easeIn" => {
                    format!("({:.4}+({:.4})*({raw_u}*{raw_u}*{raw_u}))", kf_a.value, diff)
                }
                "easeOut" => {
                    format!("({:.4}+({:.4})*(1.0-(1.0-{raw_u})*(1.0-{raw_u})*(1.0-{raw_u})))", kf_a.value, diff)
                }
                "easeInOut" => {
                    // Continuous cubic smoothstep (3u^2 - 2u^3): continuous derivative without break at the midpoint
                    format!(
                        "({:.4}+({:.4})*({raw_u}*{raw_u}*(3.0-2.0*{raw_u})))",
                        kf_a.value, diff
                    )
                }
                _ => {
                    format!("({:.4}+({:.4})*{raw_u})", kf_a.value, diff)
                }
            }
        };

        current_expr = format!(
            "if(lte({time_var},{:.6}),{seg_val},{current_expr})",
            kf_b.time_sec
        );
    }

    format!("if(lte({time_var},{:.6}),{:.4},{current_expr})", t_first, v_first)
}
