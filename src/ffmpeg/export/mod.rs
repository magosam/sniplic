pub mod types;
pub mod keyframes;
pub mod subtitles;
pub mod video;
pub mod audio;
pub mod hwaccel;

use std::path::PathBuf;
use std::sync::Arc;
use crate::error::{AppError, AppResult};
use crate::core::project::{Project, MediaType, TrackType};

pub use types::{ExportSettings, SubtitleExportConfig, SubtitleExportItem, TextKeyframeExportItem, AudioRenderItem, ExportProgressUpdate};
pub use keyframes::{build_keyframe_expr, is_track_dynamic};
pub use subtitles::{generate_ass_subtitles_file, escape_ffmpeg_filter_path};

/// Determines the correct flag to load filter_complex from a temporary file.
/// In modern FFmpeg (v8+/v9+), the legacy flag '-filter_complex_script' was removed in favor of '-/filter_complex'.
pub fn get_filter_complex_script_flag() -> &'static str {
    static FLAG: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    *FLAG.get_or_init(|| {
        let mut test_cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
        let temp_file = std::env::temp_dir().join(format!("sniplic_probe_fc_{}.txt", uuid::Uuid::new_v4().simple()));
        let _ = std::fs::write(&temp_file, "nullsrc=s=16x16:d=0.04[v]");
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            test_cmd.creation_flags(0x08000000);
        }
        test_cmd.args([
            "-hide_banner",
            "-v", "error",
            "-/filter_complex",
            temp_file.to_string_lossy().as_ref(),
            "-map", "[v]",
            "-f", "null",
            "-",
        ]);
        let supports_dash_slash = test_cmd.status().map(|s| s.success()).unwrap_or(false);
        let _ = std::fs::remove_file(&temp_file);

        if supports_dash_slash {
            tracing::info!("FFmpeg supports modern syntax '-/filter_complex'");
            "-/filter_complex"
        } else {
            tracing::info!("Legacy FFmpeg: using fallback '-filter_complex_script'");
            "-filter_complex_script"
        }
    })
}

pub struct ExportEngine;

impl ExportEngine {
    pub fn render_project(
        project: &Project,
        settings: &ExportSettings,
        progress_callback: Option<Arc<dyn Fn(ExportProgressUpdate) + Send + Sync>>,
        cancel_flag: Arc<std::sync::atomic::AtomicBool>,
        on_pid: Option<Arc<dyn Fn(u32) + Send + Sync>>,
    ) -> AppResult<Vec<String>> {
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(AppError::InvalidOperation("Export cancelled by user.".into()));
        }

        if let Some(ref cb) = progress_callback {
            cb(ExportProgressUpdate {
                percentage: 0.0,
                stage: "preparing".to_string(),
                message: "Validating timeline...".to_string(),
                fps: None,
                frame: None,
                speed: None,
            });
        }

        if !settings.include_video && !settings.include_audio {
            return Err(AppError::InvalidOperation(
                "Select at least Video or Audio to export.".into(),
            ));
        }

        let (width, height) = Self::parse_resolution(&settings.resolution)?;
        let project_fps = if project.config.fps > 0.0 { project.config.fps } else { 30.0 };
        let export_fps = if settings.fps > 0.0 { settings.fps } else { project_fps };

        // 1. Total duration calculated in timeline frames (excluding hidden video tracks and muted audio)
        let mut total_duration_frames: u64 = 0;
        for track in &project.tracks {
            if track.hidden || (track.track_type == TrackType::Audio && track.muted) {
                continue;
            }
            for clip in &track.clips {
                let end = clip.start_frame + clip.duration_frames;
                if end > total_duration_frames {
                    total_duration_frames = end;
                }
            }
        }

        if total_duration_frames == 0 {
            return Err(AppError::InvalidOperation(
                "The timeline has no active media to export (tracks are hidden or empty).".into(),
            ));
        }

        // Actual duration in seconds is calculated based on the project timeline frame rate
        let total_duration_sec = total_duration_frames as f64 / project_fps;

        // 2. Collect audio sources (audio tracks + embedded audio in videos; ignores muted and hidden tracks)
        let mut audio_items: Vec<AudioRenderItem> = Vec::new();
        for track in &project.tracks {
            if track.muted || track.hidden {
                continue;
            }
            for clip in &track.clips {
                if clip.is_compound.unwrap_or(false) {
                    if let Some(ref children) = clip.compound_clips {
                        for child in children {
                            if child.audio.mute || child.audio.volume <= 0.0 {
                                continue;
                            }
                            if let Some(media) = project.media_pool.get(&child.media_id) {
                                let has_audio = match media.media_type {
                                    MediaType::Audio => true,
                                    MediaType::Video => media.channels.unwrap_or(0) > 0 || media.sample_rate.is_some(),
                                    MediaType::Image => false,
                                };

                                if has_audio {
                                    let effective_audio_path = child
                                        .audio
                                        .processed_audio_path
                                        .as_ref()
                                        .map(std::path::PathBuf::from)
                                        .filter(|p| p.exists())
                                        .unwrap_or_else(|| media.file_path.clone());

                                    audio_items.push(AudioRenderItem {
                                        file_path: effective_audio_path,
                                        start_frame: clip.start_frame + child.start_frame,
                                        in_point_frames: child.in_point_frames,
                                        duration_frames: child.duration_frames,
                                        volume: child.audio.volume,
                                        reversed: child.reversed.unwrap_or(false),
                                        fade_in_frames: child.audio.fade_in_frames,
                                        fade_out_frames: child.audio.fade_out_frames,
                                        speed: child.speed.unwrap_or(1.0),
                                    });
                                }
                            }
                        }
                    }
                    continue;
                }

                if clip.audio.mute || clip.audio.volume <= 0.0 {
                    continue;
                }
                if let Some(media) = project.media_pool.get(&clip.media_id) {
                    let has_audio = match media.media_type {
                        MediaType::Audio => true,
                        MediaType::Video => media.channels.unwrap_or(0) > 0 || media.sample_rate.is_some(),
                        MediaType::Image => false,
                    };

                    if has_audio {
                        let effective_audio_path = clip
                            .audio
                            .processed_audio_path
                            .as_ref()
                            .map(std::path::PathBuf::from)
                            .filter(|p| p.exists())
                            .unwrap_or_else(|| media.file_path.clone());

                        audio_items.push(AudioRenderItem {
                            file_path: effective_audio_path,
                            start_frame: clip.start_frame,
                            in_point_frames: clip.in_point_frames,
                            duration_frames: clip.duration_frames,
                            volume: clip.audio.volume,
                            reversed: clip.reversed.unwrap_or(false),
                            fade_in_frames: clip.audio.fade_in_frames,
                            fade_out_frames: clip.audio.fade_out_frames,
                            speed: clip.speed.unwrap_or(1.0),
                        });
                    }
                }
            }
        }

