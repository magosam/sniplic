use crate::core::project::Project;
use crate::error::AppResult;
use super::batch::move_clips_batch;
use super::types::MoveClipItem;

pub fn move_clip(
    project: &mut Project,
    clip_id: &str,
    target_track_id: &str,
    new_start: u64,
    push: bool,
    gapless_enabled: bool,
) -> AppResult<()> {
    let moves = vec![MoveClipItem {
        clip_id: clip_id.to_string(),
        target_track_id: target_track_id.to_string(),
        new_start_frame: new_start,
        near_track_id: None,
    }];
    move_clips_batch(project, &moves, push, gapless_enabled)?;
    Ok(())
}
