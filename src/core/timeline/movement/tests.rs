use super::*;
use crate::core::project::{
    Clip, MediaItem, MediaType, ProjectConfig, ProjectMetadata, Track, TrackType,
};
use std::collections::HashMap;

fn make_clip(id: &str, media_id: &str, start: u64, dur: u64) -> Clip {
    Clip {
        id: id.to_string(),
        media_id: media_id.to_string(),
        name: id.to_string(),
        start_frame: start,
        duration_frames: dur,
        in_point_frames: 0,
        out_point_frames: dur,
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
            name: "Teste".into(),
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

fn video_media(id: &str) -> MediaItem {
    MediaItem {
        id: id.to_string(),
        name: id.to_string(),
        file_path: "/tmp/dummy.mp4".into(),
        media_type: MediaType::Video,
        duration_frames: 1000,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        sample_rate: None,
        channels: None,
        added_at: 0,
        folder_id: None,
    }
}

fn audio_media(id: &str) -> MediaItem {
    MediaItem {
        id: id.to_string(),
        name: id.to_string(),
        file_path: "/tmp/dummy.mp3".into(),
        media_type: MediaType::Audio,
        duration_frames: 1000,
        width: None,
        height: None,
        fps: None,
        sample_rate: Some(48000),
        channels: Some(2),
        added_at: 0,
        folder_id: None,
    }
}

#[test]
fn move_clip_relocates_to_target_track_and_position() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let mut project = make_project(vec![
        make_track("origin", TrackType::Video, vec![c1], false),
        make_track("v1", TrackType::Video, vec![], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    MovementOperations::move_clip(&mut project, "c1", "v1", 60, false, false).unwrap();

    let v1 = project.tracks.iter().find(|t| t.id == "v1").unwrap();
    assert_eq!(v1.clips.len(), 1);
    assert_eq!(v1.clips[0].start_frame, 60);

    let origin = project.tracks.iter().find(|t| t.id == "origin");
    assert!(origin.is_none() || origin.unwrap().clips.is_empty());
}

#[test]
fn move_clip_non_ripple_splits_overlapping_existing_clip() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let c2 = make_clip("c2", "m1", 50, 50);
    let mut project = make_project(vec![
        make_track("origin", TrackType::Video, vec![c1], false),
        make_track("v1", TrackType::Video, vec![c2], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    MovementOperations::move_clip(&mut project, "c1", "v1", 60, false, false).unwrap();

    let v1 = project.tracks.iter().find(|t| t.id == "v1").unwrap();
    assert_eq!(v1.clips.len(), 3);

    let left_piece = v1.clips.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(left_piece.start_frame, 50);
    assert_eq!(left_piece.duration_frames, 10);

    let moved = v1.clips.iter().find(|c| c.id == "c1").unwrap();
    assert_eq!(moved.start_frame, 60);
    assert_eq!(moved.duration_frames, 30);

    let right_piece = v1.clips.iter().find(|c| c.id != "c1" && c.id != "c2").unwrap();
    assert_eq!(right_piece.start_frame, 90);
    assert_eq!(right_piece.duration_frames, 10);
}

#[test]
fn move_clip_ripple_pushes_only_the_missing_space_not_full_duration() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let c2 = make_clip("c2", "m1", 50, 10);
    let mut project = make_project(vec![
        make_track("origin", TrackType::Video, vec![c1], false),
        make_track("v1", TrackType::Video, vec![c2], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    MovementOperations::move_clip(&mut project, "c1", "v1", 40, true, false).unwrap();

    let v1 = project.tracks.iter().find(|t| t.id == "v1").unwrap();
    let c2_after = v1.clips.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(c2_after.start_frame, 70);
}

#[test]
fn move_clip_ripple_does_not_touch_clips_when_gap_already_fits() {
    let c1 = make_clip("c1", "m1", 0, 10);
    let c2 = make_clip("c2", "m1", 100, 20);
    let mut project = make_project(vec![
        make_track("origin", TrackType::Video, vec![c1], false),
        make_track("v1", TrackType::Video, vec![c2], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    MovementOperations::move_clip(&mut project, "c1", "v1", 40, true, false).unwrap();

    let v1 = project.tracks.iter().find(|t| t.id == "v1").unwrap();
    let c2_after = v1.clips.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(c2_after.start_frame, 100);
}

#[test]
fn move_clip_rejects_when_origin_track_is_locked() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let mut project = make_project(vec![
        make_track("origin", TrackType::Video, vec![c1], true),
        make_track("v1", TrackType::Video, vec![], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    assert!(MovementOperations::move_clip(&mut project, "c1", "v1", 60, false, false).is_err());
}

#[test]
fn move_clip_rejects_when_target_track_is_locked() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let mut project = make_project(vec![
        make_track("origin", TrackType::Video, vec![c1], false),
        make_track("v1", TrackType::Video, vec![], true),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    assert!(MovementOperations::move_clip(&mut project, "c1", "v1", 60, false, false).is_err());
}

#[test]
fn move_clip_rejects_audio_clip_into_video_track() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let mut project = make_project(vec![
        make_track("origin", TrackType::Audio, vec![c1], false),
        make_track("v1", TrackType::Video, vec![], false),
    ]);
    project.media_pool.insert("m1".to_string(), audio_media("m1"));

    assert!(MovementOperations::move_clip(&mut project, "c1", "v1", 60, false, false).is_err());
}

#[test]
fn move_clip_rejects_video_clip_into_audio_track() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let mut project = make_project(vec![
        make_track("origin", TrackType::Video, vec![c1], false),
        make_track("a1", TrackType::Audio, vec![], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    assert!(MovementOperations::move_clip(&mut project, "c1", "a1", 60, false, false).is_err());
}

#[test]
fn remove_clip_deletes_clip_from_track() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![c1], false)]);

    MovementOperations::remove_clip(&mut project, "c1", false).unwrap();

    let v1 = project.tracks.iter().find(|t| t.id == "v1");
    assert!(v1.is_none() || v1.unwrap().clips.is_empty());
}

#[test]
fn remove_clip_without_ripple_leaves_gap() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let c2 = make_clip("c2", "m1", 30, 20);
    let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);

    MovementOperations::remove_clip(&mut project, "c1", false).unwrap();

    let v1 = project.tracks.iter().find(|t| t.id == "v1").unwrap();
    let c2_after = v1.clips.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(c2_after.start_frame, 30);
}

#[test]
fn remove_clip_with_ripple_closes_gap() {
    let c1 = make_clip("c1", "m1", 10, 30);
    let c2 = make_clip("c2", "m1", 40, 20);
    let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![c1, c2], false)]);

    MovementOperations::remove_clip(&mut project, "c1", true).unwrap();

    let v1 = project.tracks.iter().find(|t| t.id == "v1").unwrap();
    assert_eq!(v1.clips.len(), 1);
    let c2_after = v1.clips.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(c2_after.start_frame, 10);
}

#[test]
fn remove_clip_with_ripple_does_not_move_clips_before_it() {
    let c0 = make_clip("c0", "m1", 0, 5);
    let c1 = make_clip("c1", "m1", 10, 30);
    let c2 = make_clip("c2", "m1", 40, 20);
    let mut project =
        make_project(vec![make_track("v1", TrackType::Video, vec![c0, c1, c2], false)]);

    MovementOperations::remove_clip(&mut project, "c1", true).unwrap();

    let v1 = project.tracks.iter().find(|t| t.id == "v1").unwrap();
    let c0_after = v1.clips.iter().find(|c| c.id == "c0").unwrap();
    assert_eq!(c0_after.start_frame, 0);
}

#[test]
fn remove_clip_with_gapless_does_not_close_gap_on_extra_track() {
    let c1 = make_clip("c1", "m1", 10, 30);
    let c2 = make_clip("c2", "m1", 40, 20);
    let mut project = make_project(vec![
        make_track("v_extra", TrackType::Video, vec![c1, c2], false),
        make_track("v_main", TrackType::Video, vec![], false),
    ]);

    MovementOperations::remove_clip(&mut project, "c1", true).unwrap();

    let v_extra = project.tracks.iter().find(|t| t.id == "v_extra").unwrap();
    let c2_after = v_extra.clips.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(c2_after.start_frame, 40);
}

#[test]
fn move_clips_batch_gapless_does_not_close_origin_gap_on_extra_track() {
    let c1 = make_clip("c1", "m1", 10, 30);
    let c2 = make_clip("c2", "m1", 40, 20);
    let mut project = make_project(vec![
        make_track("v_extra", TrackType::Video, vec![c1, c2], false),
        make_track("v_main", TrackType::Video, vec![], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    MovementOperations::move_clip(&mut project, "c1", "v_main", 200, false, true).unwrap();

    let v_extra = project.tracks.iter().find(|t| t.id == "v_extra").unwrap();
    let c2_after = v_extra.clips.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(c2_after.start_frame, 40);
}

#[test]
fn move_clips_batch_gapless_closes_origin_gap_on_main_track() {
    let c1 = make_clip("c1", "m1", 10, 30);
    let c2 = make_clip("c2", "m1", 40, 20);
    let mut project = make_project(vec![
        make_track("v_main", TrackType::Video, vec![c1, c2], false),
        make_track("a1", TrackType::Audio, vec![], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    MovementOperations::move_clip(&mut project, "c1", "v_main", 200, false, true).unwrap();

    let v_main = project.tracks.iter().find(|t| t.id == "v_main").unwrap();
    let c2_after = v_main.clips.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(c2_after.start_frame, 10);
}

#[test]
fn move_clips_batch_new_track_sentinel_with_near_track_id_inserts_beside_it() {
    let c0 = make_clip("c0", "m1", 0, 10);
    let c1 = make_clip("c1", "m1", 100, 10);
    let mut project = make_project(vec![
        make_track("v_extra", TrackType::Video, vec![c0, c1], false),
        make_track("v_main", TrackType::Video, vec![], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    let moves = vec![MoveClipItem {
        clip_id: "c1".to_string(),
        target_track_id: "__NEW_VIDEO_TRACK__".to_string(),
        new_start_frame: 0,
        near_track_id: Some("v_extra".to_string()),
    }];

    MovementOperations::move_clips_batch(&mut project, &moves, false, false).unwrap();

    let video_ids: Vec<&str> = project
        .tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Video)
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(video_ids.len(), 3);
    assert_eq!(video_ids[1], "v_extra");
    assert_eq!(video_ids[2], "v_main");

    let new_track_id = video_ids[0].to_string();
    let new_track = project.tracks.iter().find(|t| t.id == new_track_id).unwrap();
    assert_eq!(new_track.clips.len(), 1);
    assert_eq!(new_track.clips[0].id, "c1");

    let v_extra = project.tracks.iter().find(|t| t.id == "v_extra").unwrap();
    assert_eq!(v_extra.clips.len(), 1);
    assert_eq!(v_extra.clips[0].id, "c0");
}

#[test]
fn remove_clip_rejects_locked_track() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let mut project = make_project(vec![make_track("v1", TrackType::Video, vec![c1], true)]);

    assert!(MovementOperations::remove_clip(&mut project, "c1", false).is_err());
}

#[test]
fn move_clip_push_does_not_push_on_extra_track() {
    let c1 = make_clip("c1", "m1", 0, 30);
    let c2 = make_clip("c2", "m1", 100, 50);
    let mut project = make_project(vec![
        make_track("v2", TrackType::Video, vec![c2], false),
        make_track("v1", TrackType::Video, vec![c1], false),
    ]);
    project.media_pool.insert("m1".to_string(), video_media("m1"));

    MovementOperations::move_clip(&mut project, "c1", "v2", 50, true, false).unwrap();

    let v2 = project.tracks.iter().find(|t| t.id == "v2").unwrap();
    let c2_after = v2.clips.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(c2_after.start_frame, 100);
}
