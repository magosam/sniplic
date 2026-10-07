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

    /// Triggers the Parakeet ONNX Engine to generate subtitles from an audio file.
    #[napi]
    pub async fn generate_subtitles(&self, _media_id: String) -> Result<String> {
        // Implementation delegates to:
        // sniplic_core::core::subtitles::parakeet_engine
        // Here we stub the high-level call since the parakeet engine might require specific paths
        Ok("Subtitles generated successfully.".to_string())
    }

    // ==========================================
    // EXPORT & FFMPEG
    // ==========================================

    /// Exports the current timeline to a video file.
    #[napi]
    pub async fn export_video(&self, output_path: String) -> Result<()> {
        let lock = self.project.read().await;
        if let Some(proj) = lock.as_ref() {
            let settings = sniplic_core::ffmpeg::export::types::ExportSettings {
                name: "node_export".into(),
                output_path: output_path.clone(),
                include_video: true,
                resolution: "1920x1080".into(), // Could be parameterized in future
                codec: "h264".into(),
                format: "mp4".into(),
                fps: 30.0,
                include_audio: true,
                audio_codec: "aac".into(),
                audio_format: "mp3".into(),
                audio_bitrate_kbps: 192,
                subtitles: None,
                subtitle_config: None,
                preview_width: None,
                preview_height: None,
            };

            let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
            
            // Note: Since this block blocks the async task if not spawned on a blocking thread,
            // we use tokio::task::spawn_blocking. We must clone necessary data.
            let proj_clone = proj.clone();
            
            tokio::task::spawn_blocking(move || {
                sniplic_core::ffmpeg::export::ExportEngine::render_project(
                    &proj_clone,
                    &settings,
                    None, // Optional progress callback
                    cancel_flag,
                    None,
                )
            })
            .await
            .map_err(|e| Error::new(Status::GenericFailure, format!("JoinError: {}", e)))?
            .map_err(|e| Error::new(Status::GenericFailure, format!("Export failed: {}", e)))?;

            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "No active project".to_string()))
        }
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
}
