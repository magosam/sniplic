#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ErrorStrategy, ThreadsafeFunctionCallMode};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use sniplic_core::core::project::{Project, SharedProjectState};
use sniplic_core::playback::engine::{PlaybackEngine, PlaybackTickEvent};
// Use statements for core modules (assuming typical NLE API structures based on core)
// Note: Depending on exact module signatures, we wrap them in safe async calls

#[napi]
pub struct SniplicEngine {
    project: SharedProjectState,
    playback: Arc<PlaybackEngine>,
    undo_stack: Arc<RwLock<Vec<String>>>,
    redo_stack: Arc<RwLock<Vec<String>>>,
}

#[napi]
impl SniplicEngine {
    /// Creates a new Sniplic Engine instance with a blank project.
    #[napi(constructor)]
    pub fn new(project_name: String) -> Self {
        let project = Project::new(project_name);
        Self { 
            project: Arc::new(RwLock::new(Some(project))),
            playback: Arc::new(PlaybackEngine::new()),
            undo_stack: Arc::new(RwLock::new(Vec::new())),
            redo_stack: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Saves the current project state to the undo stack.
    #[napi]
    pub async fn snapshot_history(&self) -> Result<()> {
        let lock = self.project.read().await;
        if let Some(proj) = lock.as_ref() {
            let json = serde_json::to_string(proj)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Serialization error: {}", e)))?;
            let mut u_lock = self.undo_stack.write().await;
            u_lock.push(json);
            self.redo_stack.write().await.clear();
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Reverts the project to the last saved state in the undo stack.
    #[napi]
    pub async fn undo(&self) -> Result<bool> {
        let mut u_lock = self.undo_stack.write().await;
        if let Some(last_json) = u_lock.pop() {
            let mut lock = self.project.write().await;
            if let Some(proj) = lock.as_mut() {
                // save current to redo
                let current = serde_json::to_string(proj)
                    .map_err(|e| Error::new(Status::GenericFailure, format!("Serialization error: {}", e)))?;
                self.redo_stack.write().await.push(current);
                
                // restore from last_json
                *proj = serde_json::from_str(&last_json)
                    .map_err(|e| Error::new(Status::GenericFailure, format!("Deserialization error: {}", e)))?;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Reapplies a previously undone state.
    #[napi]
    pub async fn redo(&self) -> Result<bool> {
        let mut r_lock = self.redo_stack.write().await;
        if let Some(next_json) = r_lock.pop() {
            let mut lock = self.project.write().await;
            if let Some(proj) = lock.as_mut() {
                // save current to undo
                let current = serde_json::to_string(proj)
                    .map_err(|e| Error::new(Status::GenericFailure, format!("Serialization error: {}", e)))?;
                self.undo_stack.write().await.push(current);
                
                // restore from next_json
                *proj = serde_json::from_str(&next_json)
                    .map_err(|e| Error::new(Status::GenericFailure, format!("Deserialization error: {}", e)))?;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Loads a project from a JSON file.
    #[napi]
    pub async fn load_project(&self, file_path: String) -> Result<()> {
        let loaded = Project::load_from_file(&file_path)
            .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load project: {}", e)))?;
        
        let mut lock = self.project.write().await;
        *lock = Some(loaded);
        Ok(())
    }

    /// Saves the current project to a JSON file.
    #[napi]
    pub async fn save_project(&self, file_path: String) -> Result<()> {
        let lock = self.project.read().await;
        if let Some(proj) = lock.as_ref() {
            proj.save_to_file(&file_path)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to save project: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Returns the entire project state as a serialized JSON string.
    /// This allows JavaScript to read all tracks, clips, and media without overhead.
    #[napi]
    pub async fn get_state_json(&self) -> Result<String> {
        let lock = self.project.read().await;
        if let Some(proj) = lock.as_ref() {
            serde_json::to_string(proj)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to serialize project: {}", e)))
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    // ==========================================
    // PLAYBACK & TIMELINE CONTROLS
    // ==========================================

    /// Subscribes to playback timecode events.
    /// JavaScript can pass a callback `(err, event) => {}` to update the UI playhead.
    #[napi(ts_args_type = "callback: (err: null | Error, result: { frame: number, is_playing: boolean }) => void")]
    pub fn subscribe_to_playback(&self, callback: JsFunction) -> Result<()> {
        let tsfn: ThreadsafeFunction<PlaybackTickEvent, ErrorStrategy::Fatal> = callback
            .create_threadsafe_function(0, |ctx: napi::threadsafe_function::ThreadSafeCallContext<PlaybackTickEvent>| {
                let mut obj = ctx.env.create_object()?;
                obj.set("frame", ctx.value.frame as f64)?;
                obj.set("is_playing", ctx.value.is_playing)?;
                Ok(vec![obj])
            })?;

        let mut rx = self.playback.subscribe();
        
        // Spawn a background listener that forwards Rust events to Node.js
        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                tsfn.call(event, ThreadsafeFunctionCallMode::NonBlocking);
            }
        });

        Ok(())
    }

    #[napi]
    pub async fn play(&self) -> Result<()> {
        self.playback.play().await;
        Ok(())
    }

    #[napi]
    pub async fn pause(&self) -> Result<()> {
        self.playback.pause().await;
        Ok(())
    }

    #[napi]
    pub async fn toggle_playback(&self) -> Result<()> {
        self.playback.toggle().await;
        Ok(())
    }

    #[napi]
    pub async fn seek(&self, target_frame: u32) -> Result<()> {
        self.playback.seek(target_frame as u64).await;
        Ok(())
    }

    // ==========================================
    // AI & SUBTITLES
    // ==========================================

    /// Generates subtitles from a 16kHz WAV file using the local Parakeet AI model.
    /// Returns a JSON string containing the timed words/clauses.
    #[napi]
    pub async fn generate_subtitles(&self, wav_path: String, language: String, style: String, max_chars: u32) -> Result<String> {
        let result = tokio::task::spawn_blocking(move || {
            sniplic_core::core::subtitles::parakeet_engine::transcribe_parakeet(
                std::path::Path::new(&wav_path),
                &language,
                &style,
                Some(max_chars),
            )
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("JoinError: {}", e)))?
        .map_err(|e| Error::new(Status::GenericFailure, format!("AI Transcription failed: {}", e)))?;

        let json = serde_json::to_string(&result)
            .map_err(|e| Error::new(Status::GenericFailure, format!("JSON Error: {}", e)))?;
        Ok(json)
    }

    /// Appends subtitle data (JSON array of SubtitleChunk) directly to the project.
    #[napi]
    pub async fn set_project_subtitles(&self, subtitles_json: String) -> Result<()> {
        let items: Vec<sniplic_core::core::project::subtitles::SubtitleBlock> = serde_json::from_str(&subtitles_json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid Subtitle JSON: {}", e)))?;
        
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            if let Some(subs) = &mut proj.subtitles {
                subs.items = items;
            } else {
                proj.subtitles = Some(sniplic_core::core::project::subtitles::ProjectSubtitles {
                    items,
                    ..Default::default()
                });
            }
            proj.bump_revision();
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    // ==========================================
    // ADVANCED TIMELINE & BATCH OPERATIONS
    // ==========================================

    /// Splits a clip at a specific frame and deletes the left or right side immediately.
    #[napi]
    pub async fn split_and_trim(&self, clip_id: String, split_frame: u32, is_left: bool, gapless: bool) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            sniplic_core::core::timeline::TimelineEngine::split_and_trim(
                proj,
                &clip_id,
                split_frame as u64,
                is_left,
                gapless
            ).map_err(|e| Error::new(Status::GenericFailure, format!("SplitAndTrim failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Splits the timeline precisely at the playhead across all or selected tracks.
    #[napi]
    pub async fn split_at_playhead(&self, split_frame: u32, split_all: bool, selected_clip_ids_json: String) -> Result<()> {
        let selected_ids: Vec<String> = serde_json::from_str(&selected_clip_ids_json)
            .unwrap_or_default();
            
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            sniplic_core::core::timeline::TimelineEngine::split_at_playhead(
                proj,
                split_frame as u64,
                split_all,
                &selected_ids
            ).map_err(|e| Error::new(Status::GenericFailure, format!("SplitAtPlayhead failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Adds multiple clips to the timeline in one atomic operation.
    #[napi]
    pub async fn add_clips_batch(&self, media_ids_json: String, start_frame: u32, target_track_id: Option<String>, push: bool) -> Result<()> {
        let media_ids: Vec<String> = serde_json::from_str(&media_ids_json)
            .map_err(|_| Error::new(Status::InvalidArg, "Invalid media_ids JSON array".to_string()))?;
            
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            sniplic_core::core::timeline::TimelineEngine::add_clips_batch(
                proj,
                &media_ids,
                start_frame as u64,
                target_track_id,
                push,
                None
            ).map_err(|e| Error::new(Status::GenericFailure, format!("BatchAdd failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    // ==========================================
    // EXPORT & FFMPEG
    // ==========================================

    /// Exports the current timeline to a video file with full configuration and progress reporting.
    /// This method returns immediately. The provided callback is called with progress updates, and finally with stage="done" or "error".
    #[napi(ts_args_type = "settings_json: string, on_progress: (err: null | Error, result: any) => void")]
    pub fn export_project(
        &self,
        settings_json: String,
        on_progress: JsFunction,
    ) -> Result<()> {
        let settings: sniplic_core::ffmpeg::export::types::ExportSettings = serde_json::from_str(&settings_json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid Settings JSON: {}", e)))?;

        let tsfn: ThreadsafeFunction<sniplic_core::ffmpeg::export::types::ExportProgressUpdate, ErrorStrategy::Fatal> = on_progress
            .create_threadsafe_function(0, |ctx: napi::threadsafe_function::ThreadSafeCallContext<sniplic_core::ffmpeg::export::types::ExportProgressUpdate>| {
                let mut obj = ctx.env.create_object()?;
                obj.set("percentage", ctx.value.percentage)?;
                obj.set("stage", ctx.value.stage)?;
                obj.set("message", ctx.value.message)?;
                Ok(vec![obj])
            })?;

        let proj_clone = {
            let lock = self.project.blocking_read();
            if let Some(proj) = lock.as_ref() {
                proj.clone()
            } else {
                return Err(Error::new(Status::InvalidArg, "No active project".to_string()));
            }
        };

        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        
        tokio::spawn(async move {
            let cb = Arc::new(move |prog: sniplic_core::ffmpeg::export::types::ExportProgressUpdate| {
                tsfn.call(prog, napi::threadsafe_function::ThreadsafeFunctionCallMode::NonBlocking);
            });

            let cb_clone = cb.clone();
            let res = tokio::task::spawn_blocking(move || {
                sniplic_core::ffmpeg::export::ExportEngine::render_project(
                    &proj_clone,
                    &settings,
                    Some(cb_clone),
                    cancel_flag,
                    None,
                )
            }).await;

            let final_msg = match res {
                Ok(Ok(_)) => sniplic_core::ffmpeg::export::types::ExportProgressUpdate {
                    percentage: 100.0,
                    stage: "done".to_string(),
                    message: "Export completed successfully".to_string(),
                    fps: None,
                    speed: None,
                    frame: None,
                },
                Ok(Err(e)) => sniplic_core::ffmpeg::export::types::ExportProgressUpdate {
                    percentage: 0.0,
                    stage: "error".to_string(),
                    message: format!("Export failed: {}", e),
                    fps: None,
                    speed: None,
                    frame: None,
                },
                Err(e) => sniplic_core::ffmpeg::export::types::ExportProgressUpdate {
                    percentage: 0.0,
                    stage: "error".to_string(),
                    message: format!("JoinError: {}", e),
                    fps: None,
                    speed: None,
                    frame: None,
                },
            };
            cb(final_msg);
        });

        Ok(())
    }

    /// Adds a clip to the timeline.
    #[napi]
    pub async fn add_clip(&self, track_id: String, media_id: String, start_frame: u32) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            sniplic_core::core::timeline::TimelineEngine::add_clip(
                proj,
                &track_id,
                &media_id,
                start_frame as u64,
                false, // Push
                None, // Image duration
            ).map_err(|e| Error::new(Status::GenericFailure, format!("AddClip failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Splits a clip at a specific frame.
    #[napi]
    pub async fn split_clip(&self, clip_id: String, split_frame: u32) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            sniplic_core::core::timeline::TimelineEngine::split_clip(
                proj,
                &clip_id,
                split_frame as u64,
            ).map_err(|e| Error::new(Status::GenericFailure, format!("SplitClip failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Trims a clip's edge ('left' or 'right') to a specific target frame.
    #[napi]
    pub async fn trim_clip(&self, clip_id: String, edge: String, target_frame: u32, push: bool, snap: bool, gapless: bool) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            sniplic_core::core::timeline::TimelineEngine::trim_clip(
                proj,
                &clip_id,
                &edge,
                target_frame as u64,
                push,
                snap,
                gapless,
            ).map_err(|e| Error::new(Status::GenericFailure, format!("TrimClip failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Moves a clip to a new start frame or a different track.
    #[napi]
    pub async fn move_clip(&self, clip_id: String, target_track_id: String, new_start: u32, push: bool, gapless: bool) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            sniplic_core::core::timeline::TimelineEngine::move_clip(
                proj,
                &clip_id,
                &target_track_id,
                new_start as u64,
                push,
                gapless,
            ).map_err(|e| Error::new(Status::GenericFailure, format!("MoveClip failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Removes a clip from the timeline.
    #[napi]
    pub async fn remove_clip(&self, clip_id: String, gapless: bool) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            sniplic_core::core::timeline::TimelineEngine::remove_clip(
                proj,
                &clip_id,
                gapless,
            ).map_err(|e| Error::new(Status::GenericFailure, format!("RemoveClip failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Adds media to the project's media pool manually via JSON.
    #[napi]
    pub async fn add_media(&self, media_item_json: String) -> Result<()> {
        let media_item: sniplic_core::core::project::MediaItem = serde_json::from_str(&media_item_json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid MediaItem JSON: {}", e)))?;
        
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            proj.media_pool.insert(media_item.id.clone(), media_item);
            proj.bump_revision();
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Probes a file via FFprobe, generates a MediaItem, adds it to the project, and returns the generated media_id.
    #[napi]
    pub async fn import_media(&self, file_path: String) -> Result<String> {
        // Read project settings first (FPS is needed for duration conversion)
        let target_fps = {
            let lock = self.project.read().await;
            if let Some(proj) = lock.as_ref() {
                if proj.config.fps > 0.0 { proj.config.fps } else { 30.0 }
            } else {
                return Err(Error::new(Status::InvalidArg, "No active project".to_string()));
            }
        };

        // Run ffprobe (CPU/IO bound, so we spawn blocking)
        let fp_clone = file_path.clone();
        let probe_result = tokio::task::spawn_blocking(move || {
            sniplic_core::ffmpeg::probe::probe_file(&fp_clone, target_fps)
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("JoinError: {}", e)))?
        .map_err(|e| Error::new(Status::GenericFailure, format!("Probe failed: {}", e)))?;

        // Create MediaItem
        let media_id = format!("media_{}", uuid::Uuid::new_v4().simple());
        let file_name = std::path::Path::new(&file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let media_item = sniplic_core::core::project::MediaItem {
            id: media_id.clone(),
            name: file_name,
            file_path: std::path::PathBuf::from(file_path),
            media_type: probe_result.media_type,
            duration_frames: probe_result.duration_frames,
            width: probe_result.width,
            height: probe_result.height,
            fps: probe_result.fps,
            sample_rate: probe_result.sample_rate,
            channels: probe_result.channels,
            added_at: now,
            folder_id: None,
        };

        // Insert into pool
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            proj.media_pool.insert(media_item.id.clone(), media_item);
            proj.bump_revision();
            Ok(media_id)
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Removes media from the project's media pool.
    #[napi]
    pub async fn remove_media(&self, media_id: String) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            proj.media_pool.remove(&media_id);
            proj.bump_revision();
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Adds a new track to the timeline.
    /// track_type should be "Video" or "Audio".
    #[napi]
    pub async fn add_track(&self, track_type: String, near_track_id: Option<String>) -> Result<String> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            let t_type = if track_type.eq_ignore_ascii_case("audio") {
                sniplic_core::core::project::TrackType::Audio
            } else {
                sniplic_core::core::project::TrackType::Video
            };
            
            let track = sniplic_core::core::timeline::TimelineEngine::add_track(
                proj,
                t_type,
                near_track_id.as_deref(),
            ).map_err(|e| Error::new(Status::GenericFailure, format!("AddTrack failed: {}", e)))?;
            
            Ok(track.id)
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Adds a special FX Filter track to the timeline (Adjustment Layer).
    #[napi]
    pub async fn add_filter_track(&self, near_track_id: Option<String>) -> Result<String> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            let track = sniplic_core::core::timeline::effect_tracks::EffectTrackOperations::add_filter_track(
                proj,
                near_track_id.as_deref(),
            ).map_err(|e| Error::new(Status::GenericFailure, format!("AddFilterTrack failed: {}", e)))?;
            
            Ok(track.id)
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Removes a track from the timeline.
    #[napi]
    pub async fn remove_track(&self, track_id: String) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            proj.tracks.retain(|t| t.id != track_id);
            proj.bump_revision();
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Sets the volume of a clip (0.0 to 1.0+).
    #[napi]
    pub async fn set_clip_volume(&self, clip_id: String, volume: f64) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            let mut found = false;
            for track in &mut proj.tracks {
                for clip in &mut track.clips {
                    if clip.id == clip_id {
                        clip.audio.volume = volume as f32;
                        found = true;
                        break;
                    }
                }
            }
            if found {
                proj.bump_revision();
                Ok(())
            } else {
                Err(Error::new(Status::InvalidArg, "Clip not found".to_string()))
            }
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Sets advanced audio configurations: EQ, Denoise, Fades, Volume.
    #[napi]
    pub async fn apply_audio_plugin(&self, clip_id: String, audio_json: String) -> Result<()> {
        let audio_data: serde_json::Value = serde_json::from_str(&audio_json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid Audio JSON: {}", e)))?;

        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            let mut found = false;
            for track in &mut proj.tracks {
                for clip in &mut track.clips {
                    if clip.id == clip_id {
                        if let Some(v) = audio_data.get("volume").and_then(|v| v.as_f64()) {
                            clip.audio.volume = v as f32;
                        }
                        if let Some(v) = audio_data.get("denoise").and_then(|v| v.as_f64()) {
                            clip.audio.denoise = v as f32;
                        }
                        if let Some(v) = audio_data.get("eqBass").and_then(|v| v.as_f64()) {
                            clip.audio.eq_bass = v as f32;
                        }
                        if let Some(v) = audio_data.get("eqMid").and_then(|v| v.as_f64()) {
                            clip.audio.eq_mid = v as f32;
                        }
                        if let Some(v) = audio_data.get("eqTreble").and_then(|v| v.as_f64()) {
                            clip.audio.eq_treble = v as f32;
                        }
                        if let Some(v) = audio_data.get("fadeInFrames").and_then(|v| v.as_u64()) {
                            clip.audio.fade_in_frames = v as u32;
                        }
                        if let Some(v) = audio_data.get("fadeOutFrames").and_then(|v| v.as_u64()) {
                            clip.audio.fade_out_frames = v as u32;
                        }
                        found = true;
                        break;
                    }
                }
            }
            if found {
                proj.bump_revision();
                Ok(())
            } else {
                Err(Error::new(Status::InvalidArg, "Clip not found".to_string()))
            }
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Sets the transform plugin data (JSON) of a clip.
    #[napi]
    pub async fn set_clip_transform(&self, clip_id: String, transform_json: String) -> Result<()> {
        let transform_data: serde_json::Value = serde_json::from_str(&transform_json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid Transform JSON: {}", e)))?;

        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            let mut found = false;
            for track in &mut proj.tracks {
                for clip in &mut track.clips {
                    if clip.id == clip_id {
                        clip.transform_plugin_data = Some(transform_data.clone());
                        clip.plugins.insert("transform".to_string(), transform_data.clone());
                        found = true;
                        break;
                    }
                }
            }
            if found {
                proj.bump_revision();
                Ok(())
            } else {
                Err(Error::new(Status::InvalidArg, "Clip not found".to_string()))
            }
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Sets the filter plugin data (JSON) of a clip (LUTs, Blur, etc).
    #[napi]
    pub async fn set_clip_filter(&self, clip_id: String, filter_json: String) -> Result<()> {
        let filter_data: serde_json::Value = serde_json::from_str(&filter_json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid Filter JSON: {}", e)))?;

        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            let mut found = false;
            for track in &mut proj.tracks {
                for clip in &mut track.clips {
                    if clip.id == clip_id {
                        clip.filter_plugin_data = Some(filter_data.clone());
                        clip.plugins.insert("filter".to_string(), filter_data.clone());
                        found = true;
                        break;
                    }
                }
            }
            if found {
                proj.bump_revision();
                Ok(())
            } else {
                Err(Error::new(Status::InvalidArg, "Clip not found".to_string()))
            }
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Sets the effect plugin data (JSON) of a clip (Glitch, Shake, etc).
    #[napi]
    pub async fn set_clip_effect(&self, clip_id: String, effect_json: String) -> Result<()> {
        let effect_data: serde_json::Value = serde_json::from_str(&effect_json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid Effect JSON: {}", e)))?;

        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            let mut found = false;
            for track in &mut proj.tracks {
                for clip in &mut track.clips {
                    if clip.id == clip_id {
                        clip.effect_plugin_data = Some(effect_data.clone());
                        clip.plugins.insert("effect".to_string(), effect_data.clone());
                        found = true;
                        break;
                    }
                }
            }
            if found {
                proj.bump_revision();
                Ok(())
            } else {
                Err(Error::new(Status::InvalidArg, "Clip not found".to_string()))
            }
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Sets the color adjustments data (JSON) of a clip (Brightness, Contrast, etc).
    #[napi]
    pub async fn set_clip_adjustments(&self, clip_id: String, adjustments_json: String) -> Result<()> {
        let adj_data: serde_json::Value = serde_json::from_str(&adjustments_json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid Adjustments JSON: {}", e)))?;

        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            let mut found = false;
            for track in &mut proj.tracks {
                for clip in &mut track.clips {
                    if clip.id == clip_id {
                        clip.adjustments_plugin_data = Some(adj_data.clone());
                        clip.plugins.insert("adjustments".to_string(), adj_data.clone());
                        found = true;
                        break;
                    }
                }
            }
            if found {
                proj.bump_revision();
                Ok(())
            } else {
                Err(Error::new(Status::InvalidArg, "Clip not found".to_string()))
            }
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Sets the transition plugin data (JSON) of a clip (applied between the previous clip and this one).
    #[napi]
    pub async fn set_clip_transition(&self, clip_id: String, transition_json: String) -> Result<()> {
        let transition_data: serde_json::Value = serde_json::from_str(&transition_json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid Transition JSON: {}", e)))?;

        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            let mut found = false;
            for track in &mut proj.tracks {
                for clip in &mut track.clips {
                    if clip.id == clip_id {
                        clip.transition_plugin_data = Some(transition_data.clone());
                        clip.plugins.insert("transition".to_string(), transition_data.clone());
                        found = true;
                        break;
                    }
                }
            }
            if found {
                proj.bump_revision();
                Ok(())
            } else {
                Err(Error::new(Status::InvalidArg, "Clip not found".to_string()))
            }
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }

    /// Compacts main tracks (V1 and A1) to eliminate gaps.
    #[napi]
    pub async fn compact_main_tracks(&self) -> Result<()> {
        let mut lock = self.project.write().await;
        if let Some(proj) = lock.as_mut() {
            sniplic_core::core::timeline::TimelineEngine::compact_main_tracks(proj)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Compact failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
    }
}
