use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_scale() -> f32 {
    1.0
}

fn default_opacity() -> f32 {
    1.0
}

fn default_volume() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transform {
    #[serde(default)]
    pub position_x: f32,
    #[serde(default)]
    pub position_y: f32,
    #[serde(default = "default_scale")]
    pub scale_x: f32,
    #[serde(default = "default_scale")]
    pub scale_y: f32,
    #[serde(default)]
    pub rotation_degrees: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_degrees: 0.0,
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSettings {
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(rename = "muted", alias = "mute", default)]
    pub mute: bool,
    #[serde(default)]
    pub solo: bool,
    #[serde(default)]
    pub fade_in_frames: u32,
    #[serde(default)]
    pub fade_out_frames: u32,
    #[serde(default)]
    pub eq_bass: f32,
    #[serde(default)]
    pub eq_mid: f32,
    #[serde(default)]
    pub eq_treble: f32,
    #[serde(default)]
    pub denoise: f32,
    #[serde(default, rename = "processedAudioPath", alias = "processed_audio_path", skip_serializing_if = "Option::is_none")]
    pub processed_audio_path: Option<String>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            mute: false,
            solo: false,
            fade_in_frames: 0,
            fade_out_frames: 0,
            processed_audio_path: None,
            eq_bass: 0.0,
            eq_mid: 0.0,
            eq_treble: 0.0,
            denoise: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Clip {
    pub id: String,
    pub media_id: String,
    pub name: String,
    pub start_frame: u64,
    pub duration_frames: u64,
    pub in_point_frames: u64,
    pub out_point_frames: u64,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default)]
    pub audio: AudioSettings,
    #[serde(default, rename = "plugins", skip_serializing_if = "HashMap::is_empty")]
    pub plugins: HashMap<String, serde_json::Value>,
    #[serde(default, rename = "transformPluginData", alias = "transform_plugin_data", skip_serializing_if = "Option::is_none")]
    pub transform_plugin_data: Option<serde_json::Value>,
    #[serde(default, rename = "filterPluginData", alias = "filter_plugin_data", skip_serializing_if = "Option::is_none")]
    pub filter_plugin_data: Option<serde_json::Value>,
    #[serde(default, rename = "effectPluginData", alias = "effect_plugin_data", skip_serializing_if = "Option::is_none")]
    pub effect_plugin_data: Option<serde_json::Value>,
    #[serde(default, rename = "adjustmentsPluginData", alias = "adjustments_plugin_data", skip_serializing_if = "Option::is_none")]
    pub adjustments_plugin_data: Option<serde_json::Value>,
    #[serde(default, rename = "enhancementPluginData", alias = "enhancement_plugin_data", skip_serializing_if = "Option::is_none")]
    pub enhancement_plugin_data: Option<serde_json::Value>,
    #[serde(default, rename = "canvasPluginData", alias = "canvas_plugin_data", skip_serializing_if = "Option::is_none")]
    pub canvas_plugin_data: Option<serde_json::Value>,
    #[serde(default, rename = "transitionPluginData", alias = "transition_plugin_data", skip_serializing_if = "Option::is_none")]
    pub transition_plugin_data: Option<serde_json::Value>,
    #[serde(default, rename = "chromaKeyPluginData", alias = "chroma_key_plugin_data", skip_serializing_if = "Option::is_none")]
    pub chroma_key_plugin_data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reversed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(default, rename = "fadeInFrames", alias = "fade_in_frames", skip_serializing_if = "Option::is_none")]
    pub fade_in_frames: Option<u32>,
    #[serde(default, rename = "fadeOutFrames", alias = "fade_out_frames", skip_serializing_if = "Option::is_none")]
    pub fade_out_frames: Option<u32>,
    #[serde(default, rename = "audioExtracted", alias = "audio_extracted", skip_serializing_if = "Option::is_none")]
    pub audio_extracted: Option<bool>,
    #[serde(default, rename = "isCompound", alias = "is_compound", skip_serializing_if = "Option::is_none")]
    pub is_compound: Option<bool>,
    #[serde(default, rename = "compoundClips", alias = "compound_clips", skip_serializing_if = "Option::is_none")]
    pub compound_clips: Option<Vec<Clip>>,
    #[serde(default, rename = "clipPath", alias = "clip_path", skip_serializing_if = "Option::is_none")]
    pub clip_path: Option<String>,
}

impl<'de> Deserialize<'de> for Clip {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let media_id = v.get("media_id").or_else(|| v.get("mediaId")).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or_default().to_string();

        let parse_u64 = |val: Option<&serde_json::Value>| -> u64 {
            val.and_then(|x| {
                x.as_u64()
                    .or_else(|| x.as_f64().map(|f| f.max(0.0).round() as u64))
                    .or_else(|| x.as_i64().map(|i| i.max(0) as u64))
            })
            .unwrap_or(0)
        };

        let start_frame = parse_u64(v.get("start_frame").or_else(|| v.get("startFrame")));
        let duration_frames = parse_u64(v.get("duration_frames").or_else(|| v.get("durationFrames")));
        let in_point_frames = parse_u64(v.get("in_point_frames").or_else(|| v.get("inPointFrames")));
        let out_point_frames = parse_u64(v.get("out_point_frames").or_else(|| v.get("outPointFrames")));

