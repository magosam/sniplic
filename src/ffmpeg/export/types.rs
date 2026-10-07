use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextKeyframeExportItem {
    pub frame: u64,
    #[serde(default)]
    pub pos_x: Option<f32>,
    #[serde(default)]
    pub pos_y: Option<f32>,
    #[serde(default)]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub opacity: Option<f32>,
    #[serde(default)]
    pub rotation: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleWordExportItem {
    pub text: String,
    pub start_frame: u64,
    pub end_frame: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleExportItem {
    pub start_frame: u64,
    pub end_frame: u64,
    pub text: String,
    #[serde(default)]
    pub is_unlocked: Option<bool>,
    #[serde(default)]
    pub pos_x: Option<f32>,
    #[serde(default)]
    pub pos_y: Option<f32>,
    #[serde(default)]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub box_width: Option<f32>,
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub italic: Option<bool>,
    #[serde(default)]
    pub underline: Option<bool>,
    #[serde(default)]
    pub align_x: Option<String>,
    #[serde(default)]
    pub border_style: Option<String>,
    #[serde(default)]
    pub border_color: Option<String>,
    #[serde(default)]
    pub border_width: Option<f32>,
    #[serde(default)]
    pub bg_enabled: Option<bool>,
    #[serde(default)]
    pub bg_color: Option<String>,
    #[serde(default)]
    pub bg_opacity: Option<f32>,
    #[serde(default)]
    pub rotation: Option<f32>,
    #[serde(default)]
    pub letter_spacing: Option<f32>,
    #[serde(default)]
    pub keyframes: Option<Vec<TextKeyframeExportItem>>,
    #[serde(default)]
    pub animation_in: Option<String>,
    #[serde(default)]
    pub animation_out: Option<String>,
    #[serde(default)]
    pub animation_duration: Option<f32>,
    #[serde(default)]
    pub animated_captions_enabled: Option<bool>,
    #[serde(default)]
    pub animated_captions_style: Option<String>,
    #[serde(default)]
    pub words: Option<Vec<SubtitleWordExportItem>>,
    #[serde(default)]
    pub generated_by_ai: Option<bool>,
    #[serde(default)]
    pub single_line_enabled: Option<bool>,
    #[serde(default)]
    pub words_per_batch: Option<u32>,
    #[serde(default)]
    pub words_batch_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleExportConfig {
    #[serde(default = "default_sub_pos_x")]
    pub pos_x: f32,
    #[serde(default = "default_sub_pos_y")]
    pub pos_y: f32,
    #[serde(default = "default_sub_font_size")]
    pub font_size: f32,
    #[serde(default = "default_sub_box_width")]
    pub box_width: Option<f32>,
    #[serde(default)]
    pub max_chars: Option<u32>,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub italic: Option<bool>,
    #[serde(default)]
    pub underline: Option<bool>,
    #[serde(default)]
    pub align_x: Option<String>,
    #[serde(default)]
    pub border_style: Option<String>,
    #[serde(default)]
    pub border_color: Option<String>,
    #[serde(default)]
    pub border_width: Option<f32>,
    #[serde(default)]
    pub bg_enabled: Option<bool>,
    #[serde(default)]
    pub bg_color: Option<String>,
    #[serde(default)]
    pub bg_opacity: Option<f32>,
    #[serde(default)]
    pub animated_captions_enabled: Option<bool>,
    #[serde(default)]
    pub animated_captions_style: Option<String>,
    #[serde(default)]
    pub single_line_enabled: Option<bool>,
    #[serde(default)]
    pub words_per_batch: Option<u32>,
    #[serde(default)]
    pub words_batch_enabled: Option<bool>,
}

pub fn default_sub_pos_x() -> f32 { 50.0 }
pub fn default_sub_pos_y() -> f32 { 82.0 }
pub fn default_sub_font_size() -> f32 { 18.0 }
pub fn default_sub_box_width() -> Option<f32> { Some(68.0) }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSettings {
    pub name: String,
    pub output_path: String,
    pub include_video: bool,
    pub resolution: String,
    pub codec: String,
    pub format: String,
    pub fps: f64,
    pub include_audio: bool,
    pub audio_codec: String,
    pub audio_format: String,
    pub audio_bitrate_kbps: u32,
    #[serde(default)]
    pub subtitles: Option<Vec<SubtitleExportItem>>,
    #[serde(default)]
    pub subtitle_config: Option<SubtitleExportConfig>,
    #[serde(default)]
    pub preview_width: Option<f64>,
    #[serde(default)]
    pub preview_height: Option<f64>,
}

#[derive(Clone)]
pub struct AudioRenderItem {
    pub file_path: PathBuf,
    pub start_frame: u64,
    pub in_point_frames: u64,
    pub duration_frames: u64,
    pub volume: f32,
    pub reversed: bool,
    pub fade_in_frames: u32,
    pub fade_out_frames: u32,
    pub speed: f64,
    pub eq_bass: f32,
    pub eq_mid: f32,
    pub eq_treble: f32,
    pub denoise: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgressUpdate {
    pub percentage: f64,
    pub stage: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
}
