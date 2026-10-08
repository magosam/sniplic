pub mod compact;
pub mod helpers;
pub mod management;

use crate::core::project::{Project, Track, TrackType};
use crate::error::AppResult;

pub struct TrackOperations;

impl TrackOperations {
    #[inline]
    pub fn reindex_tracks(project: &mut Project) {
        management::reindex_tracks(project)
    }

    #[inline]
    pub fn cleanup_empty_tracks(project: &mut Project) {
        management::cleanup_empty_tracks(project)
    }

    #[inline]
    pub fn add_track(
        project: &mut Project,
        track_type: TrackType,
        near_track_id: Option<&str>,
    ) -> AppResult<Track> {
        management::add_track(project, track_type, near_track_id)
    }

    #[inline]
    pub fn ensure_track_unlocked(project: &Project, track_id: &str) -> AppResult<()> {
        helpers::ensure_track_unlocked(project, track_id)
    }

    #[inline]
    pub fn main_video_track_id(project: &Project) -> Option<String> {
        helpers::main_video_track_id(project)
    }

    #[inline]
    pub fn main_audio_track_id(project: &Project) -> Option<String> {
        helpers::main_audio_track_id(project)
    }

    #[inline]
    pub fn find_or_create_available_track(
        project: &mut Project,
        start_track_id: &str,
        start_frame: u64,
        dur: u64,
        ignore_ids: &[String],
    ) -> AppResult<String> {
        helpers::find_or_create_available_track(project, start_track_id, start_frame, dur, ignore_ids)
    }

    #[inline]
    pub fn compact_track(track: &mut Track) {
        compact::compact_track(track)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::project::Clip;

    fn clip(id: &str, start: u64, dur: u64) -> Clip {
        Clip {
            id: id.to_string(),
            media_id: "m1".to_string(),
            name: id.to_string(),
            start_frame: start,
            duration_frames: dur,
            in_point_frames: 0,
            out_point_frames: dur,
            ..Default::default()
        }
    }

    fn track(id: &str, track_type: TrackType, clips: Vec<Clip>) -> Track {
        Track {
            id: id.to_string(),
            name: id.to_string(),
            track_type,
            locked: false,
            muted: false,
            hidden: false,
            solo: false,
            clips,
            height: None,
        }
    }

    #[test]
    fn main_video_track_id_is_the_last_video_in_the_group() {
        let project = Project {
            metadata: crate::core::project::ProjectMetadata {
                id: "p".into(), name: "p".into(), version: "1".into(), revision: 1, created_at: 0, updated_at: 0,
            },
            config: crate::core::project::ProjectConfig::default(),
            media_pool: std::collections::HashMap::new(),
            media_folders: None,
            tracks: vec![
                track("v_new", TrackType::Video, vec![]),
                track("v_base", TrackType::Video, vec![]),
                track("a_base", TrackType::Audio, vec![]),
            ],
            subtitles: None,
            markers: None,
        };

        assert_eq!(TrackOperations::main_video_track_id(&project), Some("v_base".to_string()));
        assert_eq!(TrackOperations::main_audio_track_id(&project), Some("a_base".to_string()));
    }

    #[test]
    fn add_track_video_without_near_track_id_goes_to_absolute_top() {
        let mut project = Project {
            metadata: crate::core::project::ProjectMetadata {
                id: "p".into(), name: "p".into(), version: "1".into(), revision: 1, created_at: 0, updated_at: 0,
            },
            config: crate::core::project::ProjectConfig::default(),
            media_pool: std::collections::HashMap::new(),
            media_folders: None,
            tracks: vec![track("v_main", TrackType::Video, vec![])],
            subtitles: None,
            markers: None,
        };

        let created = TrackOperations::add_track(&mut project, TrackType::Video, None).unwrap();

        assert_eq!(project.tracks[0].id, created.id);
        assert_eq!(project.tracks[1].id, "v_main");
    }

    #[test]
    fn add_track_video_with_near_track_id_inserts_directly_above_it_only() {
        let mut project = Project {
            metadata: crate::core::project::ProjectMetadata {
                id: "p".into(), name: "p".into(), version: "1".into(), revision: 1, created_at: 0, updated_at: 0,
            },
            config: crate::core::project::ProjectConfig::default(),
            media_pool: std::collections::HashMap::new(),
            media_folders: None,
            tracks: vec![
                track("v_extra2", TrackType::Video, vec![]),
                track("v_extra1", TrackType::Video, vec![]),
                track("v_main", TrackType::Video, vec![]),
            ],
            subtitles: None,
            markers: None,
        };

        let created = TrackOperations::add_track(&mut project, TrackType::Video, Some("v_extra1")).unwrap();

        let ids: Vec<&str> = project.tracks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["v_extra2", created.id.as_str(), "v_extra1", "v_main"]);
        assert_eq!(TrackOperations::main_video_track_id(&project), Some("v_main".to_string()));
    }

