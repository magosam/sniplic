use serde_json::Value;

/// Locates a keyframe track across multiple payload formats:
/// 1. Directly at the top level: `data.get("positionXTrack")`
/// 2. Nested under the transform plugin identifier: `data.get("core.video.transform").get("positionXTrack")`
/// 3. Nested under alternative serde keys: `data.get("transformPluginData")` or `data.get("transform_plugin_data")`
pub fn get_keyframe_track<'a>(plugin_data: Option<&'a Value>, track_name: &str) -> Option<&'a Value> {
    let data = plugin_data?;
    if data.is_null() || !data.is_object() {
        return None;
    }

    // 1. Direct search at top level
    if let Some(track) = data.get(track_name) {
        if track.is_object() {
            return Some(track);
        }
    }

    // 2. Nested search under "core.video.transform"
    if let Some(nested) = data.get("core.video.transform") {
        if let Some(track) = nested.get(track_name) {
            if track.is_object() {
                return Some(track);
            }
        }
    }

    // 3. Nested search under transform plugin keys in camelCase or snake_case
    if let Some(nested) = data.get("transformPluginData").or_else(|| data.get("transform_plugin_data")) {
        if let Some(track) = nested.get(track_name) {
            if track.is_object() {
                return Some(track);
            }
        }
        if let Some(sub) = nested.get("core.video.transform") {
            if let Some(track) = sub.get(track_name) {
                if track.is_object() {
                    return Some(track);
                }
            }
        }
    }

    None
}

/// Determines if a track has active motion (at least 2 keyframes with distinct values).
pub fn is_track_dynamic(plugin_data: Option<&Value>, track_name: &str) -> bool {
    let Some(track) = get_keyframe_track(plugin_data, track_name) else {
        return false;
    };

    let enabled = track.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    if !enabled {
        return false;
    }

    if let Some(keyframes) = track.get("keyframes").and_then(|v| v.as_array()) {
        if keyframes.len() <= 1 {
            return false;
        }
        let first_val = keyframes[0].get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        keyframes.iter().any(|k| {
            let v = k.get("value").and_then(|val| val.as_f64()).unwrap_or(0.0);
            (v - first_val).abs() > 0.001
        })
    } else {
        false
    }
}

/// Gets the maximum absolute value reached by a track across its keyframes.
pub fn get_track_max_abs(plugin_data: Option<&Value>, track_name: &str, default_val: f64) -> f64 {
    let mut max_val = default_val.abs();
    if let Some(track) = get_keyframe_track(plugin_data, track_name) {
        let enabled = track.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        if enabled {
            if let Some(keyframes) = track.get("keyframes").and_then(|v| v.as_array()) {
                for k in keyframes {
                    if let Some(v) = k.get("value").and_then(|val| val.as_f64()) {
                        if v.abs() > max_val {
                            max_val = v.abs();
                        }
                    }
                }
            }
        }
    }
    max_val.max(0.01)
}
