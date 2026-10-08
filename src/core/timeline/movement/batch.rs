use crate::core::project::{Clip, MediaType, Project, TrackType};
use crate::error::{AppError, AppResult};
use super::super::gapless;
use super::super::ripple::{push_left_minimal, push_right_minimal};
use super::super::tracks::TrackOperations;
use super::truncate::truncate_overlapping_clips_ignoring;
use super::types::MoveClipItem;

pub fn move_clips_batch(
    project: &mut Project,
    moves: &[MoveClipItem],
    push: bool,
    gapless_enabled: bool,
) -> AppResult<bool> {
    let moving_ids: Vec<String> = moves.iter().map(|m| m.clip_id.clone()).collect();
    let mut extracted_clips: Vec<(Clip, String)> = Vec::new();

    // 0. Validation of locked tracks (source and target)
    for mv in moves {
        let origin_track = project.tracks.iter().find(|t| t.clips.iter().any(|c| c.id == mv.clip_id));
        if let Some(t) = origin_track {
            if t.locked {
                return Err(AppError::InvalidOperation(format!("Track '{}' is locked", t.name)));
            }
        }
        if mv.target_track_id != "__NEW_VIDEO_TRACK__" && mv.target_track_id != "__NEW_AUDIO_TRACK__" {
            if let Some(t) = project.tracks.iter().find(|t| t.id == mv.target_track_id) {
                if t.locked {
                    return Err(AppError::InvalidOperation(format!("Target track '{}' is locked", t.name)));
                }
            }
        }
    }

    // 1. Validation of media type vs track type lock
    for mv in moves {
        let clip_info = project
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.id == mv.clip_id);

        if let Some(c) = clip_info {
            if let Some(media) = project.media_pool.get(&c.media_id) {
                let is_audio_media = media.media_type == MediaType::Audio;

                if mv.target_track_id == "__NEW_VIDEO_TRACK__" && is_audio_media {
                    return Err(AppError::InvalidOperation("Audio cannot go into a video track".into()));
                }
                if mv.target_track_id == "__NEW_AUDIO_TRACK__" && !is_audio_media {
                    return Err(AppError::InvalidOperation("Video/Image cannot go into an audio track".into()));
                }

                if mv.target_track_id != "__NEW_VIDEO_TRACK__" && mv.target_track_id != "__NEW_AUDIO_TRACK__" {
                    if let Some(target) = project.tracks.iter().find(|t| t.id == mv.target_track_id) {
                        if target.track_type == TrackType::Filter {
                            return Err(AppError::InvalidOperation("Video and audio media cannot go into an effects track".into()));
                        }
                        if target.track_type == TrackType::Video && is_audio_media {
                            return Err(AppError::InvalidOperation("Audio blocked in video track".into()));
                        }
                        if target.track_type == TrackType::Audio && !is_audio_media {
                            return Err(AppError::InvalidOperation("Video/Image blocked in audio track".into()));
                        }
                    }
                }
            }
        }
    }

    let main_track_ids = gapless::gapless_track_ids(project);
    let gapless_ids = if gapless_enabled {
        main_track_ids.clone()
    } else {
        Default::default()
    };

    // 2. Extract moving clips
    for track in &mut project.tracks {
        let mut remaining = Vec::new();
        for clip in track.clips.drain(..) {
            if moving_ids.contains(&clip.id) {
                extracted_clips.push((clip, track.id.clone()));
            } else {
                remaining.push(clip);
            }
        }
        track.clips = remaining;
    }

    // 3. Close gaps at source -- strictly when Gapless is active or
    // when Push is performing a swap (direct overlap with another clip on the same track).
    // Push NEVER closes gaps or drags media to the left when moving to free space!
    for track in &mut project.tracks {
        let is_main = main_track_ids.contains(&track.id);
        let is_gapless_active = gapless_ids.contains(&track.id);

        let is_push_swap = push && is_main && moves.iter().any(|mv| {
            mv.target_track_id == track.id
                && extracted_clips.iter().any(|(c, src)| src == &track.id && c.id == mv.clip_id)
                && track.clips.iter().any(|c| {
                    let dur = extracted_clips.iter().find(|(ec, _)| ec.id == mv.clip_id).map(|(ec, _)| ec.duration_frames).unwrap_or(0);
                    mv.new_start_frame < c.start_frame + c.duration_frames && mv.new_start_frame + dur > c.start_frame
                })
        });

        if !(is_gapless_active || is_push_swap) {
            continue;
        }

        let lifted_here: Vec<&Clip> = extracted_clips
            .iter()
            .filter(|(_, src_id)| src_id == &track.id)
            .map(|(c, _)| c)
            .collect();

        if !lifted_here.is_empty() {
            for remaining in track.clips.iter_mut() {
                let mut shift: u64 = 0;
                for lifted in &lifted_here {
                    if lifted.start_frame < remaining.start_frame {
                        shift += lifted.duration_frames;
                    }
                }
                remaining.start_frame = remaining.start_frame.saturating_sub(shift);
            }
        }
    }

    // 4. Insert into targets
    let mut sorted_moves = moves.to_vec();
    sorted_moves.sort_by_key(|m| m.new_start_frame);

    for mv in sorted_moves {
        if let Some(idx) = extracted_clips.iter().position(|(c, _)| c.id == mv.clip_id) {
            let (mut clip, _) = extracted_clips.remove(idx);
            clip.start_frame = mv.new_start_frame;

            let target_track_id = if mv.target_track_id == "__NEW_VIDEO_TRACK__" {
                TrackOperations::add_track(project, TrackType::Video, mv.near_track_id.as_deref())?.id
            } else if mv.target_track_id == "__NEW_AUDIO_TRACK__" {
                TrackOperations::add_track(project, TrackType::Audio, mv.near_track_id.as_deref())?.id
            } else {
                mv.target_track_id.clone()
            };

            let is_main = gapless::track_allows_gapless(project, &target_track_id);
            
            // If gapless is disabled OR it's not the main track, we must NOT push.
            // Instead of truncating (overwriting), we find an available track above it (CapCut behavior).
            let actual_target_id = if push && gapless_enabled && is_main {
                target_track_id.clone()
            } else {
                TrackOperations::find_or_create_available_track(
                    project,
                    &target_track_id,
                    mv.new_start_frame,
                    clip.duration_frames,
                    &moving_ids,
                )?
            };

            if let Some(target) = project.tracks.iter_mut().find(|t| t.id == actual_target_id) {
                if push && gapless_enabled && is_main {
                    if mv.push_direction.as_deref() == Some("left") {
                        push_left_minimal(&mut target.clips, mv.new_start_frame, mv.new_start_frame);
                        push_right_minimal(&mut target.clips, mv.new_start_frame, mv.new_start_frame + clip.duration_frames);
                    } else {
                        push_right_minimal(&mut target.clips, mv.new_start_frame, mv.new_start_frame + clip.duration_frames);
                        push_left_minimal(&mut target.clips, mv.new_start_frame, mv.new_start_frame);
                    }
                }
                target.clips.push(clip);
                target.clips.sort_by_key(|c| c.start_frame);
            }
        }
    }

    for track in &mut project.tracks {
        track.clips.sort_by_key(|c| c.start_frame);
    }

    TrackOperations::cleanup_empty_tracks(project);
    Ok(true)
}
