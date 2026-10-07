pub mod split;
pub mod split_trim;
pub mod trim;
pub mod trim_snap;

use crate::core::project::{Clip, Project};
use crate::error::AppResult;

pub struct SliceOperations;

impl SliceOperations {
    #[inline]
    pub fn split_clip(
        project: &mut Project,
        clip_id: &str,
        split_frame: u64,
    ) -> AppResult<(Clip, Clip)> {
        split::split_clip(project, clip_id, split_frame)
    }

    #[inline]
    pub fn split_at_playhead(
        project: &mut Project,
        split_frame: u64,
        split_all: bool,
        selected_clip_ids: &[String],
    ) -> AppResult<bool> {
        split::split_at_playhead(project, split_frame, split_all, selected_clip_ids)
    }

    #[inline]
    pub fn split_and_trim(
        project: &mut Project,
        clip_id: &str,
        split_frame: u64,
        is_left: bool,
        gapless_enabled: bool,
    ) -> AppResult<Clip> {
        split_trim::split_and_trim(project, clip_id, split_frame, is_left, gapless_enabled)
    }

    #[inline]
    pub fn split_and_trim_at_playhead(
        project: &mut Project,
        split_frame: u64,
        is_left: bool,
        gapless_enabled: bool,
        split_all: bool,
        selected_clip_ids: &[String],
    ) -> AppResult<bool> {
        split_trim::split_and_trim_at_playhead(
            project,
            split_frame,
            is_left,
            gapless_enabled,
            split_all,
            selected_clip_ids,
        )
    }

