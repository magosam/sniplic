use crate::core::project::Clip;

/// Pushes to the RIGHT, by the **minimum necessary**, clips on a track
/// (with `start_frame >= from_start_inclusive`) that would be overlapped by
/// a freshly inserted/moved interval `[from_start_inclusive, incoming_end)`.
///
/// Unlike "push everything by the full duration of the new clip", this
/// opens exactly the missing space: if there was already enough free space
/// before the next clip, it is not even touched. The cascade stops as soon as
/// it encounters a clip that already has enough space, because the original
/// non-overlapping guarantee ensures that the remainder is also OK.
pub fn push_right_minimal(clips: &mut Vec<Clip>, from_start_inclusive: u64, incoming_end: u64) {
    clips.sort_by_key(|c| c.start_frame);
    let mut cursor = incoming_end;
    for c in clips.iter_mut() {
        if c.start_frame < from_start_inclusive {
            continue;
        }
        if c.start_frame < cursor {
            c.start_frame = cursor;
            cursor = c.start_frame + c.duration_frames;
        } else {
            break;
        }
    }
}

/// Mirror of `push_right_minimal`: pushes to the LEFT, by the minimum
/// necessary, clips on a track (with `start_frame < before_start_exclusive`)
/// whose end would exceed `max_end_allowed`. Used when the user stretches the
/// left edge of a clip backwards and that would encroach on the previous clip.
pub fn push_left_minimal(clips: &mut Vec<Clip>, before_start_exclusive: u64, max_end_allowed: u64) {
    clips.sort_by_key(|c| c.start_frame);
    let mut cursor = max_end_allowed;
    for c in clips.iter_mut().rev() {
        if c.start_frame >= before_start_exclusive {
            continue;
        }
        let end = c.start_frame + c.duration_frames;
        if end > cursor {
            let shift = end - cursor;
            c.start_frame = c.start_frame.saturating_sub(shift);
            cursor = c.start_frame;
        } else {
            break;
        }
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

    #[test]
    fn push_right_minimal_does_nothing_when_gap_is_enough() {
        let mut clips = vec![clip("a", 50, 20)];
        push_right_minimal(&mut clips, 40, 50); // incoming 40..50, touches exactly at 50
        assert_eq!(clips[0].start_frame, 50);
    }

    #[test]
    fn push_right_minimal_opens_only_the_missing_space() {
        let mut clips = vec![clip("a", 45, 20)];
        push_right_minimal(&mut clips, 40, 50); // incoming 40..50, overlap of 5 (45..50)
        assert_eq!(clips[0].start_frame, 50); // 45 + 5, not 45+10
    }

    #[test]
    fn push_right_minimal_cascades_through_chain() {
        let mut clips = vec![clip("a", 45, 5), clip("b", 50, 5)]; // a:45..50, b:50..55 (adjacent)
        push_right_minimal(&mut clips, 40, 50); // incoming 40..50, overlap of 5 with "a"
        let a = clips.iter().find(|c| c.id == "a").unwrap();
        let b = clips.iter().find(|c| c.id == "b").unwrap();
        assert_eq!(a.start_frame, 50); // pushed the minimum (5)
        assert_eq!(b.start_frame, 55); // pushed in cascade to prevent overlap with "a"
    }

    #[test]
    fn push_left_minimal_does_nothing_when_no_overlap() {
        let mut clips = vec![clip("a", 10, 20)]; // 10..30
        push_left_minimal(&mut clips, 35, 32); // stretched clip now starts at 32, no overlap with end of "a" (30)
        assert_eq!(clips[0].start_frame, 10); // there was no overlap (30 <= 32), nothing changes
    }

    #[test]
    fn push_left_minimal_shifts_when_overlapping() {
        let mut clips = vec![clip("a", 10, 25)]; // 10..35
        push_left_minimal(&mut clips, 35, 30); // stretched clip now starts at 30, overlap of 5 with "a" (which ends at 35)
        assert_eq!(clips[0].start_frame, 5); // 10 - 5
    }
}