    #[test]
    fn add_track_audio_with_near_track_id_inserts_directly_below_it_only() {
        let mut project = Project {
            metadata: crate::core::project::ProjectMetadata {
                id: "p".into(), name: "p".into(), version: "1".into(), revision: 1, created_at: 0, updated_at: 0,
            },
            config: crate::core::project::ProjectConfig::default(),
            media_pool: std::collections::HashMap::new(),
            media_folders: None,
            tracks: vec![
                track("a_main", TrackType::Audio, vec![]),
                track("a_extra1", TrackType::Audio, vec![]),
                track("a_extra2", TrackType::Audio, vec![]),
            ],
            subtitles: None,
            markers: None,
        };

        let created = TrackOperations::add_track(&mut project, TrackType::Audio, Some("a_extra1")).unwrap();

        let ids: Vec<&str> = project.tracks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["a_main", "a_extra1", created.id.as_str(), "a_extra2"]);
        assert_eq!(TrackOperations::main_audio_track_id(&project), Some("a_main".to_string()));
    }

    #[test]
    fn add_track_falls_back_to_absolute_position_when_near_track_id_has_wrong_type() {
        let mut project = Project {
            metadata: crate::core::project::ProjectMetadata {
                id: "p".into(), name: "p".into(), version: "1".into(), revision: 1, created_at: 0, updated_at: 0,
            },
            config: crate::core::project::ProjectConfig::default(),
            media_pool: std::collections::HashMap::new(),
            media_folders: None,
            tracks: vec![
                track("v_main", TrackType::Video, vec![]),
                track("a_main", TrackType::Audio, vec![]),
            ],
            subtitles: None,
            markers: None,
        };

        let created = TrackOperations::add_track(&mut project, TrackType::Video, Some("a_main")).unwrap();

        assert_eq!(project.tracks[0].id, created.id);
    }

    #[test]
    fn add_track_inherits_height_from_near_track_or_existing_track() {
        let mut project = Project {
            metadata: crate::core::project::ProjectMetadata {
                id: "p".into(), name: "p".into(), version: "1".into(), revision: 1, created_at: 0, updated_at: 0,
            },
            config: crate::core::project::ProjectConfig::default(),
            media_pool: std::collections::HashMap::new(),
            media_folders: None,
            tracks: vec![
                Track {
                    id: "a_custom".into(),
                    name: "A1".into(),
                    track_type: TrackType::Audio,
                    locked: false,
                    muted: false,
                    hidden: false,
                    solo: false,
                    clips: vec![],
                    height: Some(72),
                },
            ],
            subtitles: None,
            markers: None,
        };

        // Adds new audio track without specific near_track_id, should inherit 72 from existing audio track
        let created = TrackOperations::add_track(&mut project, TrackType::Audio, None).unwrap();
        assert_eq!(created.height, Some(72));

        // Adds new audio track with near_track_id="a_custom", should inherit 72
        let created2 = TrackOperations::add_track(&mut project, TrackType::Audio, Some("a_custom")).unwrap();
        assert_eq!(created2.height, Some(72));
    }

    #[test]
    fn compact_track_packs_clips_from_zero_with_no_gaps() {
        let mut t = track("v1", TrackType::Video, vec![
            clip("b", 100, 20),
            clip("a", 10, 30),
            clip("c", 500, 5),
        ]);

        TrackOperations::compact_track(&mut t);

        let a = t.clips.iter().find(|c| c.id == "a").unwrap();
        let b = t.clips.iter().find(|c| c.id == "b").unwrap();
        let c = t.clips.iter().find(|c| c.id == "c").unwrap();
        assert_eq!(a.start_frame, 0);
        assert_eq!(b.start_frame, 30);
        assert_eq!(c.start_frame, 50);
    }

    #[test]
    fn compact_track_is_noop_on_already_packed_track() {
        let mut t = track("v1", TrackType::Video, vec![clip("a", 0, 10), clip("b", 10, 10)]);
        TrackOperations::compact_track(&mut t);
        assert_eq!(t.clips.iter().find(|c| c.id == "a").unwrap().start_frame, 0);
        assert_eq!(t.clips.iter().find(|c| c.id == "b").unwrap().start_frame, 10);
    }
}
