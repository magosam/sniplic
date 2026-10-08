use crate::core::project::Project;
use crate::error::AppResult;

/// A reversible action that can be executed on a project.
pub trait Command: Send + Sync {
    /// Executes the command, modifying the project.
    fn execute(&mut self, project: &mut Project) -> AppResult<()>;
    /// Reverts the modifications applied by this command.
    fn undo(&mut self, project: &mut Project) -> AppResult<()>;
}

/// Manages a stack of commands to provide Undo and Redo functionality without serializing the whole project.
pub struct HistoryManager {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
}

impl HistoryManager {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Executes a command and registers it in the history stack.
    pub fn execute_command(&mut self, mut command: Box<dyn Command>, project: &mut Project) -> AppResult<()> {
        command.execute(project)?;
        self.undo_stack.push(command);
        self.redo_stack.clear(); // Any new action invalidates the redo future
        Ok(())
    }

    /// Pops the last command from the undo stack and calls undo() on it.
    pub fn undo(&mut self, project: &mut Project) -> AppResult<bool> {
        if let Some(mut command) = self.undo_stack.pop() {
            command.undo(project)?;
            self.redo_stack.push(command);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Pops the last undone command from the redo stack and calls execute() on it.
    pub fn redo(&mut self, project: &mut Project) -> AppResult<bool> {
        if let Some(mut command) = self.redo_stack.pop() {
            command.execute(project)?;
            self.undo_stack.push(command);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

// ------------------------------------------------------------------
// PROOF OF CONCEPT: Command Implementations
// ------------------------------------------------------------------

use crate::core::timeline::TimelineEngine;
use crate::core::project::Clip;

pub struct AddClipCommand {
    track_id: String,
    media_id: String,
    start_frame: u64,
    push: bool,
    image_duration_frames: Option<u64>,
    // State needed for Undo
    inserted_clip_id: Option<String>,
}

impl AddClipCommand {
    pub fn new(track_id: String, media_id: String, start_frame: u64, push: bool, image_duration_frames: Option<u64>) -> Self {
        Self { 
            track_id, 
            media_id, 
            start_frame, 
            push, 
            image_duration_frames, 
            inserted_clip_id: None 
        }
    }
}

impl Command for AddClipCommand {
    fn execute(&mut self, project: &mut Project) -> AppResult<()> {
        let clip = TimelineEngine::add_clip(
            project, 
            &self.track_id, 
            &self.media_id, 
            self.start_frame, 
            self.push, 
            self.image_duration_frames
        )?;
        self.inserted_clip_id = Some(clip.id);
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> AppResult<()> {
        if let Some(clip_id) = &self.inserted_clip_id {
            TimelineEngine::remove_clip(project, clip_id, self.push)?;
        }
        Ok(())
    }
}

pub struct RemoveClipCommand {
    clip_id: String,
    gapless: bool,
    // State needed for Undo
    removed_clip: Option<Clip>,
    track_id: Option<String>,
}

impl RemoveClipCommand {
    pub fn new(clip_id: String, gapless: bool) -> Self {
        Self { 
            clip_id, 
            gapless, 
            removed_clip: None, 
            track_id: None 
        }
    }
}

impl Command for RemoveClipCommand {
    fn execute(&mut self, project: &mut Project) -> AppResult<()> {
        // Save the clip state before removing it
        for t in &project.tracks {
            if let Some(c) = t.clips.iter().find(|c| c.id == self.clip_id) {
                self.removed_clip = Some(c.clone());
                self.track_id = Some(t.id.clone());
                break;
            }
        }
        TimelineEngine::remove_clip(project, &self.clip_id, self.gapless)?;
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> AppResult<()> {
        if let (Some(clip), Some(track_id)) = (&self.removed_clip, &self.track_id) {
            let track = project.tracks.iter_mut().find(|t| t.id == *track_id)
                .ok_or_else(|| crate::error::AppError::InvalidOperation("Track not found for undo".to_string()))?;
            track.clips.push(clip.clone());
            track.clips.sort_by_key(|c| c.start_frame);
            // Note: If gapless was true, restoring the ripple shift would require moving the other clips back.
            // For a robust implementation, complex ripples can clone the Track state before execution instead.
        }
        Ok(())
    }
}
