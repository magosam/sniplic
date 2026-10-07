use super::*;
use serde_json::json;

#[test]
fn test_velocity_static_clip() {
    let data = json!({
        "positionXTrack": {
            "enabled": true,
            "keyframes": [
                { "frame": 0, "value": 100.0, "easing": "linear" }
            ]
        }
    });
    let vel = calculate_clip_max_velocity(Some(&data), 30.0, 1.0, 1.0, 1920.0, 1080.0);
    assert_eq!(vel, 0.0);
    assert_eq!(compute_adaptive_supersample_factor(vel, 24.0), 1);
}

#[test]
fn test_velocity_linear_motion() {
    let data = json!({
        "positionXTrack": {
            "enabled": true,
            "keyframes": [
                { "frame": 0, "value": 0.0, "easing": "linear" },
                { "frame": 30, "value": 300.0, "easing": "linear" }
            ]
        }
    });
    // 300 px in 1 second at 30 fps = 300 px/s
    let vel = calculate_clip_max_velocity(Some(&data), 30.0, 1.0, 1.0, 1920.0, 1080.0);
    assert!((vel - 300.0).abs() < 0.1);
    // 300 px/s falls into Tier 2: 4x (96 FPS at 24 FPS)
    assert_eq!(compute_adaptive_supersample_factor(vel, 24.0), 4);
}

#[test]
fn test_velocity_ease_in_out_motion() {
    let data = json!({
        "positionXTrack": {
            "enabled": true,
            "keyframes": [
                { "frame": 0, "value": 0.0, "easing": "easeInOut" },
                { "frame": 30, "value": 300.0, "easing": "easeInOut" }
            ]
        }
    });
    // 300 px in 1s with easeInOut = maximum derivative of 1.5 * 300 = 450 px/s
    let vel = calculate_clip_max_velocity(Some(&data), 30.0, 1.0, 1.0, 1920.0, 1080.0);
    assert!((vel - 450.0).abs() < 0.1);
    assert_eq!(compute_adaptive_supersample_factor(vel, 24.0), 4);
}

#[test]
fn test_velocity_fast_swipe() {
    let data = json!({
        "positionXTrack": {
            "enabled": true,
            "keyframes": [
                { "frame": 0, "value": -500.0, "easing": "easeIn" },
                { "frame": 15, "value": 500.0, "easing": "easeIn" }
            ]
        }
    });
    // 1000 px in 0.5s = average 2000 px/s * easeIn derivative (3.0) = 6000 px/s
    let vel = calculate_clip_max_velocity(Some(&data), 30.0, 1.0, 1.0, 1920.0, 1080.0);
    assert!(vel >= 6000.0);
    // Extreme velocity falls into Tier 4: 16x (384 FPS at 24 FPS)
    assert_eq!(compute_adaptive_supersample_factor(vel, 24.0), 16);
}

#[test]
fn test_shutter_weights() {
    assert_eq!(generate_shutter_weights(1), "1");
    assert_eq!(generate_shutter_weights(2), "1 1");
    assert_eq!(generate_shutter_weights(4), "1 2 2 1");
    assert_eq!(generate_shutter_weights(8), "1 2 4 8 8 4 2 1");
    assert_eq!(generate_shutter_weights(16), "1 1 2 3 5 8 13 16 16 13 8 5 3 2 1 1");
}

#[test]
fn test_nested_plugin_keyframes_detection_and_expression() {
    // Simulates nested data under "core.video.transform" (automation/plugin format)
    let nested_data = json!({
        "core.video.transform": {
            "positionXTrack": {
                "enabled": true,
                "keyframes": [
                    { "frame": 0, "value": 10.0, "easing": "linear" },
                    { "frame": 30, "value": 110.0, "easing": "linear" }
                ]
            }
        }
    });

    assert!(is_track_dynamic(Some(&nested_data), "positionXTrack"));
    let expr = build_keyframe_expr(Some(&nested_data), "positionXTrack", 0.0, 1.0, 30.0, "t");
    assert!(expr.contains("if(lte(t"));
    assert!(expr.contains("10.0000"));
    assert!(expr.contains("110.0000"));

    let vel = calculate_clip_max_velocity(Some(&nested_data), 30.0, 1.0, 1.0, 1920.0, 1080.0);
    assert!((vel - 100.0).abs() < 0.1);
}

#[test]
fn test_nested_camel_case_keyframes() {
    let data = json!({
        "transformPluginData": {
            "scaleXTrack": {
                "enabled": true,
                "keyframes": [
                    { "frame": 0, "value": 1.0, "easing": "easeInOut" },
                    { "frame": 60, "value": 2.5, "easing": "easeInOut" }
                ]
            }
        }
    });

    assert!(is_track_dynamic(Some(&data), "scaleXTrack"));
    assert_eq!(get_track_max_abs(Some(&data), "scaleXTrack", 1.0), 2.5);
}