        let mut exported_files: Vec<String> = Vec::new();
        let base_path = PathBuf::from(&settings.output_path);
        let parent_dir = base_path.parent().unwrap_or_else(|| std::path::Path::new(""));
        let stem = base_path.file_stem().and_then(|s| s.to_str()).unwrap_or("video");

        // 3. Video rendering (includes composition and multiplexed timeline audio)
        if settings.include_video {
            let video_path = parent_dir
                .join(format!("{}.{}", stem, settings.format))
                .to_string_lossy()
                .to_string();

            if let Err(e) = video::render_video_pipeline(
                project,
                settings,
                &video_path,
                &audio_items,
                width,
                height,
                export_fps,
                total_duration_sec,
                progress_callback,
                cancel_flag.clone(),
                on_pid.clone(),
            ) {
                for f in &exported_files {
                    let _ = std::fs::remove_file(f);
                }
                return Err(e);
            }

            exported_files.push(video_path);
        }

        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            for f in &exported_files {
                let _ = std::fs::remove_file(f);
            }
            return Err(AppError::InvalidOperation("Export cancelled by user.".into()));
        }

        // 4. Standalone Audio Rendering
        if settings.include_audio {
            let audio_ext = if settings.audio_format.is_empty() { "mp3" } else { &settings.audio_format };
            let audio_filename = if settings.include_video {
                format!("{}_audio.{}", stem, audio_ext)
            } else {
                format!("{}.{}", stem, audio_ext)
            };

            let audio_path = parent_dir
                .join(audio_filename)
                .to_string_lossy()
                .to_string();

            if let Err(e) = audio::render_audio_pipeline(
                settings,
                &audio_path,
                &audio_items,
                project_fps,
                total_duration_sec,
                cancel_flag.clone(),
                on_pid.clone(),
            ) {
                for f in &exported_files {
                    let _ = std::fs::remove_file(f);
                }
                return Err(e);
            }

            exported_files.push(audio_path);
        }

        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            for f in &exported_files {
                let _ = std::fs::remove_file(f);
            }
            return Err(AppError::InvalidOperation("Export cancelled by user.".into()));
        }

        Ok(exported_files)
    }

    pub fn parse_resolution(res: &str) -> AppResult<(u32, u32)> {
        let parts: Vec<&str> = res.split('x').collect();
        if parts.len() == 2 {
            let w = parts[0].parse::<u32>().unwrap_or(1920);
            let h = parts[1].parse::<u32>().unwrap_or(1080);
            return Ok((w, h));
        }
        Ok((1920, 1080))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::project::Project;

    #[test]
    #[ignore = "Slow integration test requiring project_autosave.json media"]
    fn test_debug_export() {
        let path = ".editor_cache/project_autosave.json";
        if !std::path::Path::new(path).exists() {
            println!("project_autosave.json does not exist at {}", path);
            return;
        }
        let content = std::fs::read_to_string(path).unwrap();
        let project: Project = serde_json::from_str(&content).unwrap();

        let temp_out = std::env::temp_dir().join("test_export_out.mp4").to_string_lossy().to_string();
        let settings = ExportSettings {
            name: "test_export".into(),
            output_path: temp_out,
            include_video: true,
            resolution: "1920x1080".into(),
            codec: "h264".into(),
            format: "mp4".into(),
            fps: 30.0,
            include_audio: false,
            audio_codec: "aac".into(),
            audio_format: "mp3".into(),
            audio_bitrate_kbps: 192,
            subtitles: None,
            subtitle_config: None,
            preview_width: Some(640.0),
            preview_height: Some(360.0),
        };

        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let start = std::time::Instant::now();
        let cb = Arc::new(move |up: ExportProgressUpdate| {
            println!("[{:.2}s] {:.1}% - {} (fps: {:?}, frame: {:?}, speed: {:?})", start.elapsed().as_secs_f64(), up.percentage, up.message, up.fps, up.frame, up.speed);
        });
        match ExportEngine::render_project(&project, &settings, Some(cb), cancel_flag, None) {
            Ok(files) => println!("SUCCESS! Exported files: {:?}", files),
            Err(e) => panic!("EXPORT FAILED: {:?}", e),
        }
    }
}
