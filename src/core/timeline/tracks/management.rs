use crate::core::project::{Project, Track, TrackType};
use crate::error::AppResult;
use uuid::Uuid;

/// Reindexes track names: Vn...V1 for video (top to bottom) and A1...An for audio
pub fn reindex_tracks(project: &mut Project) {
    let video_count = project
        .tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Video)
        .count();
    let filter_count = project
        .tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Filter)
        .count();
    let mut cur_v = video_count;
    let mut cur_fx = filter_count;
    let mut cur_a = 1;

    for t in project.tracks.iter_mut() {
        match t.track_type {
            TrackType::Filter => {
                t.name = format!("FX {}", cur_fx);
                cur_fx = cur_fx.saturating_sub(1);
            }
            TrackType::Video => {
                t.name = format!("V{}", cur_v);
                cur_v = cur_v.saturating_sub(1);
            }
            TrackType::Audio => {
                t.name = format!("A{}", cur_a);
                cur_a += 1;
            }
        }
    }
}

/// Removes empty extra tracks (V2+, A2+, empty FX), always keeping V1 and A1 fixed
pub fn cleanup_empty_tracks(project: &mut Project) {
    let mut final_filters: Vec<Track> = Vec::new();
    let mut videos: Vec<Track> = Vec::new();
    let mut audios: Vec<Track> = Vec::new();

    for track in std::mem::take(&mut project.tracks) {
        match track.track_type {
            TrackType::Filter => {
                if !track.clips.is_empty() {
                    final_filters.push(track);
                }
            }
            TrackType::Video => videos.push(track),
            TrackType::Audio => audios.push(track),
        }
    }

    let mut final_videos: Vec<Track> = Vec::new();
    let video_len = videos.len();
    for (idx, track) in videos.into_iter().enumerate() {
        let is_base_track = idx + 1 == video_len;
        if is_base_track || !track.clips.is_empty() {
            final_videos.push(track);
        }
    }

    if final_videos.is_empty() {
        final_videos.push(Track {
            id: format!("trk_{}", Uuid::new_v4().simple()),
            name: "V1".to_string(),
            track_type: TrackType::Video,
            locked: false,
            muted: false,
            hidden: false,
            solo: false,
            clips: Vec::new(),
            height: None,
        });
    }

    let mut final_audios: Vec<Track> = Vec::new();
    for (idx, track) in audios.into_iter().enumerate() {
        let is_base_track = idx == 0;
        if is_base_track || !track.clips.is_empty() {
            final_audios.push(track);
        }
    }

    if final_audios.is_empty() {
        final_audios.push(Track {
            id: format!("trk_{}", Uuid::new_v4().simple()),
            name: "A1".to_string(),
            track_type: TrackType::Audio,
            locked: false,
            muted: false,
            hidden: false,
            solo: false,
            clips: Vec::new(),
            height: None,
        });
    }

    let mut all_tracks = final_filters;
    all_tracks.extend(final_videos);
    all_tracks.extend(final_audios);
    project.tracks = all_tracks;

    reindex_tracks(project);
}

/// Creates a new track. If `near_track_id` points to an existing track
/// OF THE SAME TYPE, the new track is inserted next to it --
/// above, if video; below, if audio -- instead of always at the
/// absolute top/bottom of the group.
pub fn add_track(
    project: &mut Project,
    track_type: TrackType,
    near_track_id: Option<&str>,
) -> AppResult<Track> {
    let is_v = track_type == TrackType::Video;
    let id = format!("trk_{}", Uuid::new_v4().simple());
    let count = project.tracks.iter().filter(|t| t.track_type == track_type).count() + 1;

    // Needs to be calculated BEFORE moving `track_type` into the
    // `Track` struct below (TrackType does not implement Copy).
    let near_idx = near_track_id.and_then(|nid| {
        project
            .tracks
            .iter()
            .position(|t| t.id == nid && t.track_type == track_type)
    });

    let inherited_height = near_idx
        .and_then(|idx| project.tracks.get(idx).and_then(|t| t.height))
        .or_else(|| {
            project
                .tracks
                .iter()
                .find(|t| t.track_type == track_type)
                .and_then(|t| t.height)
        });

    let track = Track {
        id: id.clone(),
        name: if is_v { format!("V{}", count) } else { format!("A{}", count) },
        track_type,
        locked: false,
        muted: false,
        hidden: false,
        solo: false,
        clips: Vec::new(),
        height: inherited_height,
    };

    let insert_idx = match near_idx {
        // Video: the new track goes ABOVE the reference track --
        // index 0 is the top of the video group, so inserting AT its
        // index pushes it (and whatever comes after) down by one index.
        Some(idx) if is_v => idx,
        // Audio: the new track goes BELOW the reference track --
        // index right after it.
        Some(idx) => idx + 1,
        // Without a valid reference track: inserts at the top of the video group,
        // but ALWAYS below any existing filter tracks.
        None if is_v => {
            project.tracks.iter().position(|t| t.track_type == TrackType::Video).unwrap_or_else(|| {
                project.tracks.iter().filter(|t| t.track_type == TrackType::Filter).count()
            })
        },
        None => project.tracks.len(),
    };

    project.tracks.insert(insert_idx, track.clone());

    reindex_tracks(project);
    Ok(track)
}
