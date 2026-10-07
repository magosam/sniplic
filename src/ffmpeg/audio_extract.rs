use std::fs;
use std::path::Path;
use tracing::{error, info};
use uuid::Uuid;

use crate::core::paths::clean_path_for_ffmpeg;
use crate::core::project::{MediaItem, MediaType, SharedProjectState};
use crate::ffmpeg::probe::probe_file;
use crate::media::cache::CacheManager;

pub async fn extract_audio_from_media(
    media_id: &str,
    state: &SharedProjectState,
) -> Result<MediaItem, String> {
    let (file_path, original_name, target_fps, project_id) = {
        let lock = state.read().await;
        let proj = lock.as_ref().ok_or("No active project")?;
        let media = proj
            .media_pool
            .get(media_id)
            .ok_or_else(|| format!("Media '{}' not found in project", media_id))?;

        (
            clean_path_for_ffmpeg(&media.file_path),
            media.name.clone(),
            proj.config.fps,
            proj.metadata.id.clone(),
        )
    };

    if !file_path.exists() {
        return Err(format!("Media file not found on disk: {:?}", file_path));
    }

    let audio_dir = crate::core::paths::get_project_extracted_audio_dir(&project_id);
    fs::create_dir_all(&audio_dir)
        .map_err(|e| format!("Failed to create project extracted audio directory: {}", e))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let output_filename = format!("{}_{}.m4a", media_id, timestamp);
    let output_path = clean_path_for_ffmpeg(audio_dir.join(&output_filename));

    info!(
        "Extracting audio from media '{}' to '{:?}'",
        media_id, output_path
    );

    let mut cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();

    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(&file_path)
        .args(["-vn", "-c:a", "aac", "-b:a", "192k"])
        .arg(&output_path);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute ffmpeg to extract audio: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("FFmpeg failed to extract audio: {}", stderr);
        return Err(format!("FFmpeg failed to extract audio: {}", stderr));
    }

    let probe = probe_file(&output_path, target_fps).map_err(|e| e.to_string())?;
    let new_media_id = format!("med_{}", Uuid::new_v4().simple());

    let cache = CacheManager::for_project(&project_id);
    let _ = cache.get_or_generate_waveform(&new_media_id, &output_path);

    let base_name = Path::new(&original_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&original_name);
    let audio_name = format!("{} (Audio).m4a", base_name);

    let item = MediaItem {
        id: new_media_id.clone(),
        name: audio_name,
        file_path: output_path,
        media_type: MediaType::Audio,
        duration_frames: probe.duration_frames,
        width: None,
        height: None,
        fps: probe.fps,
        sample_rate: probe.sample_rate,
        channels: probe.channels,
        added_at: timestamp,
        folder_id: None,
    };

    let mut lock = state.write().await;
    if let Some(ref mut proj) = *lock {
        proj.media_pool.insert(new_media_id, item.clone());
    }

    Ok(item)
}
