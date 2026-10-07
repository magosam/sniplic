use crate::core::project::{Project, TrackType};
use crate::error::{AppError, AppResult};

/// Prevents editing operations (move, split, trim, remove) on locked tracks.
/// Non-existent tracks are not blocked here; the caller handles that case separately.
pub fn ensure_track_unlocked(project: &Project, track_id: &str) -> AppResult<()> {
    if let Some(t) = project.tracks.iter().find(|t| t.id == track_id) {
        if t.locked {
            return Err(AppError::InvalidOperation(format!(
                "Track '{}' is locked",
                t.name
            )));
        }
    }
    Ok(())
}

/// The "main" video track (fixed, never removed even when empty) is the
/// last in the video group in `project.tracks`. `add_track` always
/// inserts new video tracks BEFORE it within the group -- either at the
/// absolute top (index 0, when there is no reference track), or next
/// to an extra reference track (see `add_track`) -- so it
/// remains the last in the group in any case. See `cleanup_empty_tracks`
/// for more context.
pub fn main_video_track_id(project: &Project) -> Option<String> {
    project
        .tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Video)
        .last()
        .map(|t| t.id.clone())
}

/// The "main" audio track (fixed) is the first in the audio group --
/// `add_track` always inserts new audio tracks AFTER it within the
/// group -- either at the absolute bottom, or right after an extra reference
/// track -- so it remains the first in the group in any case.
pub fn main_audio_track_id(project: &Project) -> Option<String> {
    project
        .tracks
        .iter()
        .find(|t| t.track_type == TrackType::Audio)
        .map(|t| t.id.clone())
}
