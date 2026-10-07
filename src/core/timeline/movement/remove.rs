use crate::core::project::Project;
use crate::error::{AppError, AppResult};
use super::super::gapless;
use super::super::tracks::TrackOperations;

/// Removes a clip from the timeline. When `gapless` is true ("Gapless"
/// mode) **and** the clip's track is one of the two main tracks
/// (V1/A1), closes the gap left behind by shifting subsequent clips on the
/// same track to the left. On any extra track, the gap remains
/// as is, even when the mode is active -- extra tracks are free.
pub fn remove_clip(project: &mut Project, clip_id: &str, gapless_enabled: bool) -> AppResult<()> {
    let gapless_ids = if gapless_enabled {
        gapless::gapless_track_ids(project)
    } else {
        Default::default()
    };

    for track in &mut project.tracks {
        if let Some(pos) = track.clips.iter().position(|c| c.id == clip_id) {
            if track.locked {
                return Err(AppError::InvalidOperation(format!("Track '{}' is locked", track.name)));
            }
            let removed = track.clips.remove(pos);

            if gapless_enabled && gapless_ids.contains(&track.id) {
                let removed_end = removed.start_frame + removed.duration_frames;
                for c in track.clips.iter_mut() {
                    if c.start_frame >= removed_end {
                        c.start_frame -= removed.duration_frames;
                    }
                }
            }

            TrackOperations::cleanup_empty_tracks(project);
            return Ok(());
        }
    }
    Err(AppError::InvalidOperation("Clip not found".to_string()))
}
