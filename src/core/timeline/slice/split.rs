use crate::core::project::{Clip, Project};
use crate::error::{AppError, AppResult};
use uuid::Uuid;

pub fn split_clip(
    project: &mut Project,
    clip_id: &str,
    split_frame: u64,
) -> AppResult<(Clip, Clip)> {
    for track in &mut project.tracks {
        if let Some(pos) = track.clips.iter().position(|c| c.id == clip_id) {
            if track.locked {
                return Err(AppError::InvalidOperation(format!("A faixa '{}' está bloqueada", track.name)));
            }
            let orig = &track.clips[pos];
            if split_frame <= orig.start_frame || split_frame >= orig.start_frame + orig.duration_frames {
                return Err(AppError::InvalidOperation("Ponto de corte fora dos limites".to_string()));
            }
            let first_dur = split_frame - orig.start_frame;
            let second_dur = orig.duration_frames - first_dur;

            let mut first = orig.clone();
            first.duration_frames = first_dur;
            first.out_point_frames = orig.in_point_frames + first_dur;

            let mut second = orig.clone();
            second.id = format!("clp_{}", Uuid::new_v4().simple());
            second.start_frame = split_frame;
            second.duration_frames = second_dur;
            second.in_point_frames = orig.in_point_frames + first_dur;

            track.clips[pos] = first.clone();
            track.clips.insert(pos + 1, second.clone());
            return Ok((first, second));
        }
    }
    Err(AppError::InvalidOperation("Clipe não encontrado".to_string()))
}

pub fn split_at_playhead(
    project: &mut Project,
    split_frame: u64,
    split_all: bool,
    selected_clip_ids: &[String],
) -> AppResult<bool> {
    let mut target_clip_ids = Vec::new();

    for track in &project.tracks {
        for clip in &track.clips {
            let inside = split_frame > clip.start_frame && split_frame < clip.start_frame + clip.duration_frames;
            if inside {
                if split_all {
                    target_clip_ids.push(clip.id.clone());
                } else if selected_clip_ids.contains(&clip.id) {
                    target_clip_ids.push(clip.id.clone());
                } else if selected_clip_ids.is_empty() && target_clip_ids.is_empty() {
                    target_clip_ids.push(clip.id.clone());
                }
            }
        }
    }

    if target_clip_ids.is_empty() {
        return Ok(false);
    }

    for id in target_clip_ids {
        let _ = split_clip(project, &id, split_frame);
    }

    Ok(true)
}
