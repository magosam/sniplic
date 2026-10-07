use crate::core::project::Project;
use super::tracks::TrackOperations;
use std::collections::HashSet;

/// Sole responsibility of this file: decide WHERE the "Gapless" mode
/// (automatically close gaps) is allowed to operate. It is intentionally kept
/// separate from `ripple.rs` (which handles "Push", i.e., how to avoid overlaps)
/// — they are independent rules and each has its own file so they can be
/// modified without risk of breaking the other.
///
/// Rule: only the two MAIN and fixed tracks of the project (base V1 for
/// video/image and base A1 for audio) participate in "Gapless". Extra tracks
/// (V2+, A2+), created by the user on demand, are always free — clips on them
/// are never automatically shifted, even when the mode is active.
/// Identifying "which is the main track" uses the exact same rule as the
/// rest of the codebase (`TrackOperations::main_video_track_id` /
/// `main_audio_track_id`): the oldest video track (last of the video group,
/// since new ones are inserted at the top) and the oldest audio track (first of
/// the audio group, since new ones are appended at the end).

/// Returns the set of track IDs where "Gapless" is allowed to operate
/// in this project (at most 2: base V1 and base A1). Compute this BEFORE
/// any `&mut project.tracks` (e.g., before a `for track in &mut project.tracks`),
/// as only read access is required here.
pub fn gapless_track_ids(project: &Project) -> HashSet<String> {
    [
        TrackOperations::main_video_track_id(project),
        TrackOperations::main_audio_track_id(project),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Shortcut to check a single track (used when the ID is already known and
/// building the entire HashSet is unnecessary).
pub fn track_allows_gapless(project: &Project, track_id: &str) -> bool {
    TrackOperations::main_video_track_id(project).as_deref() == Some(track_id)
        || TrackOperations::main_audio_track_id(project).as_deref() == Some(track_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::project::{
        Clip, Project, ProjectConfig, ProjectMetadata, Track, TrackType,
    };
    use std::collections::HashMap;

    fn track(id: &str, track_type: TrackType) -> Track {
        Track {
            id: id.to_string(),
            name: id.to_string(),
            track_type,
            locked: false,
            muted: false,
            hidden: false,
            solo: false,
            clips: Vec::new(),
            height: None,
        }
    }

    #[allow(dead_code)]
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

    fn project(tracks: Vec<Track>) -> Project {
        Project {
            metadata: ProjectMetadata {
                id: "p".into(),
                name: "p".into(),
                version: "1".into(),
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

    #[test]
    fn only_main_tracks_are_gapless_eligible() {
        let p = project(vec![
            track("v_extra", TrackType::Video), // created later, stays at the top (index 0)
            track("v_main", TrackType::Video),  // original track, stays last in the group
            track("a_main", TrackType::Audio),  // first in the audio group
            track("a_extra", TrackType::Audio), // created later, stays at the end
        ]);

        let ids = gapless_track_ids(&p);
        assert!(ids.contains("v_main"));
        assert!(ids.contains("a_main"));
        assert!(!ids.contains("v_extra"));
        assert!(!ids.contains("a_extra"));

        assert!(track_allows_gapless(&p, "v_main"));
        assert!(track_allows_gapless(&p, "a_main"));
        assert!(!track_allows_gapless(&p, "v_extra"));
        assert!(!track_allows_gapless(&p, "a_extra"));
    }
}
