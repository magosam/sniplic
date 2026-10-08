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

/// Finds the track or creates a new one visually "above" it that has no overlapping clips
/// for the given time range. For Video tracks, "above" means lower index. For Audio tracks,
/// "above" means higher index.
pub fn find_or_create_available_track(
    project: &mut Project,
    start_track_id: &str,
    start_frame: u64,
    dur: u64,
    ignore_ids: &[String],
) -> AppResult<String> {
    let start_idx = project.tracks.iter().position(|t| t.id == start_track_id)
        .ok_or_else(|| AppError::InvalidOperation("Track not found".to_string()))?;
    let track_type = project.tracks[start_idx].track_type.clone();
    let is_v = track_type == TrackType::Video;

    let mut current_idx = start_idx;
    let mut last_checked_id = start_track_id.to_string();

    loop {
        let has_overlap = project.tracks[current_idx].clips.iter().any(|c| {
            !ignore_ids.contains(&c.id) && 
            start_frame < c.start_frame + c.duration_frames && 
            start_frame + dur > c.start_frame
        });

        if !has_overlap {
            return Ok(project.tracks[current_idx].id.clone());
        }

        if is_v {
            if current_idx == 0 || project.tracks[current_idx - 1].track_type != track_type {
                let new_track = super::management::add_track(project, track_type, Some(&last_checked_id))?;
                return Ok(new_track.id);
            }
            current_idx -= 1;
        } else {
            if current_idx + 1 == project.tracks.len() || project.tracks[current_idx + 1].track_type != track_type {
                let new_track = super::management::add_track(project, track_type, Some(&last_checked_id))?;
                return Ok(new_track.id);
            }
            current_idx += 1;
        }
        last_checked_id = project.tracks[current_idx].id.clone();
    }
}
