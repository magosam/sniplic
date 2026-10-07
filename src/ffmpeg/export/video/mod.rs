use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::core::project::{MediaType, Project, TrackType};
use crate::error::AppResult;

use super::hwaccel;
use super::keyframes::{calculate_clip_max_velocity, compute_adaptive_supersample_factor};
use super::types::{AudioRenderItem, ExportProgressUpdate, ExportSettings};

pub mod audio_pipeline;
pub mod filter_builder;
pub mod runner;
pub mod transitions;

pub use transitions::TransitionConfig;

pub fn render_video_pipeline(
    project: &Project,
    settings: &ExportSettings,
    target_path: &str,
    audio_items: &[AudioRenderItem],
    width: u32,
    height: u32,
    fps: f64,
    total_duration_sec: f64,
    progress_callback: Option<Arc<dyn Fn(ExportProgressUpdate) + Send + Sync>>,
    cancel_flag: Arc<AtomicBool>,
    on_pid: Option<Arc<dyn Fn(u32) + Send + Sync>>,
) -> AppResult<()> {
    if let Some(ref cb) = progress_callback {
        cb(ExportProgressUpdate {
            percentage: 0.0,
            stage: "preparing".to_string(),
            message: "Preparing video...".to_string(),
            fps: None,
            frame: None,
            speed: None,
        });
    }

    let mut visual_clips = Vec::new();
    // Tracks from bottom (V1) to top (V2, V3...)
    for track in project.tracks.iter().rev() {
        if track.track_type == TrackType::Video && !track.hidden {
            for clip in &track.clips {
                if clip.is_compound.unwrap_or(false) {
                    if let Some(ref children) = clip.compound_clips {
                        for child in children {
                            if let Some(media) = project.media_pool.get(&child.media_id) {
                                if media.media_type != MediaType::Audio {
                                    let mut flattened = child.clone();
                                    flattened.start_frame = clip.start_frame + child.start_frame;
                                    visual_clips.push((flattened, media.clone()));
                                }
                            }
                        }
                    }
                } else if let Some(media) = project.media_pool.get(&clip.media_id) {
                    visual_clips.push((clip.clone(), media.clone()));
                }
            }
        }
    }

    // Collect active filter and effect (FX) tracks on timeline
    let mut fx_clips = Vec::new();
    for track in &project.tracks {
        if (track.track_type == TrackType::Filter || track.is_filter()) && !track.hidden {
            for clip in &track.clips {
                fx_clips.push(clip.clone());
            }
        }
    }
    fx_clips.sort_by_key(|c| c.start_frame);

    let proj_w = (project.config.width as f64).max(1.0);
    let proj_h = (project.config.height as f64).max(1.0);
    let project_fps = if project.config.fps > 0.0 { project.config.fps } else { 30.0 };
    let export_fps = fps;
    let scale_factor_x = width as f64 / proj_w;
    let scale_factor_y = height as f64 / proj_h;

    let mut clip_factors: Vec<u32> = Vec::with_capacity(visual_clips.len());
    for (clip, _) in &visual_clips {
        let max_vel = calculate_clip_max_velocity(
            clip.transform_plugin_data.as_ref(),
            project_fps,
            scale_factor_x,
            scale_factor_y,
            width as f64,
            height as f64,
        );
        let factor = compute_adaptive_supersample_factor(max_vel, export_fps);
        clip_factors.push(factor);
    }

    let clip_id_to_index: HashMap<String, usize> = visual_clips
        .iter()
        .enumerate()
        .map(|(i, (c, _))| (c.id.clone(), i))
        .collect();

    let mut incoming_trans: HashMap<String, TransitionConfig> = HashMap::new();
    let mut outgoing_trans: HashMap<String, TransitionConfig> = HashMap::new();
    let mut cut_transitions: Vec<(String, String, TransitionConfig)> = Vec::new();

    for track in &project.tracks {
        if track.track_type != TrackType::Video || track.hidden {
            continue;
        }
        let mut sorted_clips = track.clips.clone();
        sorted_clips.sort_by_key(|c| c.start_frame);

        for idx in 0..sorted_clips.len().saturating_sub(1) {
            let left_clip = &sorted_clips[idx];
            let right_clip = &sorted_clips[idx + 1];

            let left_end = left_clip.start_frame + left_clip.duration_frames;
            let right_start = right_clip.start_frame;

            if (right_start as i64 - left_end as i64).abs() <= 2 {
                if clip_id_to_index.contains_key(&left_clip.id) && clip_id_to_index.contains_key(&right_clip.id) {
                    // In the canonical project model, the cut transition is recorded exclusively on right_clip.
                    // We do not query left_clip to prevent subsequent cuts without transitions from inadvertently inheriting previous transitions.
                    if let Some(cfg) = transitions::extract_transition_config(right_clip) {
                        outgoing_trans.insert(left_clip.id.clone(), cfg.clone());
                        incoming_trans.insert(right_clip.id.clone(), cfg.clone());
                        cut_transitions.push((left_clip.id.clone(), right_clip.id.clone(), cfg));
                    }
                }
            }
        }
    }

    // 1. Build video filter_complex
    if let Some(ref cb) = progress_callback {
        cb(ExportProgressUpdate {
            percentage: 0.0,
            stage: "preparing".to_string(),
            message: "Preparing composition and subtitles...".to_string(),
            fps: None,
            frame: None,
            speed: None,
        });
    }

    let (video_filters, final_v_label, temp_ass_file, temp_lut_files) = filter_builder::build_video_filter_complex(
        &visual_clips,
        &fx_clips,
        &clip_factors,
        &incoming_trans,
        &outgoing_trans,
        &cut_transitions,
        &clip_id_to_index,
        width,
        height,
        export_fps,
        project_fps,
        total_duration_sec,
        scale_factor_x,
        scale_factor_y,
        settings,
    );

    // 2. Build audio filter_complex
    let visual_count = visual_clips.len();
    let (audio_filters, final_a_label) = audio_pipeline::build_audio_filter_complex(
        audio_items,
        visual_count,
        project_fps,
        total_duration_sec,
    );

    if let Some(ref cb) = progress_callback {
        cb(ExportProgressUpdate {
            percentage: 0.0,
            stage: "starting".to_string(),
            message: "Starting rendering...".to_string(),
            fps: None,
            frame: None,
            speed: None,
        });
    }

    let full_filter_complex = format!("{}{}", video_filters, audio_filters);

    let temp_script_path = std::env::temp_dir().join(format!("sniplic_video_filter_{}.txt", uuid::Uuid::new_v4().simple()));
    if let Err(e) = std::fs::write(&temp_script_path, &full_filter_complex) {
        return Err(crate::error::AppError::Io(e));
    }

    let mut temp_files = Vec::new();
    if let Some(ref p) = temp_ass_file {
        temp_files.push(p.clone());
    }
    temp_files.extend(temp_lut_files);
    temp_files.push(temp_script_path.clone());

    let is_hw = hwaccel::is_hardware_encoder_used(settings.codec.as_str());
    let initial_video_enc_args = hwaccel::build_video_encoder_args(settings.codec.as_str(), width, height, export_fps);

    let build_cmd = |video_enc_args: &[String]| -> std::process::Command {
        let mut cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
        cmd.args(["-hide_banner", "-y"]);

        // Video and image inputs with adaptive margin to accommodate transitions
        for (idx, (clip, media)) in visual_clips.iter().enumerate() {
            let in_tc = incoming_trans.get(&clip.id);
            let out_tc = outgoing_trans.get(&clip.id);

            let in_half_dur = in_tc.map(|t| (t.duration_frames as f64 / project_fps) / 2.0).unwrap_or(0.0);
            let out_half_dur = out_tc.map(|t| (t.duration_frames as f64 / project_fps) / 2.0).unwrap_or(0.0);

            let clip_start_sec = clip.start_frame as f64 / project_fps;
            let clip_dur_sec = clip.duration_frames as f64 / project_fps;
            let clip_end_sec = clip_start_sec + clip_dur_sec;

            let eff_start_sec = (clip_start_sec - in_half_dur).max(0.0);
            let eff_end_sec = clip_end_sec + out_half_dur;
            let eff_dur_sec = eff_end_sec - eff_start_sec;

            let in_sec = clip.in_point_frames as f64 / project_fps;
            let factor = clip_factors[idx];
            let clip_fps = export_fps * (factor as f64);

            if media.media_type == MediaType::Image {
                cmd.args([
                    "-loop", "1",
                    "-framerate", &format!("{:.4}", clip_fps),
                    "-t", &format!("{:.6}", eff_dur_sec + 2.0),
                    "-i",
                ])
                .arg(&media.file_path);
            } else {
                let speed = clip.speed.unwrap_or(1.0);
                let in_offset = clip_start_sec - eff_start_sec;
                let eff_in_sec = (in_sec - in_offset * speed).max(0.0);
                let eff_source_dur_sec = eff_dur_sec * speed;
                let extra_margin = (1.0 * speed).max(1.0);

                let is_webm = media
                    .file_path
                    .extension()
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("webm"));
                if is_webm {
                    cmd.args(["-c:v", "libvpx-vp9"]);
                }

                cmd.args([
                    "-ss",
                    &format!("{:.6}", eff_in_sec),
                    "-t",
                    &format!("{:.6}", eff_source_dur_sec + extra_margin),
                    "-i",
                ])
                .arg(&media.file_path);
            }
        }

        // Timeline audio inputs
        for item in audio_items {
            let in_sec = item.in_point_frames as f64 / project_fps;
            let dur_sec = (item.duration_frames as f64 / project_fps) * item.speed;
            cmd.args([
                "-ss",
                &format!("{:.6}", in_sec),
                "-t",
                &format!("{:.6}", dur_sec + 0.5),
                "-i",
            ])
            .arg(&item.file_path);
        }

        let filter_flag = crate::ffmpeg::export::get_filter_complex_script_flag();
        cmd.args([filter_flag, temp_script_path.to_string_lossy().as_ref()]);
        cmd.args(["-map", &final_v_label]);
        cmd.args(["-map", &final_a_label]);

        cmd.args(video_enc_args);
        cmd.args(["-r", &format!("{:.4}", export_fps), "-fps_mode", "cfr"]);

        let is_webm = settings.format.eq_ignore_ascii_case("webm") || target_path.to_lowercase().ends_with(".webm");
        let acodec = if is_webm { "libopus" } else { "aac" };
        cmd.args(["-c:a", acodec, "-b:a", &format!("{}k", settings.audio_bitrate_kbps)]);
        cmd.args(["-t", &format!("{:.4}", total_duration_sec)]);
        cmd.arg(target_path);

        cmd
    };

    let first_cmd = build_cmd(&initial_video_enc_args);

    let export_result = runner::run_ffmpeg_export(
        first_cmd,
        target_path,
        total_duration_sec,
        Vec::new(),
        cancel_flag.clone(),
        progress_callback.clone(),
        on_pid.clone(),
    );

    let final_result = match export_result {
        Ok(()) => Ok(()),
        Err(err) => {
            if is_hw && !cancel_flag.load(Ordering::SeqCst) {
                tracing::warn!(
                    "Hardware-accelerated encoding failed ({}). Activating automatic fallback to high-compatibility CPU encoder...",
                    err
                );

                if let Some(ref cb) = progress_callback {
                    cb(ExportProgressUpdate {
                        percentage: 0.0,
                        stage: "fallback".to_string(),
                        message: "Switching to high-compatibility software encoder...".to_string(),
                        fps: None,
                        frame: None,
                        speed: None,
                    });
                }

                let fallback_video_enc_args = hwaccel::build_software_encoder_args(settings.codec.as_str(), width, height, export_fps);
                let fallback_cmd = build_cmd(&fallback_video_enc_args);

                runner::run_ffmpeg_export(
                    fallback_cmd,
                    target_path,
                    total_duration_sec,
                    Vec::new(),
                    cancel_flag,
                    progress_callback,
                    on_pid,
                )
            } else {
                Err(err)
            }
        }
    };

    // Remove temporary script and subtitle files
    for path in &temp_files {
        let _ = std::fs::remove_file(path);
    }

    final_result
}
