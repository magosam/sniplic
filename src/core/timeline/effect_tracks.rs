use crate::core::project::{Project, Track, TrackType};
use crate::error::AppResult;
use uuid::Uuid;

pub struct EffectTrackOperations;

impl EffectTrackOperations {
    /// Adds a new special compact filter track (half height, 22px, Filter type).
    /// Can be positioned anywhere above the main video track,
    /// including in-between existing tracks if `near_track_id` is specified.
    pub fn add_filter_track(
        project: &mut Project,
        near_track_id: Option<&str>,
    ) -> AppResult<Track> {
        let id = format!("trk_fx_{}", Uuid::new_v4().simple());
        let count = project.tracks.iter().filter(|t| t.track_type == TrackType::Filter).count() + 1;

        // Find the reference track position
        let near_idx = near_track_id.and_then(|nid| {
            project.tracks.iter().position(|t| t.id == nid)
        });

        let inherited_height = project
            .tracks
            .iter()
            .find(|t| t.track_type == TrackType::Filter)
            .and_then(|t| t.height)
            .or(Some(22));

        let track = Track {
            id: id.clone(),
            name: format!("FX {}", count),
            track_type: TrackType::Filter,
            locked: false,
            muted: false,
            hidden: false,
            solo: false,
            clips: Vec::new(),
            height: inherited_height,
        };

        let insert_idx = match near_idx {
            // If a reference track was indicated, insert at its position (pushing it down)
            Some(idx) => idx,
            // If not indicated, insert right below subtitles (always at absolute top of media tracks)
            None => 0,
        };

        project.tracks.insert(insert_idx, track.clone());
        Ok(track)
    }

    /// Returns whether a track is a special compact filter track
    pub fn is_filter_track(track: &Track) -> bool {
        track.track_type == TrackType::Filter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_filter_track_creates_compact_track() {
        let mut project = Project::new("Filter Test".to_string());
        let track = EffectTrackOperations::add_filter_track(&mut project, None).unwrap();

        assert_eq!(track.track_type, TrackType::Filter);
        assert_eq!(track.height, Some(22));
        assert!(track.name.starts_with("FX"));
        assert!(EffectTrackOperations::is_filter_track(&track));
    }

    #[test]
    fn add_filter_track_inserts_near_specified_track() {
        let mut project = Project::new("Filter Test 2".to_string());
        let v1_id = project.tracks[0].id.clone();
        let track = EffectTrackOperations::add_filter_track(&mut project, Some(&v1_id)).unwrap();

        assert_eq!(project.tracks[0].id, track.id);
        assert_eq!(track.track_type, TrackType::Filter);
        assert_eq!(track.height, Some(22));
    }
}
