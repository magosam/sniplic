use crate::core::project::{Project, Track};
use crate::error::AppResult;
use crate::core::timeline::TimelineEngine;
use std::any::Any;

/// A reversible action that can be executed on a project.
pub trait Command: Any + Send + Sync {
    /// Executes the command, modifying the project.
    fn execute(&mut self, project: &mut Project) -> AppResult<()>;
    
    /// Reverts the modifications applied by this command.
    fn undo(&mut self, project: &mut Project) -> AppResult<()>;
    
    /// The display name of the command for the UI History panel.
    fn name(&self) -> &str;
    
    /// Attempts to merge this command with a new incoming command.
    /// Useful for debouncing continuous changes like volume sliders.
    /// Returns true if merged successfully.
    fn merge(&mut self, _other: &dyn Command) -> bool {
        false
    }

    /// Allows downcasting for merge implementations
    fn as_any(&self) -> &dyn Any;
}

/// Metadata for a history entry
pub struct HistoryEntry {
    pub id: String,
    pub timestamp: u64,
    pub command: Box<dyn Command>,
}

/// Manages a stack of commands to provide Undo and Redo functionality.
pub struct HistoryManager {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
}

impl HistoryManager {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn execute_command(&mut self, mut command: Box<dyn Command>, project: &mut Project) -> AppResult<()> {
        command.execute(project)?;
        
        // Attempt to merge with the last command in the stack
        if let Some(last_entry) = self.undo_stack.last_mut() {
            if last_entry.command.merge(command.as_ref()) {
                // Merged successfully, no need to push a new command
                self.redo_stack.clear();
                return Ok(());
            }
        }

        let entry = HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            command,
        };

        self.undo_stack.push(entry);
        self.redo_stack.clear(); // Any new action invalidates the redo future
        Ok(())
    }

    pub fn undo(&mut self, project: &mut Project) -> AppResult<bool> {
        if let Some(mut entry) = self.undo_stack.pop() {
            entry.command.undo(project)?;
            self.redo_stack.push(entry);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn redo(&mut self, project: &mut Project) -> AppResult<bool> {
        if let Some(mut entry) = self.redo_stack.pop() {
            entry.command.execute(project)?;
            self.undo_stack.push(entry);
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
// COMMAND IMPLEMENTATIONS
// ------------------------------------------------------------------

pub struct AddClipCommand {
    track_id: String,
    media_id: String,
    start_frame: u64,
    push: bool,
    image_duration_frames: Option<u64>,
    // State needed for Undo
    inserted_clip_id: Option<String>,
    track_snapshot: Option<Track>, // Snapshot of the track before insertion (to revert complex ripples)
}

impl AddClipCommand {
    pub fn new(track_id: String, media_id: String, start_frame: u64, push: bool, image_duration_frames: Option<u64>) -> Self {
        Self { 
            track_id, 
            media_id, 
            start_frame, 
            push, 
            image_duration_frames, 
            inserted_clip_id: None,
            track_snapshot: None,
        }
    }
}

impl Command for AddClipCommand {
    fn execute(&mut self, project: &mut Project) -> AppResult<()> {
        // Snapshot the track before we mess with it, just in case push/ripple changes many clips
        if let Some(track) = project.tracks.iter().find(|t| t.id == self.track_id) {
            self.track_snapshot = Some(track.clone());
        }

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
        // To perfectly revert any ripple push, we restore the track to its snapshot state
        if let Some(snapshot) = &self.track_snapshot {
            if let Some(track) = project.tracks.iter_mut().find(|t| t.id == self.track_id) {
                *track = snapshot.clone();
            }
        }
        Ok(())
    }

    fn name(&self) -> &str { "Add Clip" }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct RemoveClipCommand {
    clip_id: String,
    gapless: bool,
    // State needed for Undo
    track_id: Option<String>,
    track_snapshot: Option<Track>,
}

impl RemoveClipCommand {
    pub fn new(clip_id: String, gapless: bool) -> Self {
        Self { 
            clip_id, 
            gapless, 
            track_id: None,
            track_snapshot: None,
        }
    }
}

impl Command for RemoveClipCommand {
    fn execute(&mut self, project: &mut Project) -> AppResult<()> {
        // Snapshot the track before removing, so we can restore all rippled clips exactly
        for t in &project.tracks {
            if t.clips.iter().any(|c| c.id == self.clip_id) {
                self.track_id = Some(t.id.clone());
                self.track_snapshot = Some(t.clone());
                break;
            }
        }
        TimelineEngine::remove_clip(project, &self.clip_id, self.gapless)?;
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> AppResult<()> {
        if let (Some(track_id), Some(snapshot)) = (&self.track_id, &self.track_snapshot) {
            if let Some(track) = project.tracks.iter_mut().find(|t| t.id == *track_id) {
                *track = snapshot.clone();
            }
        }
        Ok(())
    }

    fn name(&self) -> &str { "Remove Clip" }
    fn as_any(&self) -> &dyn Any { self }
}

// ------------------------------------------------------------------
// PROOF OF CONCEPT: Mergeable Command (Volume Slider)
// ------------------------------------------------------------------

pub struct SetClipVolumeCommand {
    clip_id: String,
    new_volume: f32,
    old_volume: f32,
}

impl SetClipVolumeCommand {
    pub fn new(clip_id: String, new_volume: f32, old_volume: f32) -> Self {
        Self { clip_id, new_volume, old_volume }
    }
}

impl Command for SetClipVolumeCommand {
    fn execute(&mut self, project: &mut Project) -> AppResult<()> {
        for t in &mut project.tracks {
            if let Some(c) = t.clips.iter_mut().find(|c| c.id == self.clip_id) {
                c.audio.volume = self.new_volume;
            }
        }
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> AppResult<()> {
        for t in &mut project.tracks {
            if let Some(c) = t.clips.iter_mut().find(|c| c.id == self.clip_id) {
                c.audio.volume = self.old_volume;
            }
        }
        Ok(())
    }

    fn name(&self) -> &str { "Change Volume" }
    fn as_any(&self) -> &dyn Any { self }

    fn merge(&mut self, other: &dyn Command) -> bool {
        // If the next command is also a SetClipVolumeCommand for the same clip,
        // we absorb its new_volume, but keep our original old_volume!
        if let Some(other_cmd) = other.as_any().downcast_ref::<SetClipVolumeCommand>() {
            if self.clip_id == other_cmd.clip_id {
                self.new_volume = other_cmd.new_volume;
                return true;
            }
        }
        false
    }
}
