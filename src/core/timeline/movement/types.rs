use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MoveClipItem {
    pub clip_id: String,
    pub target_track_id: String,
    pub new_start_frame: u64,
    /// ID of the EXTRA track that collided and triggered redirection to a
    /// new track (sentinels `__NEW_VIDEO_TRACK__`/`__NEW_AUDIO_TRACK__`
    /// in `target_track_id`) -- used to insert the new track next to
    /// it instead of at the absolute top/bottom (see
    /// `TrackOperations::add_track`). `None`/absent when the target is not
    /// a sentinel, or when the frontend had no reference track.
    #[serde(default)]
    pub near_track_id: Option<String>,
    /// Explicit direction for the push tiebreaker when inserting or moving a clip 
    /// exactly on the boundary of another clip ("left" or "right").
    #[serde(default)]
    pub push_direction: Option<String>,
}
