use serde::{Deserialize, Serialize};



#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TextKeyframeValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextKeyframe {
    pub id: String,
    pub frame: u64,
    pub value: TextKeyframeValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub easing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleWord {
    pub text: String,
    pub start_frame: u64,
    pub end_frame: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleBlock {
    pub id: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_unlocked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<SubtitlePosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_x: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_y: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_padding_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_padding_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_style_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyframes: Option<Vec<TextKeyframe>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation_out: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation_duration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animated_captions_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animated_captions_style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<SubtitleWord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by_ai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_line_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words_per_batch: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words_batch_enabled: Option<bool>,
}

impl<'de> Deserialize<'de> for SubtitleBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or_default().to_string();

        let parse_u64 = |val: Option<&serde_json::Value>| -> u64 {
            val.and_then(|x| {
                x.as_u64()
                    .or_else(|| x.as_f64().map(|f| f.max(0.0).round() as u64))
                    .or_else(|| x.as_i64().map(|i| i.max(0) as u64))
            })
            .unwrap_or(0)
        };

        let start_frame = parse_u64(v.get("startFrame").or_else(|| v.get("start_frame")));
        let end_frame = parse_u64(v.get("endFrame").or_else(|| v.get("end_frame")));
        let text = v.get("text").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let confidence = v.get("confidence").and_then(|x| x.as_f64().map(|f| f as f32));
        let track_index = v.get("trackIndex")
            .or_else(|| v.get("track_index"))
            .and_then(|x| {
                x.as_u64().map(|u| u as u32)
                    .or_else(|| x.as_i64().map(|i| i.max(0) as u32))
            });

        let is_unlocked = v.get("isUnlocked").or_else(|| v.get("is_unlocked")).and_then(|x| x.as_bool());
        let position = v.get("position").and_then(|x| serde_json::from_value::<SubtitlePosition>(x.clone()).ok());
        let width = v.get("width").and_then(|x| x.as_f64().map(|f| f as f32));
        let font_size = v.get("fontSize").or_else(|| v.get("font_size")).and_then(|x| x.as_f64().map(|f| f as f32));
        let font_family = v.get("fontFamily").or_else(|| v.get("font_family")).and_then(|x| x.as_str()).map(|s| s.to_string());
        let color = v.get("color").and_then(|x| x.as_str()).map(|s| s.to_string());
        let border_style = v.get("borderStyle").or_else(|| v.get("border_style")).and_then(|x| x.as_str()).map(|s| s.to_string());
        let border_color = v.get("borderColor").or_else(|| v.get("border_color")).and_then(|x| x.as_str()).map(|s| s.to_string());
        let border_width = v.get("borderWidth").or_else(|| v.get("border_width")).and_then(|x| x.as_f64().map(|f| f as f32));
        let bold = v.get("bold").and_then(|x| x.as_bool());
        let italic = v.get("italic").and_then(|x| x.as_bool());
        let underline = v.get("underline").and_then(|x| x.as_bool());
        let strikethrough = v.get("strikethrough").and_then(|x| x.as_bool());
        let align_x = v.get("alignX").or_else(|| v.get("align_x")).and_then(|x| x.as_str()).map(|s| s.to_string());
        let align_y = v.get("alignY").or_else(|| v.get("align_y")).and_then(|x| x.as_str()).map(|s| s.to_string());
        let opacity = v.get("opacity").and_then(|x| x.as_f64().map(|f| f as f32));
        let bg_enabled = v.get("bgEnabled").or_else(|| v.get("bg_enabled")).and_then(|x| x.as_bool());
        let bg_color = v.get("bgColor").or_else(|| v.get("bg_color")).and_then(|x| x.as_str()).map(|s| s.to_string());
        let bg_opacity = v.get("bgOpacity").or_else(|| v.get("bg_opacity")).and_then(|x| x.as_f64().map(|f| f as f32));
        let letter_spacing = v.get("letterSpacing").or_else(|| v.get("letter_spacing")).and_then(|x| x.as_f64().map(|f| f as f32));
        let bg_padding_x = v.get("bgPaddingX").or_else(|| v.get("bg_padding_x")).and_then(|x| x.as_f64().map(|f| f as f32));
        let bg_padding_y = v.get("bgPaddingY").or_else(|| v.get("bg_padding_y")).and_then(|x| x.as_f64().map(|f| f as f32));
        let rotation = v.get("rotation").and_then(|x| x.as_f64().map(|f| f as f32));
        let preset_style_id = v.get("presetStyleId").or_else(|| v.get("preset_style_id")).and_then(|x| x.as_str()).map(|s| s.to_string());
        let keyframes = v.get("keyframes").and_then(|x| serde_json::from_value::<Vec<TextKeyframe>>(x.clone()).ok());
        let animation_in = v.get("animationIn").or_else(|| v.get("animation_in")).and_then(|x| x.as_str()).map(|s| s.to_string());
        let animation_out = v.get("animationOut").or_else(|| v.get("animation_out")).and_then(|x| x.as_str()).map(|s| s.to_string());
        let animation_duration = v.get("animationDuration").or_else(|| v.get("animation_duration")).and_then(|x| x.as_f64().map(|f| f as f32));
        let animated_captions_enabled = v.get("animatedCaptionsEnabled").or_else(|| v.get("animated_captions_enabled")).and_then(|x| x.as_bool());
        let animated_captions_style = v.get("animatedCaptionsStyle").or_else(|| v.get("animated_captions_style")).and_then(|x| x.as_str().map(|s| s.to_string()));
        let words = v.get("words").and_then(|x| {
            if let Some(arr) = x.as_array() {
                let parsed: Vec<SubtitleWord> = arr.iter().filter_map(|item| {
                    let w_text = item.get("text").and_then(|t| t.as_str()).unwrap_or_default().to_string();
                    let w_start = parse_u64(item.get("startFrame").or_else(|| item.get("start_frame")));
                    let w_end = parse_u64(item.get("endFrame").or_else(|| item.get("end_frame")));
                    if !w_text.is_empty() {
                        Some(SubtitleWord {
                            text: w_text,
                            start_frame: w_start,
                            end_frame: w_end,
                        })
                    } else {
                        None
                    }
                }).collect();
                if parsed.is_empty() { None } else { Some(parsed) }
            } else {
                None
            }
        });
        let generated_by_ai = v.get("generatedByAi").or_else(|| v.get("generated_by_ai")).and_then(|x| x.as_bool());
        let single_line_enabled = v.get("singleLineEnabled").or_else(|| v.get("single_line_enabled")).and_then(|x| x.as_bool());
        let words_per_batch = v.get("wordsPerBatch").or_else(|| v.get("words_per_batch")).and_then(|x| x.as_u64().map(|n| n as u32));
        let words_batch_enabled = v.get("wordsBatchEnabled").or_else(|| v.get("words_batch_enabled")).and_then(|x| x.as_bool());

        Ok(SubtitleBlock {
            id,
            start_frame,
            end_frame,
            text,
            confidence,
            track_index,
            is_unlocked,
            position,
            width,
            font_size,
            font_family,
            color,
            border_style,
            border_color,
            border_width,
            bold,
            italic,
            underline,
            strikethrough,
            align_x,
            align_y,
            opacity,
            bg_enabled,
            bg_color,
            bg_opacity,
            letter_spacing,
            bg_padding_x,
            bg_padding_y,
            rotation,
            preset_style_id,
            keyframes,
            animation_in,
            animation_out,
            animation_duration,
            animated_captions_enabled,
            animated_captions_style,
            words,
            generated_by_ai,
            single_line_enabled,
            words_per_batch,
            words_batch_enabled,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitlePosition {
    pub x: f32,
    pub y: f32,
}

impl<'de> Deserialize<'de> for SubtitlePosition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        let parse_f32 = |val: Option<&serde_json::Value>, default: f32| -> f32 {
            val.and_then(|x| x.as_f64().map(|f| f as f32).or_else(|| x.as_i64().map(|i| i as f32)))
                .unwrap_or(default)
        };
        let x = parse_f32(v.get("x"), 50.0);
        let y = parse_f32(v.get("y"), 82.0);
        Ok(SubtitlePosition { x, y })
    }
}

fn default_sub_pos() -> SubtitlePosition {
    SubtitlePosition { x: 50.0, y: 82.0 }
}
fn default_sub_width() -> f32 { 68.0 }
fn default_sub_font_size() -> f32 { 28.0 }
fn default_sub_max_chars() -> u32 { 50 }
fn default_sub_style() -> String { "standard".to_string() }
fn default_sub_language() -> String { "pt-BR".to_string() }
fn default_sub_font_family() -> String { "Inter".to_string() }
fn default_sub_color() -> String { "#facc15".to_string() }
fn default_sub_border_style() -> String { "none".to_string() }
fn default_sub_border_color() -> String { "#000000".to_string() }
fn default_sub_border_width() -> f32 { 2.0 }
fn default_sub_align_x() -> String { "center".to_string() }
fn default_sub_align_y() -> String { "bottom".to_string() }
fn default_sub_opacity() -> f32 { 100.0 }
fn default_sub_bg_color() -> String { "#000000".to_string() }
fn default_sub_bg_opacity() -> f32 { 75.0 }
fn default_sub_letter_spacing() -> f32 { 0.0 }
fn default_sub_bg_padding_x() -> f32 { 8.0 }
fn default_sub_bg_padding_y() -> f32 { 4.0 }
fn default_sub_rotation() -> f32 { 0.0 }

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSubtitles {
    #[serde(default)]
    pub items: Vec<SubtitleBlock>,
    #[serde(default = "default_sub_pos")]
    pub position: SubtitlePosition,
    #[serde(default = "default_sub_width")]
    pub width: f32,
    #[serde(default = "default_sub_font_size")]
    pub font_size: f32,
    #[serde(default = "default_sub_max_chars")]
    pub max_chars_per_block: u32,
    #[serde(default = "default_sub_style")]
    pub style: String,
    #[serde(default = "default_sub_language")]
    pub language: String,

    #[serde(default = "default_sub_font_family")]
    pub font_family: String,
    #[serde(default = "default_sub_color")]
    pub color: String,
    #[serde(default = "default_sub_border_style")]
    pub border_style: String,
    #[serde(default = "default_sub_border_color")]
    pub border_color: String,
    #[serde(default = "default_sub_border_width")]
    pub border_width: f32,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub strikethrough: bool,
    #[serde(default = "default_sub_align_x")]
    pub align_x: String,
    #[serde(default = "default_sub_align_y")]
    pub align_y: String,
    #[serde(default = "default_sub_opacity")]
    pub opacity: f32,
    #[serde(default)]
    pub bg_enabled: bool,
    #[serde(default = "default_sub_bg_color")]
    pub bg_color: String,
    #[serde(default = "default_sub_bg_opacity")]
    pub bg_opacity: f32,
    #[serde(default = "default_sub_letter_spacing")]
    pub letter_spacing: f32,
    #[serde(default = "default_sub_bg_padding_x")]
    pub bg_padding_x: f32,
    #[serde(default = "default_sub_bg_padding_y")]
    pub bg_padding_y: f32,
    #[serde(default = "default_sub_rotation")]
    pub rotation: f32,
    #[serde(default)]
    pub animated_captions_enabled: bool,
    #[serde(default = "default_sub_animated_captions_style")]
    pub animated_captions_style: String,
    #[serde(default)]
    pub single_line_enabled: bool,
    #[serde(default = "default_sub_words_per_batch")]
    pub words_per_batch: u32,
    #[serde(default)]
    pub words_batch_enabled: bool,
}

fn default_sub_words_per_batch() -> u32 { 5 }
fn default_sub_animated_captions_style() -> String { "scale".to_string() }

impl<'de> Deserialize<'de> for ProjectSubtitles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        let items: Vec<SubtitleBlock> = v.get("items")
            .and_then(|i| serde_json::from_value(i.clone()).ok())
            .unwrap_or_default();
        let position: SubtitlePosition = v.get("position")
            .and_then(|p| serde_json::from_value(p.clone()).ok())
            .unwrap_or_else(default_sub_pos);

        let parse_f32 = |field_camel: &str, field_snake: &str, default: f32| -> f32 {
            v.get(field_camel).or_else(|| v.get(field_snake))
                .and_then(|x| x.as_f64().map(|f| f as f32).or_else(|| x.as_i64().map(|i| i as f32)))
                .unwrap_or(default)
        };

        let parse_u32 = |field_camel: &str, field_snake: &str, default: u32| -> u32 {
            v.get(field_camel).or_else(|| v.get(field_snake))
                .and_then(|x| x.as_u64().map(|u| u as u32).or_else(|| x.as_f64().map(|f| f.round().max(0.0) as u32)))
                .unwrap_or(default)
        };

        let parse_bool = |field_camel: &str, field_snake: &str| -> bool {
            v.get(field_camel).or_else(|| v.get(field_snake))
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
        };

        let parse_str = |field_camel: &str, field_snake: &str, default: &str| -> String {
            v.get(field_camel).or_else(|| v.get(field_snake))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| default.to_string())
        };

        let width = parse_f32("width", "width", default_sub_width());
        let font_size = parse_f32("fontSize", "font_size", default_sub_font_size());
        let max_chars_per_block = parse_u32("maxCharsPerBlock", "max_chars_per_block", default_sub_max_chars());
        let style = parse_str("style", "style", &default_sub_style());
        let language = parse_str("language", "language", &default_sub_language());
        let font_family = parse_str("fontFamily", "font_family", &default_sub_font_family());
        let color = parse_str("color", "color", &default_sub_color());
        let border_style = parse_str("borderStyle", "border_style", &default_sub_border_style());
        let border_color = parse_str("borderColor", "border_color", &default_sub_border_color());
        let border_width = parse_f32("borderWidth", "border_width", default_sub_border_width());
        let bold = parse_bool("bold", "bold");
        let italic = parse_bool("italic", "italic");
        let underline = parse_bool("underline", "underline");
        let strikethrough = parse_bool("strikethrough", "strikethrough");
        let align_x = parse_str("alignX", "align_x", &default_sub_align_x());
        let align_y = parse_str("alignY", "align_y", &default_sub_align_y());
        let opacity = parse_f32("opacity", "opacity", default_sub_opacity());
        let bg_enabled = parse_bool("bgEnabled", "bg_enabled");
        let bg_color = parse_str("bgColor", "bg_color", &default_sub_bg_color());
        let bg_opacity = parse_f32("bgOpacity", "bg_opacity", default_sub_bg_opacity());
        let letter_spacing = parse_f32("letterSpacing", "letter_spacing", default_sub_letter_spacing());
        let bg_padding_x = parse_f32("bgPaddingX", "bg_padding_x", default_sub_bg_padding_x());
        let bg_padding_y = parse_f32("bgPaddingY", "bg_padding_y", default_sub_bg_padding_y());
        let rotation = parse_f32("rotation", "rotation", default_sub_rotation());

        Ok(ProjectSubtitles {
            items,
            position,
            width,
            font_size,
            max_chars_per_block,
            style,
            language,
            font_family,
            color,
            border_style,
            border_color,
            border_width,
            bold,
            italic,
            underline,
            strikethrough,
            align_x,
            align_y,
            opacity,
            bg_enabled,
            bg_color,
            bg_opacity,
            letter_spacing,
            bg_padding_x,
            bg_padding_y,
            rotation,
            animated_captions_enabled: v.get("animatedCaptionsEnabled")
                .or_else(|| v.get("animated_captions_enabled"))
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            animated_captions_style: v.get("animatedCaptionsStyle")
                .or_else(|| v.get("animated_captions_style"))
                .and_then(|x| x.as_str().map(|s| s.to_string()))
                .unwrap_or_else(default_sub_animated_captions_style),
            single_line_enabled: parse_bool("singleLineEnabled", "single_line_enabled"),
            words_per_batch: v.get("wordsPerBatch")
                .or_else(|| v.get("words_per_batch"))
                .and_then(|x| x.as_u64().map(|n| n as u32))
                .unwrap_or_else(default_sub_words_per_batch),
            words_batch_enabled: parse_bool("wordsBatchEnabled", "words_batch_enabled"),
        })
    }
}


impl Default for ProjectSubtitles {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            position: default_sub_pos(),
            width: default_sub_width(),
            font_size: default_sub_font_size(),
            max_chars_per_block: default_sub_max_chars(),
            style: default_sub_style(),
            language: default_sub_language(),
            font_family: default_sub_font_family(),
            color: default_sub_color(),
            border_style: default_sub_border_style(),
            border_color: default_sub_border_color(),
            border_width: default_sub_border_width(),
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            align_x: default_sub_align_x(),
            align_y: default_sub_align_y(),
            opacity: default_sub_opacity(),
            bg_enabled: false,
            bg_color: default_sub_bg_color(),
            bg_opacity: default_sub_bg_opacity(),
            letter_spacing: default_sub_letter_spacing(),
            bg_padding_x: default_sub_bg_padding_x(),
            bg_padding_y: default_sub_bg_padding_y(),
            rotation: default_sub_rotation(),
            animated_captions_enabled: false,
            animated_captions_style: default_sub_animated_captions_style(),
            single_line_enabled: false,
            words_per_batch: default_sub_words_per_batch(),
            words_batch_enabled: false,
        }
    }
}
