pub mod batch;
pub mod remove;
pub mod single;
pub mod truncate;
pub mod types;

use crate::core::project::Project;
use crate::error::AppResult;

pub use types::MoveClipItem;

pub struct MovementOperations;

impl MovementOperations {
    #[inline]
    pub fn move_clips_batch(
        project: &mut Project,
        moves: &[MoveClipItem],
        push: bool,
        gapless_enabled: bool,
    ) -> AppResult<bool> {
        batch::move_clips_batch(project, moves, push, gapless_enabled)
    }

    #[inline]
    pub fn move_clip(
        project: &mut Project,
        clip_id: &str,
        target_track_id: &str,
        new_start: u64,
        push: bool,
        gapless_enabled: bool,
    ) -> AppResult<()> {
        single::move_clip(project, clip_id, target_track_id, new_start, push, gapless_enabled)
    }

    #[inline]
    pub fn remove_clip(project: &mut Project, clip_id: &str, gapless_enabled: bool) -> AppResult<()> {
        remove::remove_clip(project, clip_id, gapless_enabled)
    }
}

#[cfg(test)]
mod tests;

