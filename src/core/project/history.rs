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
        
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;

        // Attempt to merge with the last command in the stack
        if let Some(last_entry) = self.undo_stack.last_mut() {
            // Only allow merge if the last command occurred within a 2-second window (2000ms)
            if now.saturating_sub(last_entry.timestamp) <= 2000 {
                if last_entry.command.merge(command.as_ref()) {
                    // Merged successfully, renew the timestamp and we don't push a new command
                    last_entry.timestamp = now;
                    self.redo_stack.clear();
                    return Ok(());
                }
            }
        }

        let entry = HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now,
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

// ------------------------------------------------------------------
// MOVEMENT AND SLICING COMMANDS
// ------------------------------------------------------------------

pub struct MoveClipCommand {
    clip_id: String,
    target_track_id: String,
    new_start: u64,
    push: bool,
    gapless: bool,
    // Undo state: We snapshot both the source track and the target track
    source_track_id: Option<String>,
    source_track_snapshot: Option<Track>,
    target_track_snapshot: Option<Track>,
}

impl MoveClipCommand {
    pub fn new(clip_id: String, target_track_id: String, new_start: u64, push: bool, gapless: bool) -> Self {
        Self {
            clip_id,
            target_track_id,
            new_start,
            push,
            gapless,
            source_track_id: None,
            source_track_snapshot: None,
            target_track_snapshot: None,
        }
    }
}

impl Command for MoveClipCommand {
    fn execute(&mut self, project: &mut Project) -> AppResult<()> {
        // Snapshot the tracks involved
        for t in &project.tracks {
            if t.clips.iter().any(|c| c.id == self.clip_id) {
                self.source_track_id = Some(t.id.clone());
                self.source_track_snapshot = Some(t.clone());
                break;
            }
        }
        if let Some(target) = project.tracks.iter().find(|t| t.id == self.target_track_id) {
            self.target_track_snapshot = Some(target.clone());
        }

        TimelineEngine::move_clip(project, &self.clip_id, &self.target_track_id, self.new_start, self.push, self.gapless)?;
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> AppResult<()> {
        if let (Some(s_id), Some(s_snap)) = (&self.source_track_id, &self.source_track_snapshot) {
            if let Some(track) = project.tracks.iter_mut().find(|t| t.id == *s_id) {
                *track = s_snap.clone();
            }
        }
        // If the source and target tracks are different, restore the target track too
        if Some(&self.target_track_id) != self.source_track_id.as_ref() {
            if let Some(t_snap) = &self.target_track_snapshot {
                if let Some(track) = project.tracks.iter_mut().find(|t| t.id == self.target_track_id) {
                    *track = t_snap.clone();
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &str { "Move Clip" }
    fn as_any(&self) -> &dyn Any { self }

    fn merge(&mut self, other: &dyn Command) -> bool {
        // Allows debouncing continuous dragging of a clip
        if let Some(other_cmd) = other.as_any().downcast_ref::<MoveClipCommand>() {
            if self.clip_id == other_cmd.clip_id && self.target_track_id == other_cmd.target_track_id 
               && self.push == other_cmd.push && self.gapless == other_cmd.gapless {
                self.new_start = other_cmd.new_start;
                return true;
            }
        }
        false
    }
}

pub struct SplitClipCommand {
    clip_id: String,
    split_frame: u64,
    track_id: Option<String>,
    track_snapshot: Option<Track>,
}

impl SplitClipCommand {
    pub fn new(clip_id: String, split_frame: u64) -> Self {
        Self { clip_id, split_frame, track_id: None, track_snapshot: None }
    }
}

impl Command for SplitClipCommand {
    fn execute(&mut self, project: &mut Project) -> AppResult<()> {
        for t in &project.tracks {
            if t.clips.iter().any(|c| c.id == self.clip_id) {
                self.track_id = Some(t.id.clone());
                self.track_snapshot = Some(t.clone());
                break;
            }
        }
        TimelineEngine::split_clip(project, &self.clip_id, self.split_frame)?;
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> AppResult<()> {
        if let (Some(t_id), Some(snap)) = (&self.track_id, &self.track_snapshot) {
            if let Some(track) = project.tracks.iter_mut().find(|t| t.id == *t_id) {
                *track = snap.clone();
            }
        }
        Ok(())
    }

    fn name(&self) -> &str { "Split Clip" }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct TrimClipCommand {
    clip_id: String,
    edge: String, // "left" or "right"
    target_frame: u64,
    push: bool,
    snap: bool,
    gapless: bool,
    track_id: Option<String>,
    track_snapshot: Option<Track>,
}

impl TrimClipCommand {
    pub fn new(clip_id: String, edge: String, target_frame: u64, push: bool, snap: bool, gapless: bool) -> Self {
        Self { clip_id, edge, target_frame, push, snap, gapless, track_id: None, track_snapshot: None }
    }
}

impl Command for TrimClipCommand {
    fn execute(&mut self, project: &mut Project) -> AppResult<()> {
        for t in &project.tracks {
            if t.clips.iter().any(|c| c.id == self.clip_id) {
                self.track_id = Some(t.id.clone());
                self.track_snapshot = Some(t.clone());
                break;
            }
        }
        TimelineEngine::trim_clip(project, &self.clip_id, &self.edge, self.target_frame, self.push, self.snap, self.gapless)?;
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> AppResult<()> {
        if let (Some(t_id), Some(snap)) = (&self.track_id, &self.track_snapshot) {
            if let Some(track) = project.tracks.iter_mut().find(|t| t.id == *t_id) {
                *track = snap.clone();
            }
        }
        Ok(())
    }

    fn name(&self) -> &str { "Trim Clip" }
    fn as_any(&self) -> &dyn Any { self }

    fn merge(&mut self, other: &dyn Command) -> bool {
        // Allows debouncing continuous dragging of a trim handle
        if let Some(other_cmd) = other.as_any().downcast_ref::<TrimClipCommand>() {
            if self.clip_id == other_cmd.clip_id && self.edge == other_cmd.edge 
               && self.push == other_cmd.push && self.gapless == other_cmd.gapless {
                self.target_frame = other_cmd.target_frame;
                return true;
            }
        }
        false
    }
}