        let transform = v
            .get("transform")
            .and_then(|t| serde_json::from_value(t.clone()).ok())
            .unwrap_or_default();

        let audio = v
            .get("audio")
            .and_then(|a| serde_json::from_value(a.clone()).ok())
            .unwrap_or_default();

        let transform_plugin_data = v
            .get("transformPluginData")
            .filter(|x| !x.is_null())
            .or_else(|| v.get("transform_plugin_data").filter(|x| !x.is_null()))
            .cloned();

        let filter_plugin_data = v
            .get("filterPluginData")
            .filter(|x| !x.is_null())
            .or_else(|| v.get("filter_plugin_data").filter(|x| !x.is_null()))
            .cloned();

        let effect_plugin_data = v
            .get("effectPluginData")
            .filter(|x| !x.is_null())
            .or_else(|| v.get("effect_plugin_data").filter(|x| !x.is_null()))
            .cloned();

        let adjustments_plugin_data = v
            .get("adjustmentsPluginData")
            .filter(|x| !x.is_null())
            .or_else(|| v.get("adjustments_plugin_data").filter(|x| !x.is_null()))
            .cloned();

        let enhancement_plugin_data = v
            .get("enhancementPluginData")
            .filter(|x| !x.is_null())
            .or_else(|| v.get("enhancement_plugin_data").filter(|x| !x.is_null()))
            .cloned();

        let canvas_plugin_data = v
            .get("canvasPluginData")
            .filter(|x| !x.is_null())
            .or_else(|| v.get("canvas_plugin_data").filter(|x| !x.is_null()))
            .cloned();

        let transition_plugin_data = v
            .get("transitionPluginData")
            .filter(|x| !x.is_null())
            .or_else(|| v.get("transition_plugin_data").filter(|x| !x.is_null()))
            .cloned();

        let chroma_key_plugin_data = v
            .get("chromaKeyPluginData")
            .filter(|x| !x.is_null())
            .or_else(|| v.get("chroma_key_plugin_data").filter(|x| !x.is_null()))
            .cloned();

        let mut plugins: HashMap<String, serde_json::Value> = v
            .get("plugins")
            .or_else(|| v.get("pluginData"))
            .and_then(|p| serde_json::from_value(p.clone()).ok())
            .unwrap_or_default();

        if let Some(ref d) = transform_plugin_data {
            plugins.entry("transform".to_string()).or_insert_with(|| d.clone());
        }
        if let Some(ref d) = filter_plugin_data {
            plugins.entry("filter".to_string()).or_insert_with(|| d.clone());
        }
        if let Some(ref d) = effect_plugin_data {
            plugins.entry("effect".to_string()).or_insert_with(|| d.clone());
        }
        if let Some(ref d) = adjustments_plugin_data {
            plugins.entry("adjustments".to_string()).or_insert_with(|| d.clone());
        }
        if let Some(ref d) = enhancement_plugin_data {
            plugins.entry("enhancement".to_string()).or_insert_with(|| d.clone());
        }
        if let Some(ref d) = canvas_plugin_data {
            plugins.entry("canvas".to_string()).or_insert_with(|| d.clone());
        }
        if let Some(ref d) = transition_plugin_data {
            plugins.entry("transition".to_string()).or_insert_with(|| d.clone());
        }
        if let Some(ref d) = chroma_key_plugin_data {
            plugins.entry("chroma".to_string()).or_insert_with(|| d.clone());
        }

        let reversed = v.get("reversed").and_then(|x| x.as_bool());
        let speed = v
            .get("speed")
            .and_then(|x| x.as_f64().or_else(|| x.as_u64().map(|n| n as f64)));
        let fade_in_frames = v
            .get("fadeInFrames")
            .or_else(|| v.get("fade_in_frames"))
            .and_then(|x| x.as_u64().map(|n| n as u32));
        let fade_out_frames = v
            .get("fadeOutFrames")
            .or_else(|| v.get("fade_out_frames"))
            .and_then(|x| x.as_u64().map(|n| n as u32));
        let audio_extracted = v
            .get("audioExtracted")
            .or_else(|| v.get("audio_extracted"))
            .and_then(|x| x.as_bool());
        let is_compound = v
            .get("isCompound")
            .or_else(|| v.get("is_compound"))
            .and_then(|x| x.as_bool());
        let compound_clips = v
            .get("compoundClips")
            .or_else(|| v.get("compound_clips"))
            .and_then(|x| serde_json::from_value(x.clone()).ok());
        let clip_path = v
            .get("clipPath")
            .or_else(|| v.get("clip_path"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());

        Ok(Clip {
            id,
            media_id,
            name,
            start_frame,
            duration_frames,
            in_point_frames,
            out_point_frames,
            transform,
            audio,
            plugins,
            transform_plugin_data,
            filter_plugin_data,
            effect_plugin_data,
            adjustments_plugin_data,
            enhancement_plugin_data,
            canvas_plugin_data,
            transition_plugin_data,
            chroma_key_plugin_data,
            reversed,
            speed,
            fade_in_frames,
            fade_out_frames,
            audio_extracted,
            is_compound,
            compound_clips,
            clip_path,
        })
    }
}

