use crate::core::project::Clip;

/// Maximum distance (in frames) for a trim target to snap to the edge of
/// a neighboring clip on the same track. It is deliberately small: the goal is
/// an exact fit (zero gap / zero overlap) when the user is already close
/// to the edge, not to replace the visual pixel snap that the UI performs before
/// calling this command.
pub const TRIM_SNAP_THRESHOLD_FRAMES: u64 = 6;

/// Snaps `target_frame` to the edge of a neighboring clip on the same track
/// (start or end) if it is within the snap threshold. Ignores the
/// clip currently being trimmed. Returns the original frame if nothing is
/// close enough.
pub fn snap_to_neighbor_edge(
    track_clips: &[Clip],
    self_clip_id: &str,
    target_frame: u64,
) -> u64 {
    let mut best: Option<(u64, u64)> = None; // (distance, candidate frame)

    for c in track_clips {
        if c.id == self_clip_id {
            continue;
        }
        for candidate in [c.start_frame, c.start_frame + c.duration_frames] {
            let dist = target_frame.abs_diff(candidate);
            if dist <= TRIM_SNAP_THRESHOLD_FRAMES {
                if best.map(|(d, _)| dist < d).unwrap_or(true) {
                    best = Some((dist, candidate));
                }
            }
        }
    }

    best.map(|(_, frame)| frame).unwrap_or(target_frame)
}
