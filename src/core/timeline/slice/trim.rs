use crate::core::project::{Clip, MediaType, Project};
use crate::error::{AppError, AppResult};
use super::super::gapless;
use super::super::movement::truncate::truncate_overlapping_clips;
use super::super::ripple::push_right_minimal;
use super::super::tracks::TrackOperations;
use super::trim_snap::snap_to_neighbor_edge;

pub fn trim_clip(
    project: &mut Project,
    clip_id: &str,
    edge: &str,
    target_frame: u64,
    push: bool,
    snap: bool,
    gapless_enabled: bool,
) -> AppResult<Clip> {
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
            let orig = track.clips[pos].clone();
            let orig_start = orig.start_frame;
            let orig_dur = orig.duration_frames;
            let orig_end = orig_start + orig_dur;

            let target_frame = if snap {
                snap_to_neighbor_edge(&track.clips, clip_id, target_frame)
            } else {
                target_frame
            };

            let media = project.media_pool.get(&orig.media_id);
            let is_finite = media.map(|m| m.media_type != MediaType::Image).unwrap_or(false);
            let max_media_frames = media.map(|m| m.duration_frames).unwrap_or(u64::MAX);

            let is_gapless = gapless_ids.contains(&track.id);
            let mut updated = orig.clone();

            if edge == "left" {
                // Limit by source material: in_point cannot be less
                // than 0 (video/audio). Image does not have this limit ("infinite"
                // source), only timeline frame 0 (floor below).
                let media_min_start = if is_finite {
                    orig_start.saturating_sub(orig.in_point_frames)
                } else {
                    0
                };

                let prev_clip_end = track
                    .clips
                    .iter()
                    .filter(|c| c.id != clip_id && c.start_frame < orig_start)
                    .map(|c| c.start_frame + c.duration_frames)
                    .max()
                    .unwrap_or(0);

                // User rule:
                // "if on the left, active push cannot increase. but with push disabled it overwrites."
                // - With push active: locks at previous clip edge (cannot increase invading previous clip)
                // - With push disabled: overwrites (does not lock at prev_clip_end)
                let min_allowed_start = if push {
                    media_min_start.max(prev_clip_end)
                } else {
                    media_min_start
                };

                let new_start = target_frame.max(min_allowed_start).min(orig_end.saturating_sub(1));
                let new_dur = (orig_end - new_start).max(1);
                let diff_in = (new_start as i64) - (orig_start as i64);
                let new_in = (orig.in_point_frames as i64 + diff_in).max(0) as u64;

                updated.start_frame = new_start;
                updated.duration_frames = new_dur;
                updated.in_point_frames = new_in;
                track.clips[pos] = updated.clone();

                if is_gapless && new_start > orig_start {
                    // Clip was shortened from the left (increased start_frame),
                    // which would leave a gap between [orig_start, new_start).
                    // On gapless main track, pull this clip and all subsequent clips
                    // to the left by the exact shift, snapping to previous clip end (or frame 0).
                    let shift = new_start - orig_start;
                    for c in track.clips.iter_mut() {
                        if c.start_frame >= new_start {
                            c.start_frame = c.start_frame.saturating_sub(shift);
                        }
                    }
                    updated.start_frame = updated.start_frame.saturating_sub(shift);
                }
            } else {
                // Right limit by source material: in_point + new_dur
                // cannot exceed media.duration_frames
                let max_dur_by_media = if is_finite {
                    max_media_frames.saturating_sub(orig.in_point_frames).max(1)
                } else {
                    u64::MAX
                };

                let mut new_dur = target_frame.saturating_sub(orig_start).max(1);
                new_dur = new_dur.min(max_dur_by_media);

                // User rule:
                // "when push is active and the user expands media on the right, other media to the right are dragged, only if push is active. otherwise it overwrites media to the right."
                // If push is DISABLED (!push), does NOT limit by next_start -- overwrites!

                updated.duration_frames = new_dur;
                updated.out_point_frames = updated.in_point_frames + new_dur;
                track.clips[pos] = updated.clone();

                let new_end = updated.start_frame + new_dur;

                if push && new_end > orig_end {
                    // Clip expanded: push neighbors to the right (exclusive PUSH function)
                    push_right_minimal(&mut track.clips, orig_end, new_end);
                } else if is_gapless && new_end < orig_end {
                    // Clip decreased and Gapless is active on the main track:
                    // Pull all clips from the right to the left by the exact shift,
                    // closing the gap perfectly without leaving holes!
                    let shift = orig_end - new_end;
                    for c in track.clips.iter_mut() {
                        if c.id != clip_id && c.start_frame >= orig_end {
                            c.start_frame = c.start_frame.saturating_sub(shift);
                        }
                    }
                }
            }

            // Truncates any underlying clips that were overlapped by the resized clip:
            // "a media is never added on top of another while the underlying stays the same size.
            // the covered part is always lost, as if deleted."
            truncate_overlapping_clips(track, updated.start_frame, updated.duration_frames, &updated.id);

            track.clips.sort_by_key(|c| c.start_frame);
            TrackOperations::cleanup_empty_tracks(project);
            return Ok(updated);
        }
    }
    Err(AppError::InvalidOperation("Clip not found".to_string()))
}
