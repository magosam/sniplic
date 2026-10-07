use crate::core::project::{AudioSettings, Clip, MediaType, Project, TrackType, Transform};
use crate::error::{AppError, AppResult};
use super::gapless;
use super::movement::truncate::truncate_overlapping_clips;
use super::ripple::push_right_minimal;
use super::tracks::TrackOperations;
use uuid::Uuid;

pub struct InsertOperations;

impl InsertOperations {
    pub fn add_clip(
        project: &mut Project,
        track_id: &str,
        media_id: &str,
        start_frame: u64,
        push: bool,
        image_duration_frames: Option<u64>,
    ) -> AppResult<Clip> {
        let is_main = gapless::track_allows_gapless(project, track_id);
        let media = project.media_pool.get(media_id)
            .ok_or_else(|| AppError::InvalidOperation("Media not found".to_string()))?;

        let track = project.tracks.iter_mut().find(|t| t.id == track_id)
            .ok_or_else(|| AppError::InvalidOperation("Track not found".to_string()))?;

        if (track.track_type == TrackType::Video && media.media_type == MediaType::Audio)
            || (track.track_type == TrackType::Audio && media.media_type != MediaType::Audio) {
            return Err(AppError::InvalidOperation("Media type incompatible with track".to_string()));
        }

        if track.locked {
            return Err(AppError::InvalidOperation(format!("Track '{}' is locked", track.name)));
        }

        let dur = if media.media_type == MediaType::Image {
            image_duration_frames
                .filter(|&f| f > 0)
                .unwrap_or_else(|| {
                    let fps = project.config.fps.max(1.0);
                    let sec = if project.config.default_image_duration > 0.0 {
                        project.config.default_image_duration
                    } else {
                        5.0
                    };
                    (sec * fps).round() as u64
                })
        } else {
            media.duration_frames
        };
        if push && is_main {
            // Clip(s) that started before the insertion point but extended
            // into it: the insertion "cuts" and pushes that piece to after
            // the new clip only on the MAIN track.
            for c in track.clips.iter_mut() {
                if c.start_frame < start_frame && c.start_frame + c.duration_frames > start_frame {
                    c.start_frame = start_frame + dur;
                }
            }
            // Push the rest (start_frame >= start_frame) the MINIMUM
            // required not to overlap the new clip. Extra tracks are free and never yield space.
            push_right_minimal(&mut track.clips, start_frame, start_frame + dur);
        } else {
            // "a media item is never added on top of another while the underlying one remains the same size.
            // the obscured part is always lost, as if deleted."
            truncate_overlapping_clips(track, start_frame, dur, "");
        }

        let new_clip = Clip {
            id: format!("clp_{}", Uuid::new_v4().simple()),
            media_id: media_id.to_string(),
            name: media.name.clone(),
            start_frame,
            duration_frames: dur,
            in_point_frames: 0,
            out_point_frames: dur,
            transform: Transform::default(),
            audio: AudioSettings::default(),
            plugins: std::collections::HashMap::new(),
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
        };

        track.clips.push(new_clip.clone());
        track.clips.sort_by_key(|c| c.start_frame);
        TrackOperations::cleanup_empty_tracks(project);
        Ok(new_clip)
    }

    pub fn add_clips_batch(
        project: &mut Project,
        media_ids: &[String],
        start: u64,
        target_track: Option<String>,
        push: bool,
        image_duration_frames: Option<u64>,
    ) -> AppResult<Vec<Clip>> {
        let mut inserted = Vec::new();
        let mut v_offset = start;
        let mut a_offset = start;

        for id in media_ids {
            let m_type = project.media_pool.get(id).map(|m| m.media_type.clone());
            if let Some(mt) = m_type {
                let is_audio = mt == MediaType::Audio;
                let track_id = match target_track {
                    Some(ref tid) if project.tracks.iter().any(|t| t.id == *tid && ((is_audio && t.track_type == TrackType::Audio) || (!is_audio && t.track_type == TrackType::Video))) => tid.clone(),
                    _ => {
                        let t_type = if is_audio { TrackType::Audio } else { TrackType::Video };
                        if let Some(t) = project.tracks.iter().find(|t| t.track_type == t_type) {
                            t.id.clone()
                        } else {
                            // There is no track of this type in the project yet --
                            // no possible "reference track" here, falls back to
                            // the absolute fallback of add_track (see docstring).
                            TrackOperations::add_track(project, t_type, None)?.id
                        }
                    }
                };

                let cur_start = if is_audio {
                    a_offset.max(v_offset)
                } else {
                    v_offset
                };
                let clip = Self::add_clip(project, &track_id, id, cur_start, push, image_duration_frames)?;
                if is_audio {
                    a_offset = cur_start + clip.duration_frames;
                } else {
                    v_offset = cur_start + clip.duration_frames;
                }
                inserted.push(clip);
            }
        }
        Ok(inserted)
    }
}