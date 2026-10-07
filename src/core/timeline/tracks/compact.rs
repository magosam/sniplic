use crate::core::project::Track;

/// Removes all gaps from a track, packing clips starting from
/// frame 0 (the first clip goes to 0, each subsequent clip snaps to the end of
/// the previous one). Used by "Gapless" mode when activated.
pub fn compact_track(track: &mut Track) {
    track.clips.sort_by_key(|c| c.start_frame);
    let mut cursor: u64 = 0;
    for c in track.clips.iter_mut() {
        c.start_frame = cursor;
        cursor += c.duration_frames;
    }
}
