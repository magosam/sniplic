pub mod effect_tracks;
pub mod gapless;
pub mod insert;
pub mod movement;
pub mod ripple;
pub mod slice;
pub mod tracks;

pub use effect_tracks::EffectTrackOperations;
pub use insert::InsertOperations;
pub use movement::{MoveClipItem, MovementOperations};
pub use slice::SliceOperations;
pub use tracks::TrackOperations;

use crate::core::project::{Clip, Project, Track, TrackType};
use crate::error::AppResult;

pub struct TimelineEngine;

impl TimelineEngine {
    pub fn add_clip(
        project: &mut Project,
        track_id: &str,
        media_id: &str,
        start_frame: u64,
        push: bool,
        image_duration_frames: Option<u64>,
    ) -> AppResult<Clip> {
        InsertOperations::add_clip(project, track_id, media_id, start_frame, push, image_duration_frames)
    }

    pub fn add_clips_batch(
        project: &mut Project,
        media_ids: &[String],
        start_frame: u64,
        target_track_id: Option<String>,
        push: bool,
        image_duration_frames: Option<u64>,
    ) -> AppResult<Vec<Clip>> {
        InsertOperations::add_clips_batch(project, media_ids, start_frame, target_track_id, push, image_duration_frames)
    }

    pub fn move_clip(
        project: &mut Project,
        clip_id: &str,
        target_track_id: &str,
        new_start: u64,
        push: bool,
        gapless: bool,
    ) -> AppResult<()> {
        MovementOperations::move_clip(project, clip_id, target_track_id, new_start, push, gapless)
    }

    pub fn move_clips_batch(
        project: &mut Project,
        moves: &[MoveClipItem],
        push: bool,
        gapless: bool,
    ) -> AppResult<bool> {
        MovementOperations::move_clips_batch(project, moves, push, gapless)
    }

    pub fn remove_clip(project: &mut Project, clip_id: &str, gapless: bool) -> AppResult<()> {
        MovementOperations::remove_clip(project, clip_id, gapless)
    }

    /// Closes all gaps in the MAIN tracks (base video V1 and base audio
    /// A1), packing clips starting from frame 0. Extra tracks (V2+,
    /// A2+) are not touched -- this is the action triggered when ENABLING "Gapless"
    /// mode on the toolbar, to fix any already-spaced media in the project at once.
    pub fn compact_main_tracks(project: &mut Project) -> AppResult<()> {
        if let Some(id) = TrackOperations::main_video_track_id(project) {
            if let Some(t) = project.tracks.iter_mut().find(|t| t.id == id) {
                if !t.locked {
                    TrackOperations::compact_track(t);
                }
            }
        }
        if let Some(id) = TrackOperations::main_audio_track_id(project) {
            if let Some(t) = project.tracks.iter_mut().find(|t| t.id == id) {
                if !t.locked {
                    TrackOperations::compact_track(t);
                }
            }
        }
        Ok(())
    }

    pub fn split_clip(
        project: &mut Project,
        clip_id: &str,
        split_frame: u64,
    ) -> AppResult<(Clip, Clip)> {
        SliceOperations::split_clip(project, clip_id, split_frame)
    }

    pub fn split_and_trim(
        project: &mut Project,
        clip_id: &str,
        split_frame: u64,
        is_left: bool,
        gapless: bool,
    ) -> AppResult<Clip> {
        SliceOperations::split_and_trim(project, clip_id, split_frame, is_left, gapless)
    }

    pub fn split_at_playhead(
        project: &mut Project,
        split_frame: u64,
        split_all: bool,
        selected_clip_ids: &[String],
    ) -> AppResult<bool> {
        SliceOperations::split_at_playhead(project, split_frame, split_all, selected_clip_ids)
    }

    pub fn split_and_trim_at_playhead(
        project: &mut Project,
        split_frame: u64,
        is_left: bool,
        gapless: bool,
        split_all: bool,
        selected_clip_ids: &[String],
    ) -> AppResult<bool> {
        SliceOperations::split_and_trim_at_playhead(
            project,
            split_frame,
            is_left,
            gapless,
            split_all,
            selected_clip_ids,
        )
    }

    pub fn trim_clip(
        project: &mut Project,
        clip_id: &str,
        edge: &str,
        target_frame: u64,
        push: bool,
        snap: bool,
        gapless: bool,
    ) -> AppResult<Clip> {
        SliceOperations::trim_clip(project, clip_id, edge, target_frame, push, snap, gapless)
    }

    pub fn add_track(
        project: &mut Project,
        track_type: TrackType,
        near_track_id: Option<&str>,
    ) -> AppResult<Track> {
        TrackOperations::add_track(project, track_type, near_track_id)
    }

    pub fn add_filter_track(
        project: &mut Project,
        near_track_id: Option<&str>,
    ) -> AppResult<Track> {
        EffectTrackOperations::add_filter_track(project, near_track_id)
    }
}