    #[inline]
    pub fn trim_clip(
        project: &mut Project,
        clip_id: &str,
        edge: &str,
        target_frame: u64,
        push: bool,
        snap: bool,
        gapless: bool,
    ) -> AppResult<Clip> {
        trim::trim_clip(project, clip_id, edge, target_frame, push, snap, gapless)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::project::{
        Clip, MediaItem, MediaType, ProjectConfig, ProjectMetadata, Track, TrackType,
    };
    use std::collections::HashMap;

    fn make_clip(id: &str, media_id: &str, start: u64, dur: u64, in_point: u64) -> Clip {
        Clip {
            id: id.to_string(),
            media_id: media_id.to_string(),
            name: id.to_string(),
            start_frame: start,
            duration_frames: dur,
            in_point_frames: in_point,
            out_point_frames: in_point + dur,
            ..Default::default()
        }
    }

    fn make_track(id: &str, track_type: TrackType, clips: Vec<Clip>, locked: bool) -> Track {
        Track {
            id: id.to_string(),
            name: id.to_string(),
            track_type,
            locked,
            muted: false,
            hidden: false,
            solo: false,
            clips,
            height: None,
        }
    }

    fn make_project(tracks: Vec<Track>) -> Project {
        Project {
            metadata: ProjectMetadata {
                id: "prj_test".into(),
                name: "Test".into(),
                version: "1.0.0".into(),
                revision: 1,
                created_at: 0,
                updated_at: 0,
            },
            config: ProjectConfig::default(),
            media_pool: HashMap::new(),
            media_folders: None,
            tracks,
            subtitles: None,
            markers: None,
        }
    }

    fn video_media(id: &str, duration_frames: u64) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            name: id.to_string(),
            file_path: "/tmp/dummy.mp4".into(),
            media_type: MediaType::Video,
            duration_frames,
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            sample_rate: None,
            channels: None,
            added_at: 0,
            folder_id: None,
        }
    }

    fn image_media(id: &str) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            name: id.to_string(),
            file_path: "/tmp/dummy.png".into(),
            media_type: MediaType::Image,
            duration_frames: 150,
            width: Some(1920),
            height: Some(1080),
            fps: None,
            sample_rate: None,
            channels: None,
            added_at: 0,
            folder_id: None,
        }
    }

    // ---------- split_clip ----------

    #[test]
    fn split_clip_divides_duration_and_in_out_points() {
        let clip = make_clip("c1", "m1", 0, 100, 20);
        let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![clip], false)]);

        let (first, second) = SliceOperations::split_clip(&mut project, "c1", 40).unwrap();

        assert_eq!(first.duration_frames, 40);
        assert_eq!(first.start_frame, 0);
        assert_eq!(first.in_point_frames, 20);
        assert_eq!(first.out_point_frames, 60);

        assert_eq!(second.start_frame, 40);
        assert_eq!(second.duration_frames, 60);
        assert_eq!(second.in_point_frames, 60);

        assert_eq!(project.tracks[0].clips.len(), 2);
    }

    #[test]
    fn split_clip_rejects_point_outside_bounds() {
        let clip = make_clip("c1", "m1", 0, 100, 0);
        let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![clip], false)]);

        assert!(SliceOperations::split_clip(&mut project, "c1", 0).is_err());
        assert!(SliceOperations::split_clip(&mut project, "c1", 100).is_err());
        assert!(SliceOperations::split_clip(&mut project, "c1", 150).is_err());
    }

    #[test]
    fn split_clip_rejects_locked_track() {
        let clip = make_clip("c1", "m1", 0, 100, 0);
        let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![clip], true)]);

        assert!(SliceOperations::split_clip(&mut project, "c1", 40).is_err());
    }

    // ---------- split_and_trim (ripple) ----------

    #[test]
    fn split_and_trim_ripple_shifts_downstream_clips() {
        let c1 = make_clip("c1", "m1", 0, 100, 0);
        let c2 = make_clip("c2", "m1", 100, 50, 0);
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);

        let kept = SliceOperations::split_and_trim(&mut project, "c1", 40, false, true).unwrap();

        assert_eq!(kept.id, "c1");
        assert_eq!(kept.duration_frames, 40);

        let c2_after = project.tracks[0].clips.iter().find(|c| c.id == "c2").unwrap();
        assert_eq!(c2_after.start_frame, 40);
    }

    #[test]
    fn split_and_trim_gapless_does_not_shift_clips_on_extra_track() {
        let c1 = make_clip("c1", "m1", 0, 100, 0);
        let c2 = make_clip("c2", "m1", 100, 50, 0);
        let c3 = make_clip("c3", "m1", 100, 30, 0);
        let mut project = make_project(vec![
            make_track("v_extra", TrackType::Video, vec![c3], false),
            make_track("v1", TrackType::Video, vec![c1, c2], false),
        ]);

        SliceOperations::split_and_trim(&mut project, "c1", 40, false, true).unwrap();

        let v_extra = project.tracks.iter().find(|t| t.id == "v_extra").unwrap();
        let c3_after = v_extra.clips.iter().find(|c| c.id == "c3").unwrap();
        assert_eq!(c3_after.start_frame, 100);
    }

    // ---------- trim_clip ----------

    #[test]
    fn trim_clip_left_edge_shrinks_and_updates_in_point() {
        let clip = make_clip("c1", "m1", 0, 100, 10);
        let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![clip], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 500));

        let updated = SliceOperations::trim_clip(&mut project, "c1", "left", 30, false, false, false).unwrap();

        assert_eq!(updated.start_frame, 30);
        assert_eq!(updated.duration_frames, 70);
        assert_eq!(updated.in_point_frames, 40);
    }

    #[test]
    fn trim_clip_left_edge_cannot_pull_before_media_in_point_zero() {
        let clip = make_clip("c1", "m1", 20, 100, 10);
        let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![clip], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 500));

        let updated = SliceOperations::trim_clip(&mut project, "c1", "left", 0, false, false, false).unwrap();

        assert_eq!(updated.start_frame, 10);
        assert_eq!(updated.in_point_frames, 0);
    }

    #[test]
    fn trim_clip_right_edge_respects_media_duration_limit() {
        let clip = make_clip("c1", "m1", 0, 50, 400);
        let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![clip], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 500));

        let updated = SliceOperations::trim_clip(&mut project, "c1", "right", 300, false, false, false).unwrap();

        assert_eq!(updated.duration_frames, 100);
    }

    #[test]
    fn trim_clip_right_edge_ripple_shifts_downstream_clips() {
        let c1 = make_clip("c1", "m1", 0, 50, 0);
        let c2 = make_clip("c2", "m1", 50, 30, 0);
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 1000));

        SliceOperations::trim_clip(&mut project, "c1", "right", 80, true, false, false).unwrap();

        let c2_after = project.tracks[0].clips.iter().find(|c| c.id == "c2").unwrap();
        assert_eq!(c2_after.start_frame, 80);
    }

    #[test]
    fn trim_clip_snaps_to_neighbor_edge_when_close_enough() {
        let c1 = make_clip("c1", "m1", 0, 50, 0);
        let c2 = make_clip("c2", "m1", 100, 30, 0);
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 1000));

        let updated = SliceOperations::trim_clip(&mut project, "c1", "right", 96, false, true, false).unwrap();

        assert_eq!(updated.duration_frames, 100);
    }

    #[test]
    fn trim_clip_does_not_snap_when_outside_threshold() {
        let c1 = make_clip("c1", "m1", 0, 50, 0);
        let c2 = make_clip("c2", "m1", 100, 30, 0);
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 1000));

        let updated = SliceOperations::trim_clip(&mut project, "c1", "right", 80, false, true, false).unwrap();

        assert_eq!(updated.duration_frames, 80);
    }

    #[test]
    fn trim_clip_rejects_locked_track() {
        let clip = make_clip("c1", "m1", 0, 100, 0);
        let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![clip], true)]);

        assert!(SliceOperations::trim_clip(&mut project, "c1", "left", 10, false, false, false).is_err());
    }

    // ---------- trim_clip: Push vs Non-Push rules at edges ----------

    #[test]
    fn trim_clip_left_edge_without_push_passes_over_previous_clip() {
        // Rule: "with push disabled, it overwrites"
        // "a media item is never added on top of another while the underlying one remains the same size.
        // the obscured part is always lost, as if deleted."
        let c1 = make_clip("c1", "m_img", 0, 50, 0); // 0..50
        let c2 = make_clip("c2", "m_img", 50, 30, 0); // 50..80
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);
        project.media_pool.insert("m_img".to_string(), image_media("m_img"));

        let updated = SliceOperations::trim_clip(&mut project, "c2", "left", 20, false, false, false).unwrap();

        assert_eq!(updated.start_frame, 20);
        assert_eq!(updated.duration_frames, 60);
        let c1_after = project.tracks[0].clips.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c1_after.start_frame, 0);
        // c1 which ended at 50 was truncated at overlap [20..50): now has duration 20!
        assert_eq!(c1_after.duration_frames, 20);
    }

    #[test]
    fn trim_clip_left_edge_with_push_cannot_increase_past_previous_clip() {
        // Rule: "if on the left, active push cannot expand past previous clip"
        let c1 = make_clip("c1", "m_img", 100, 50, 0); // 100..150
        let c2 = make_clip("c2", "m_img", 150, 30, 0); // 150..180
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);
        project.media_pool.insert("m_img".to_string(), image_media("m_img"));

        let updated = SliceOperations::trim_clip(&mut project, "c2", "left", 130, true, false, false).unwrap();

        // Clamped to prev_clip_end (150): does not expand to the left!
        assert_eq!(updated.start_frame, 150);
        assert_eq!(updated.duration_frames, 30);

        let c1_after = project.tracks[0].clips.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c1_after.start_frame, 100);
        assert_eq!(c1_after.duration_frames, 50);
    }

    #[test]
    fn trim_clip_right_edge_without_push_passes_over_next_clip() {
        // Rule: "only if push is active [it drags]. otherwise it overwrites the media on the right."
        // "a media item is never added on top of another while the underlying one remains the same size.
        // the obscured part is always lost, as if deleted."
        let c1 = make_clip("c1", "m1", 0, 50, 0); // 0..50
        let c2 = make_clip("c2", "m1", 80, 100, 0); // 80..180
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 1000));

        let updated = SliceOperations::trim_clip(&mut project, "c1", "right", 150, false, false, false).unwrap();

        // Overwrites the media on the right up to 150
        assert_eq!(updated.duration_frames, 150);
        // c2 which was at 80..180 was truncated in the covered portion [80..150): now starts at 150 with duration 30!
        let c2_after = project.tracks[0].clips.iter().find(|c| c.id == "c2").unwrap();
        assert_eq!(c2_after.start_frame, 150);
        assert_eq!(c2_after.duration_frames, 30);
    }

    // ---------- trim_clip: Gapless mode ----------

    #[test]
    fn trim_clip_right_edge_shrinks_and_gapless_shifts_downstream_clips() {
        // c1 [0..50], c2 [50..80], c3 [80..120] on the main track
        let c1 = make_clip("c1", "m1", 0, 50, 0);
        let c2 = make_clip("c2", "m1", 50, 30, 0);
        let c3 = make_clip("c3", "m1", 80, 40, 0);
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2, c3], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 1000));

        // c1 shrinks from the right from 50 to 30 with gapless = true
        let updated = SliceOperations::trim_clip(&mut project, "c1", "right", 30, false, false, true).unwrap();

        assert_eq!(updated.duration_frames, 30);
        assert_eq!(updated.start_frame, 0);

        let c2_after = project.tracks[0].clips.iter().find(|c| c.id == "c2").unwrap();
        let c3_after = project.tracks[0].clips.iter().find(|c| c.id == "c3").unwrap();

        // c2 and c3 moved 20 frames to the left, snapping to the new end of c1 (30) without a gap!
        assert_eq!(c2_after.start_frame, 30);
        assert_eq!(c3_after.start_frame, 60);
    }

    #[test]
    fn trim_clip_right_edge_shrinks_without_gapless_leaves_gap() {
        let c1 = make_clip("c1", "m1", 0, 50, 0);
        let c2 = make_clip("c2", "m1", 50, 30, 0);
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 1000));

        // c1 shrinks from 50 to 30 with gapless = false (must leave a gap)
        let updated = SliceOperations::trim_clip(&mut project, "c1", "right", 30, false, false, false).unwrap();

        assert_eq!(updated.duration_frames, 30);
        let c2_after = project.tracks[0].clips.iter().find(|c| c.id == "c2").unwrap();
        assert_eq!(c2_after.start_frame, 50); // c2 remained in place, 20-frame gap preserved
    }

    #[test]
    fn trim_clip_left_edge_shrinks_and_gapless_shifts_downstream_clips() {
        let c1 = make_clip("c1", "m1", 0, 50, 0);
        let c2 = make_clip("c2", "m1", 50, 50, 0);
        let mut project =
            make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);
        project.media_pool.insert("m1".to_string(), video_media("m1", 1000));

        // c2 is trimmed from the left from 50 to 70 (in_point increases by 20, dur decreases from 50 to 30)
        let updated = SliceOperations::trim_clip(&mut project, "c2", "left", 70, false, false, true).unwrap();

        assert_eq!(updated.duration_frames, 30);
        assert_eq!(updated.in_point_frames, 20);
        // In gapless mode on the main track, c2 snaps to the end of c1 (50)!
        assert_eq!(updated.start_frame, 50);
    }
}
