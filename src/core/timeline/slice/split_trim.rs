use crate::core::project::{Clip, Project};
use crate::error::{AppError, AppResult};
use super::super::gapless;
use super::super::tracks::TrackOperations;
use super::split::split_clip;

pub fn split_and_trim(
    project: &mut Project,
    clip_id: &str,
    split_frame: u64,
    is_left: bool,
    gapless_enabled: bool,
) -> AppResult<Clip> {
    let (first, second) = split_clip(project, clip_id, split_frame)?;
    let to_remove = if is_left { first.id.clone() } else { second.id.clone() };
    let to_keep = if is_left { second.id.clone() } else { first.id.clone() };

    for track in &mut project.tracks {
        if let Some(pos) = track.clips.iter().position(|c| c.id == to_remove) {
            track.clips.remove(pos);
            break;
        }
    }

    let kept = project
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.id == to_keep)
        .cloned()
        .ok_or_else(|| AppError::InvalidOperation("Failed to retrieve clip".to_string()))?;

    if gapless_enabled {
        // Only closes the gap on tracks where "Gapless" is permitted to
        // act (main V1/A1) -- extra tracks keep their clips in
        // place even if the discarded segment was on them or before
        // them on the timeline.
        let gapless_ids = gapless::gapless_track_ids(project);

        if is_left {
            let removed_dur = first.duration_frames;
            for track in &mut project.tracks {
                if !gapless_ids.contains(&track.id) {
                    continue;
                }
                for c in &mut track.clips {
                    if c.start_frame >= split_frame {
                        c.start_frame = c.start_frame.saturating_sub(removed_dur);
                    }
                }
            }
        } else {
            let removed_dur = second.duration_frames;
            let original_end = split_frame + removed_dur;
            for track in &mut project.tracks {
                if !gapless_ids.contains(&track.id) {
                    continue;
                }
                for c in &mut track.clips {
                    if c.start_frame >= original_end {
                        c.start_frame = c.start_frame.saturating_sub(removed_dur);
                    }
                }
            }
        }
    }

    TrackOperations::cleanup_empty_tracks(project);
    Ok(kept)
}

pub fn split_and_trim_at_playhead(
    project: &mut Project,
    split_frame: u64,
    is_left: bool,
    gapless_enabled: bool,
    split_all: bool,
    selected_clip_ids: &[String],
) -> AppResult<bool> {
    let mut target_clip_ids = Vec::new();

    for track in &project.tracks {
        for clip in &track.clips {
            let inside = split_frame > clip.start_frame && split_frame < clip.start_frame + clip.duration_frames;
            if inside {
                if split_all {
                    target_clip_ids.push(clip.id.clone());
                } else if selected_clip_ids.contains(&clip.id) {
                    target_clip_ids.push(clip.id.clone());
                } else if selected_clip_ids.is_empty() && target_clip_ids.is_empty() {
                    target_clip_ids.push(clip.id.clone());
                }
            }
        }
    }

    if target_clip_ids.is_empty() {
        return Ok(false);
    }

    for id in target_clip_ids {
        let _ = split_and_trim(project, &id, split_frame, is_left, gapless_enabled);
    }

    Ok(true)
}
