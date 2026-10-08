import re

with open("src/core/project/history.rs", "r", encoding="utf-8") as f:
    code = f.read()

# AddClipCommand
code = re.sub(
    r"pub struct AddClipCommand \{([\s\S]*?)image_duration_frames: Option<u64>,",
    r"pub struct AddClipCommand {\1image_duration_frames: Option<u64>,\n    push_direction: Option<String>,",
    code
)
code = re.sub(
    r"pub fn new\(track_id: String, media_id: String, start_frame: u64, push: bool, image_duration_frames: Option<u64>\) -> Self \{([\s\S]*?)image_duration_frames,\n",
    r"pub fn new(track_id: String, media_id: String, start_frame: u64, push: bool, image_duration_frames: Option<u64>, push_direction: Option<String>) -> Self {\1image_duration_frames,\n            push_direction,\n",
    code
)
code = re.sub(
    r"self\.image_duration_frames,\n\s*\)\?;",
    r"self.image_duration_frames,\n            self.push_direction.clone(),\n        )?;",
    code
)

# MoveClipCommand
code = re.sub(
    r"pub struct MoveClipCommand \{([\s\S]*?)gapless_enabled: bool,",
    r"pub struct MoveClipCommand {\1gapless_enabled: bool,\n    push_direction: Option<String>,",
    code
)
code = re.sub(
    r"pub fn new\(clip_id: String, target_track_id: String, new_start: u64, push: bool, gapless: bool\) -> Self \{([\s\S]*?)gapless_enabled: gapless,\n",
    r"pub fn new(clip_id: String, target_track_id: String, new_start: u64, push: bool, gapless: bool, push_direction: Option<String>) -> Self {\1gapless_enabled: gapless,\n            push_direction,\n",
    code
)
code = re.sub(
    r"self\.push,\n\s*self\.gapless_enabled,\n\s*\)\?;",
    r"self.push,\n            self.gapless_enabled,\n            self.push_direction.clone(),\n        )?;",
    code
)

# AddClipsBatchCommand
code = re.sub(
    r"pub struct AddClipsBatchCommand \{([\s\S]*?)image_duration_frames: Option<u64>,",
    r"pub struct AddClipsBatchCommand {\1image_duration_frames: Option<u64>,\n    push_direction: Option<String>,",
    code
)
code = re.sub(
    r"pub fn new\(media_ids: Vec<String>, start_frame: u64, target_track_id: Option<String>, push: bool, image_duration_frames: Option<u64>\) -> Self \{([\s\S]*?)image_duration_frames,\n",
    r"pub fn new(media_ids: Vec<String>, start_frame: u64, target_track_id: Option<String>, push: bool, image_duration_frames: Option<u64>, push_direction: Option<String>) -> Self {\1image_duration_frames,\n            push_direction,\n",
    code
)

# Wait, the execute block for AddClipsBatchCommand
code = re.sub(
    r"TimelineEngine::add_clips_batch\(\s*project,\n\s*&self\.media_ids,\n\s*self\.start_frame,\n\s*self\.target_track_id\.clone\(\),\n\s*self\.push,\n\s*self\.image_duration_frames,\n\s*\)\?;",
    r"TimelineEngine::add_clips_batch(\n            project,\n            &self.media_ids,\n            self.start_frame,\n            self.target_track_id.clone(),\n            self.push,\n            self.image_duration_frames,\n            self.push_direction.clone(),\n        )?;",
    code
)

with open("src/core/project/history.rs", "w", encoding="utf-8") as f:
    f.write(code)
