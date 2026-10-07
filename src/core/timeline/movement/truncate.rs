use crate::core::project::{Clip, Track};
use uuid::Uuid;

pub fn truncate_overlapping_clips(
    track: &mut Track,
    incoming_start: u64,
    incoming_duration: u64,
    ignore_clip_id: &str,
) {
    truncate_overlapping_clips_ignoring(
        track,
        incoming_start,
        incoming_duration,
        &[ignore_clip_id.to_string()],
    );
}

pub fn truncate_overlapping_clips_ignoring(
    track: &mut Track,
    incoming_start: u64,
    incoming_duration: u64,
    ignore_clip_ids: &[String],
) {
    let incoming_end = incoming_start + incoming_duration;
    let mut new_clips: Vec<Clip> = Vec::new();
    let mut split_additions: Vec<Clip> = Vec::new();

    for mut clip in track.clips.drain(..) {
        if ignore_clip_ids.iter().any(|id| id == &clip.id) {
            new_clips.push(clip);
            continue;
        }

        let clip_start = clip.start_frame;
        let clip_end = clip.start_frame + clip.duration_frames;

        if clip_end <= incoming_start || clip_start >= incoming_end {
            new_clips.push(clip);
            continue;
        }

        if clip_start >= incoming_start && clip_end <= incoming_end {
            continue;
        }

        if clip_start < incoming_start && clip_end > incoming_end {
            let left_dur = incoming_start - clip_start;
            let mut left_piece = clip.clone();
            left_piece.duration_frames = left_dur;
            left_piece.out_point_frames = left_piece.in_point_frames + left_dur;
            new_clips.push(left_piece);

            let right_offset = incoming_end - clip_start;
            let right_dur = clip_end - incoming_end;
            let mut right_piece = clip.clone();
            right_piece.id = format!("clp_{}", Uuid::new_v4().simple());
            right_piece.start_frame = incoming_end;
            right_piece.duration_frames = right_dur;
            right_piece.in_point_frames += right_offset;
            right_piece.out_point_frames = right_piece.in_point_frames + right_dur;
            split_additions.push(right_piece);
            continue;
        }

        if clip_start < incoming_start && clip_end <= incoming_end {
            let new_dur = incoming_start - clip_start;
            clip.duration_frames = new_dur;
            clip.out_point_frames = clip.in_point_frames + new_dur;
            new_clips.push(clip);
            continue;
        }

        if clip_start >= incoming_start && clip_end > incoming_end {
            let cut_dur = incoming_end - clip_start;
            clip.start_frame = incoming_end;
            clip.duration_frames = clip.duration_frames.saturating_sub(cut_dur);
            clip.in_point_frames += cut_dur;
            new_clips.push(clip);
            continue;
        }
    }

    new_clips.extend(split_additions);
    new_clips.sort_by_key(|c| c.start_frame);
    track.clips = new_clips;
}
