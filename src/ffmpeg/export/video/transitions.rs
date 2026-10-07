use crate::core::project::Clip;

#[derive(Debug, Clone)]
pub struct TransitionConfig {
    pub transition_id: String,
    pub ffmpeg_type: String,
    pub duration_frames: u64,
}

pub fn extract_transition_config(clip: &Clip) -> Option<TransitionConfig> {
    let data = clip.transition_plugin_data.as_ref()
        .filter(|v| !v.is_null() && v.is_object())
        .or_else(|| {
            clip.transform_plugin_data.as_ref().and_then(|t| {
                t.get("core.transitions.basic")
                    .filter(|v| !v.is_null() && v.is_object())
                    .or_else(|| {
                        if t.get("transitionId").or_else(|| t.get("transition_id")).is_some() {
                            Some(t)
                        } else {
                            None
                        }
                    })
            })
        })?;

    let transition_id = data.get("transitionId")
        .or_else(|| data.get("transition_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if transition_id.is_empty() || transition_id == "none" {
        return None;
    }

    let duration_frames = data.get("durationFrames")
        .or_else(|| data.get("duration_frames"))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_f64().map(|f| f.max(1.0).round() as u64))
                .or_else(|| v.as_i64().map(|i| i.max(1) as u64))
        })
        .unwrap_or(24);

    let explicit_ffmpeg = data.get("ffmpegTransition")
        .or_else(|| data.get("ffmpeg_transition"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let norm_id = transition_id.trim().to_lowercase();

    let mapped_type = match norm_id.as_str() {
        // Basic Transitions (canonical frontend IDs and legacy variations)
        "basic.fade" | "fade" => "fade",
        "basic.fade.black" | "fadeblack" => "fadeblack",
        "basic.fade.white" | "fadewhite" => "fadewhite",
        "basic.fade.grays" | "fadegrays" => "fadegrays",
        "basic.dissolve" | "dissolve" => "dissolve",
        "basic.slide.up" | "slideup" => "slideup",
        "basic.slide.down" | "slidedown" => "slidedown",
        "basic.slide.left" | "slideleft" => "slideleft",
        "basic.slide.right" | "slideright" => "slideright",
        "basic.smooth.up" | "smoothup" => "smoothup",
        "basic.smooth.down" | "smoothdown" => "smoothdown",
        "basic.smooth.left" | "smoothleft" => "smoothleft",
        "basic.smooth.right" | "smoothright" => "smoothright",
        "basic.wipe.left" | "wipeleft" => "wipeleft",
        "basic.wipe.right" | "wiperight" => "wiperight",
        "basic.wipe.up" | "wipeup" => "wipeup",
        "basic.wipe.down" | "wipedown" => "wipedown",
        "basic.wipe.tl" | "wipetl" => "wipetl",
        "basic.wipe.tr" | "wipetr" => "wipetr",
        "basic.wipe.bl" | "wipebl" => "wipebl",
        "basic.wipe.br" | "wipebr" => "wipebr",
        "basic.diag.tl" | "diagtl" => "diagtl",
        "basic.circle.crop" | "circlecrop" => "circlecrop",
        "basic.circle.open" | "circleopen" => "circleopen",
        "basic.circle.close" | "circleclose" => "circleclose",
        "basic.rect.crop" | "rectcrop" => "rectcrop",
        "basic.vert.open" | "vertopen" => "vertopen",
        "basic.vert.close" | "vertclose" => "vertclose",
        "basic.horz.open" | "horzopen" => "horzopen",
        "basic.horz.close" | "horzclose" => "horzclose",
        "basic.radial" | "radial" => "radial",
        "basic.zoom.in" | "zoomin" => "zoomin",
        "basic.pixelize" | "pixelize" => "pixelize",
        "basic.hblur" | "hblur" => "hblur",
        "basic.distance" => "zoomin",
        "basic.squeeze.h" | "squeezeh" => "squeezeh",
        "basic.squeeze.v" | "squeezev" => "squeezev",
        "basic.hlslice" | "basic.hl.slice" | "hlslice" => "hlslice",
        "basic.hrslice" | "basic.hr.slice" | "hrslice" => "hrslice",
        "basic.vuslice" | "basic.vu.slice" | "vuslice" => "vuslice",
        "basic.vdslice" | "basic.vd.slice" | "vdslice" => "vdslice",

        // GL Built-in: Shapes & Geometry
        "gl.shape.diamond" | "gl.diamond" | "diamond" => "rectcrop",
        "gl.shape.circle" | "gl.circle" | "circle" => "circlecrop",
        "gl.shape.radial" => "radial",
        "gl.shape.polka_dots" | "gl.shape.polka" | "polka_dots" | "polka" => "circleclose",
        "gl.shape.heart" | "heart" => "circlecrop",
        "gl.shape.window_blinds" | "gl.shape.blinds" | "window_blinds" | "blinds" => "vertopen",
        "gl.shape.pinwheel" | "pinwheel" => "radial",

        // GL Built-in: Warps & Distortions
        "gl.warp.crosszoom" | "crosszoom" => "zoomin",
        "gl.warp.ripple" | "ripple" => "radial",
        "gl.warp.swirl" | "swirl" => "radial",
        "gl.warp.liquid" | "liquid" => "dissolve",
        "gl.warp.wind" | "wind" => "hlwind",
        "gl.warp.water_drop" | "gl.warp.waterdrop" | "water_drop" | "waterdrop" => "circleopen",
        "gl.warp.bounce" | "bounce" => "slidedown",

        // GL Built-in: Glitch & Digital
        "gl.glitch.digital" | "gl.glitch.glitch" | "glitch" => "pixelize",
        "gl.glitch.rgb_split" | "gl.glitch.rgbsplit" | "rgb_split" | "rgbsplit" => "pixelize",
        "gl.glitch.tv_static" | "gl.glitch.tvstatic" | "tv_static" | "tvstatic" => "pixelize",

        // GL Built-in: Artistic & Organic
        "gl.artistic.film_burn" | "gl.film_burn" | "gl.art.burn" | "film_burn" | "burn" => "fadewhite",
        "gl.artistic.dreamy" | "gl.art.dreamy" | "dreamy" => "fadewhite",
        "gl.artistic.pagecurl" | "gl.art.pagecurl" | "pagecurl" => "wipetl",
        "gl.artistic.flyeye" | "gl.art.flyeye" | "flyeye" => "radial",
        "gl.artistic.kaleidoscope" | "gl.art.kaleidoscope" | "kaleidoscope" => "radial",
        "gl.artistic.hexagonalize" | "gl.art.hexagonalize" | "hexagonalize" => "pixelize",
        "gl.artistic.mosaic" | "gl.art.mosaic" | "mosaic" => "pixelize",
        "gl.artistic.doom_screen" | "gl.3d.doomscreen" | "doom_screen" | "doomscreen" => "wipedown",

        // GL Built-in: 3D Perspective
        "gl.threed.cube" | "gl.3d.cube" | "cube" => "smoothleft",
        "gl.threed.doorway" | "gl.3d.doorway" | "doorway" => "horzopen",
        "gl.threed.grid_flip" | "gl.3d.gridflip" | "grid_flip" | "gridflip" => "vertopen",

        // GL Community Transitions
        "gl.advanced_mosaic" | "gl.mosaic_transition" => "pixelize",
        "gl.angular" => "radial",
        "gl.block_dissolve" => "dissolve",
        "gl.book_flip" => "smoothleft",
        "gl.bow_tie_horizontal" | "gl.bow_tie_vertical" | "gl.bow_tie_with_parameter" => "circlecrop",
        "gl.box" => "smoothleft",
        "gl.burn0" | "gl.undulating_burn_out" => "fadewhite",
        "gl.butterfly_wave_scrawler" => "dissolve",
        "gl.cannabisleaf" => "circlecrop",
        "gl.chessboard" => "rectcrop",
        "gl.circle_crop" => "circlecrop",
        "gl.circleopen" => "circleopen",
        "gl.fade" => "fade",
        "gl.dissolve" => "dissolve",
        "gl.colorphase" | "gl.fadecolor" | "gl.fadegrayscale" | "gl.hsvfade" => "fadegrays",
        "gl.colour_distance" | "gl.coord_from_in" | "gl.crazy_parametric_fun" => "dissolve",
        "gl.crosshatch" | "gl.crosswarp" | "gl.displacement" => "dissolve",
        "gl.defocus_blur" | "gl.linear_blur" | "gl.tangent_motion_blur" => "hblur",
        "gl.directional" | "gl.directional_easing" | "gl.directional_scaled" | "gl.directionalwarp" => "slideleft",
        "gl.directionalwipe" => "wipeleft",
        "gl.dreamy_zoom" => "zoomin",
        "gl.drop_zone_flicker" | "gl.edge_transition" | "gl.fragment" => "dissolve",
        "gl.fold" => "smoothleft",
        "gl.glitch_displace" | "gl.glitch_memories" | "gl.strip_datamosh_glitch" | "gl.parametric_glitch" | "gl.old_tv_lost_signal" => "pixelize",
        "gl.horizontal_close" => "horzclose",
        "gl.horizontal_open" => "horzopen",
        "gl.vertical_close" => "vertclose",
        "gl.vertical_open" => "vertopen",
        "gl.left_right" => "slideleft",
        "gl.top_bottom" => "slidedown",
        "gl.luma" | "gl.luminance_melt" | "gl.morph" | "gl.multiply_blend" | "gl.perlin" => "dissolve",
        "gl.overexposure" => "fadewhite",
        "gl.polar_function" | "gl.power_kaleido" => "radial",
        "gl.puzzle_right" => "slideright",
        "gl.random_noisex" => "pixelize",
        "gl.randomsquares" | "gl.squareswire" | "gl.rectangle" | "gl.rectangle_crop" => "rectcrop",
        "gl.revolve_left" => "smoothleft",
        "gl.rolls" => "slidedown",
        "gl.rotate_scale_fade" | "gl.rotate_scale_vanish" | "gl.rotate_transition" => "smoothleft",
        "gl.scale_in" => "zoomin",
        "gl.simple_flip" => "vertopen",
        "gl.simple_zoom" | "gl.simple_zoom_out" | "gl.zoom_in_circles" | "gl.zoom_in_out" | "gl.zoom_left_wipe" | "gl.zoom_rigth_wipe" => "zoomin",
        "gl.slides" => "slideleft",
        "gl.split_slide_in_horizontal" | "gl.split_slide_in_out_horizontal" | "gl.split_slide_out_horizontal" => "slideleft",
        "gl.split_slide_in_vertical" | "gl.split_slide_in_out_vertical" | "gl.split_slide_out_vertical" => "slidedown",
        "gl.squeeze" => "squeezeh",
        "gl.star_wipe" => "circlecrop",
        "gl.static_fade" => "fadegrays",
        "gl.static_wipe" => "wipeleft",
        "gl.stereo_viewer" | "gl.tiles_wave" => "dissolve",
        "gl.swap" => "smoothleft",
        "gl.windowslice" => "hlslice",
        "gl.wipe_down" => "wipedown",
        "gl.wipe_left" => "wipeleft",
        "gl.wipe_right" => "wiperight",
        "gl.wipe_up" => "wipeup",
        "gl.x_axis_translation" => "slideleft",
        _ => "",
    };

    let ffmpeg_type = if !mapped_type.is_empty() {
        mapped_type.to_string()
    } else if !explicit_ffmpeg.is_empty() && explicit_ffmpeg != "fade" {
        explicit_ffmpeg.to_string()
    } else if explicit_ffmpeg == "fade" && norm_id.contains("fade") {
        "fade".to_string()
    } else {
        // Intelligent semantic fallback for custom or uncataloged IDs
        if norm_id.contains("zoom") {
            "zoomin".to_string()
        } else if norm_id.contains("burn") {
            "fadewhite".to_string()
        } else if norm_id.contains("blur") {
            "hblur".to_string()
        } else if norm_id.contains("pixel") || norm_id.contains("glitch") || norm_id.contains("mosaic") {
            "pixelize".to_string()
        } else if norm_id.contains("circle") || norm_id.contains("heart") || norm_id.contains("star") {
            "circlecrop".to_string()
        } else if norm_id.contains("radial") || norm_id.contains("swirl") || norm_id.contains("ripple") || norm_id.contains("kaleid") || norm_id.contains("pinwheel") {
            "radial".to_string()
        } else if norm_id.contains("horz") || norm_id.contains("horizontal") {
            if norm_id.contains("close") { "horzclose".to_string() } else { "horzopen".to_string() }
        } else if norm_id.contains("vert") || norm_id.contains("vertical") {
            if norm_id.contains("close") { "vertclose".to_string() } else { "vertopen".to_string() }
        } else if norm_id.contains("slice") {
            "hlslice".to_string()
        } else if norm_id.contains("squeeze") {
            "squeezeh".to_string()
        } else if norm_id.contains("wipe") {
            if norm_id.contains("up") {
                "wipeup".to_string()
            } else if norm_id.contains("down") {
                "wipedown".to_string()
            } else if norm_id.contains("right") {
                "wiperight".to_string()
            } else {
                "wipeleft".to_string()
            }
        } else if norm_id.contains("slide") {
            if norm_id.contains("up") {
                "slideup".to_string()
            } else if norm_id.contains("down") {
                "slidedown".to_string()
            } else if norm_id.contains("right") {
                "slideright".to_string()
            } else {
                "slideleft".to_string()
            }
        } else if norm_id.contains("smooth") || norm_id.contains("flip") || norm_id.contains("cube") || norm_id.contains("rotate") {
            if norm_id.contains("up") {
                "smoothup".to_string()
            } else if norm_id.contains("down") {
                "smoothdown".to_string()
            } else if norm_id.contains("right") {
                "smoothright".to_string()
            } else {
                "smoothleft".to_string()
            }
        } else if norm_id.contains("dissolve") || norm_id.contains("liquid") || norm_id.contains("morph") || norm_id.contains("melt") {
            "dissolve".to_string()
        } else if norm_id.contains("white") {
            "fadewhite".to_string()
        } else if norm_id.contains("black") {
            "fadeblack".to_string()
        } else if norm_id.contains("gray") {
            "fadegrays".to_string()
        } else {
            "fade".to_string()
        }
    };

    Some(TransitionConfig {
        transition_id,
        ffmpeg_type,
        duration_frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_test_clip(transition_data: serde_json::Value) -> Clip {
        serde_json::from_value(json!({
            "id": "clip_test",
            "mediaId": "media_1",
            "name": "clip_test",
            "start_frame": 0,
            "duration_frames": 60,
            "in_point_frames": 0,
            "out_point_frames": 60,
            "transition_plugin_data": transition_data
        })).unwrap()
    }

    #[test]
    fn test_extract_basic_transitions() {
        let clip = make_test_clip(json!({
            "transitionId": "basic.hlslice",
            "ffmpegTransition": "hlslice",
            "durationFrames": 30
        }));
        let cfg = extract_transition_config(&clip).expect("Should extract basic transition");
        assert_eq!(cfg.ffmpeg_type, "hlslice");
        assert_eq!(cfg.duration_frames, 30);

        let clip_slide = make_test_clip(json!({
            "transitionId": "basic.slide.up",
            "durationFrames": 24
        }));
        let cfg_slide = extract_transition_config(&clip_slide).unwrap();
        assert_eq!(cfg_slide.ffmpeg_type, "slideup");
    }

    #[test]
    fn test_extract_gl_builtin_transitions_no_unwanted_fade() {
        // Digital Glitch had ffmpegTransition: "fade" in frontend
        let clip_glitch = make_test_clip(json!({
            "transitionId": "gl.glitch.digital",
            "ffmpegTransition": "fade",
            "durationFrames": 20
        }));
        let cfg_glitch = extract_transition_config(&clip_glitch).unwrap();
        assert_eq!(cfg_glitch.ffmpeg_type, "pixelize", "Glitch must not become fade");

        // Film burn
        let clip_burn = make_test_clip(json!({
            "transitionId": "gl.artistic.film_burn",
            "ffmpegTransition": "fade",
            "durationFrames": 25
        }));
        let cfg_burn = extract_transition_config(&clip_burn).unwrap();
        assert_eq!(cfg_burn.ffmpeg_type, "fadewhite");

        // Crosszoom
        let clip_zoom = make_test_clip(json!({
            "transitionId": "gl.warp.crosszoom",
            "ffmpegTransition": "fade"
        }));
        let cfg_zoom = extract_transition_config(&clip_zoom).unwrap();
        assert_eq!(cfg_zoom.ffmpeg_type, "zoomin");

        // 3D Cube
        let clip_cube = make_test_clip(json!({
            "transitionId": "gl.threed.cube"
        }));
        let cfg_cube = extract_transition_config(&clip_cube).unwrap();
        assert_eq!(cfg_cube.ffmpeg_type, "smoothleft");
    }

    #[test]
    fn test_extract_community_gl_transitions() {
        let clip_mosaic = make_test_clip(json!({
            "transitionId": "gl.advanced_mosaic",
            "ffmpegTransition": "fade"
        }));
        let cfg_mosaic = extract_transition_config(&clip_mosaic).unwrap();
        assert_eq!(cfg_mosaic.ffmpeg_type, "pixelize");

        let clip_horz = make_test_clip(json!({
            "transitionId": "gl.horizontal_close"
        }));
        let cfg_horz = extract_transition_config(&clip_horz).unwrap();
        assert_eq!(cfg_horz.ffmpeg_type, "horzclose");
    }
}