impl Clip {
    pub fn get_plugin(&self, key: &str) -> Option<&serde_json::Value> {
        self.plugins.get(key).or_else(|| match key {
            "transform" => self.transform_plugin_data.as_ref(),
            "chroma" => self.chroma_key_plugin_data.as_ref(),
            "filter" => self.filter_plugin_data.as_ref(),
            "effect" => self.effect_plugin_data.as_ref(),
            "adjustments" => self.adjustments_plugin_data.as_ref(),
            "enhancement" => self.enhancement_plugin_data.as_ref(),
            "canvas" => self.canvas_plugin_data.as_ref(),
            "transition" => self.transition_plugin_data.as_ref(),
            _ => None,
        })
    }

    pub fn set_plugin(&mut self, key: String, data: serde_json::Value) {
        match key.as_str() {
            "transform" => self.transform_plugin_data = Some(data.clone()),
            "filter" => self.filter_plugin_data = Some(data.clone()),
            "effect" => self.effect_plugin_data = Some(data.clone()),
            "adjustments" => self.adjustments_plugin_data = Some(data.clone()),
            "enhancement" => self.enhancement_plugin_data = Some(data.clone()),
            "canvas" => self.canvas_plugin_data = Some(data.clone()),
            "transition" => self.transition_plugin_data = Some(data.clone()),
            _ => {}
        }
        self.plugins.insert(key, data);
    }

    pub fn has_plugin(&self, key: &str) -> bool {
        self.plugins.contains_key(key) || match key {
            "transform" => self.transform_plugin_data.is_some(),
            "filter" => self.filter_plugin_data.is_some(),
            "effect" => self.effect_plugin_data.is_some(),
            "adjustments" => self.adjustments_plugin_data.is_some(),
            "enhancement" => self.enhancement_plugin_data.is_some(),
            "canvas" => self.canvas_plugin_data.is_some(),
            "transition" => self.transition_plugin_data.is_some(),
            _ => false,
        }
    }

    /// Identifies whether the clip represents a visual effect (FX)
    pub fn is_effect_clip(&self) -> bool {
        self.media_id.starts_with("fx_effect_")
            || self.effect_plugin_data.is_some()
            || self
                .transform_plugin_data
                .as_ref()
                .and_then(|t| t.get("core.video.effects"))
                .is_some()
            || self.plugins.contains_key("effect")
            || self.plugins.contains_key("core.video.effects")
    }

    /// Identifies whether the clip represents a color style filter (FX)
    pub fn is_filter_clip(&self) -> bool {
        if self.is_effect_clip() {
            return false;
        }
        self.media_id.starts_with("fx_")
            || self.filter_plugin_data.is_some()
            || self
                .transform_plugin_data
                .as_ref()
                .and_then(|t| t.get("core.video.filters"))
                .is_some()
            || self.plugins.contains_key("filter")
            || self.plugins.contains_key("core.video.filters")
    }
}

impl Default for Clip {
    fn default() -> Self {
        Self {
            id: String::new(),
            media_id: String::new(),
            name: String::new(),
            start_frame: 0,
            duration_frames: 0,
            in_point_frames: 0,
            out_point_frames: 0,
            transform: Transform::default(),
            audio: AudioSettings::default(),
            plugins: HashMap::new(),
            transform_plugin_data: None,
            filter_plugin_data: None,
            effect_plugin_data: None,
            adjustments_plugin_data: None,
            enhancement_plugin_data: None,
            canvas_plugin_data: None,
            transition_plugin_data: None,
            chroma_key_plugin_data: None,
            reversed: None,
            speed: None,
            fade_in_frames: None,
            fade_out_frames: None,
            audio_extracted: None,
            is_compound: None,
            compound_clips: None,
            clip_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrackType {
    Video,
    Audio,
    Filter,
}

fn deserialize_option_u32_flexible<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    match opt {
        Some(serde_json::Value::Number(n)) => {
            if let Some(u) = n.as_u64() {
                Ok(Some(u as u32))
            } else if let Some(f) = n.as_f64() {
                Ok(Some(f.max(0.0).round() as u32))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub track_type: TrackType,
    pub locked: bool,
    /// Mutes track audio (affects video and audio tracks).
    pub muted: bool,
    /// Hides the track's visual content in the composition.
    #[serde(default)]
    pub hidden: bool,
    pub solo: bool,
    pub clips: Vec<Clip>,
    /// Custom track height in pixels. If None, uses default height.
    #[serde(
        default,
        deserialize_with = "deserialize_option_u32_flexible",
        skip_serializing_if = "Option::is_none"
    )]
    pub height: Option<u32>,
}

impl Track {
    pub fn new(id: String, name: String, track_type: TrackType) -> Self {
        Self {
            id,
            name,
            track_type,
            locked: false,
            muted: false,
            hidden: false,
            solo: false,
            clips: Vec::new(),
            height: None,
        }
    }

    pub fn is_filter(&self) -> bool {
        self.track_type == TrackType::Filter
    }
}
