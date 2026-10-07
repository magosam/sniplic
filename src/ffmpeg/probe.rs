use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use crate::core::project::MediaType;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub media_type: MediaType,
    pub duration_seconds: f64,
    pub duration_frames: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub codec_name: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

#[derive(Deserialize)]
struct FfprobeOutput {
    streams: Option<Vec<FfprobeStream>>,
    format: Option<FfprobeFormat>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    avg_frame_rate: Option<String>,
    duration: Option<String>,
    nb_frames: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u16>,
    #[serde(default)]
    tags: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
    #[serde(default)]
    tags: Option<HashMap<String, String>>,
}

/// Uses ffprobe to extract the real dimensions of a static image.
/// Returns None if ffprobe fails or does not find a video/image stream,
/// in which case the caller should apply a fallback.
fn probe_image_dimensions<P: AsRef<Path>>(file_path: P) -> Option<(u32, u32)> {
    let mut cmd = crate::ffmpeg::binary::get_ffprobe_cmd();

    let output = cmd
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
        ])
        .arg(file_path.as_ref().as_os_str())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout).ok()?;
    let streams = parsed.streams?;

    for stream in streams {
        if let (Some(w), Some(h)) = (stream.width, stream.height) {
            if w > 0 && h > 0 {
                return Some((w, h));
            }
        }
    }

    None
}

fn parse_timecode_to_seconds(tc: &str) -> Option<f64> {
    let parts: Vec<&str> = tc.split(':').collect();
    if parts.len() == 3 {
        let hours: f64 = parts[0].parse().ok()?;
        let mins: f64 = parts[1].parse().ok()?;
        let secs: f64 = parts[2].parse().ok()?;
        return Some(hours * 3600.0 + mins * 60.0 + secs);
    }
    None
}

fn probe_audio_native(path: &Path, ext: &str) -> Option<(f64, Option<u32>, Option<u16>)> {
    match ext {
        "wav" => probe_wav_native(path),
        "mp3" => probe_mp3_native(path),
        _ => None,
    }
}

fn probe_wav_native(path: &Path) -> Option<(f64, Option<u32>, Option<u16>)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let mut riff_header = [0u8; 12];
    file.read_exact(&mut riff_header).ok()?;
    if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WAVE" {
        return None;
    }

    let mut channels = None;
    let mut sample_rate = None;
    let mut byte_rate = None;
    let mut data_size: Option<u64> = None;

    let mut chunk_header = [0u8; 8];
    while file.read_exact(&mut chunk_header).is_ok() {
        let chunk_id = &chunk_header[0..4];
        let chunk_len = u32::from_le_bytes(chunk_header[4..8].try_into().ok()?) as u64;

        if chunk_id == b"fmt " {
            let to_read = chunk_len.min(40) as usize;
            let mut fmt_buf = vec![0u8; to_read];
            file.read_exact(&mut fmt_buf).ok()?;
            if fmt_buf.len() >= 16 {
                let ch = u16::from_le_bytes(fmt_buf[2..4].try_into().ok()?);
                let sr = u32::from_le_bytes(fmt_buf[4..8].try_into().ok()?);
                let br = u32::from_le_bytes(fmt_buf[8..12].try_into().ok()?);
                channels = Some(ch);
                sample_rate = Some(sr);
                byte_rate = Some(br);
            }
            let remaining = chunk_len.saturating_sub(to_read as u64);
            if remaining > 0 {
                file.seek(SeekFrom::Current(remaining as i64)).ok()?;
            }
        } else if chunk_id == b"data" {
            data_size = Some(chunk_len);
            if let (Some(ds), Some(br)) = (data_size, byte_rate) {
                if br > 0 {
                    let dur = ds as f64 / br as f64;
                    return Some((dur, sample_rate, channels));
                }
            }
            file.seek(SeekFrom::Current(chunk_len as i64)).ok()?;
        } else {
            file.seek(SeekFrom::Current(chunk_len as i64)).ok()?;
        }
    }

    if let (Some(ds), Some(br)) = (data_size, byte_rate) {
        if br > 0 {
            return Some((ds as f64 / br as f64, sample_rate, channels));
        }
    }

    None
}

fn probe_mp3_native(path: &Path) -> Option<(f64, Option<u32>, Option<u16>)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    if file_len < 128 {
        return None;
    }

    let mut start_offset = 0u64;
    let mut id3_buf = [0u8; 10];
    if file.read_exact(&mut id3_buf).is_ok() && &id3_buf[0..3] == b"ID3" {
        let s1 = (id3_buf[6] & 0x7F) as u32;
        let s2 = (id3_buf[7] & 0x7F) as u32;
        let s3 = (id3_buf[8] & 0x7F) as u32;
        let s4 = (id3_buf[9] & 0x7F) as u32;
        let tag_size = ((s1 << 21) | (s2 << 14) | (s3 << 7) | s4) as u64;
        start_offset = 10 + tag_size;
    }

    file.seek(SeekFrom::Start(start_offset)).ok()?;

    let mut scan_buf = vec![0u8; 8192];
    let bytes_read = file.read(&mut scan_buf).ok()?;
    if bytes_read < 4 {
        return None;
    }

    for i in 0..bytes_read.saturating_sub(4) {
        if scan_buf[i] == 0xFF && (scan_buf[i + 1] & 0xE0) == 0xE0 {
            let version_bits = (scan_buf[i + 1] >> 3) & 0x03;
            let layer_bits = (scan_buf[i + 1] >> 1) & 0x03;
            let bitrate_index = (scan_buf[i + 2] >> 4) & 0x0F;
            let samplerate_index = (scan_buf[i + 2] >> 2) & 0x03;
            let channel_mode = (scan_buf[i + 3] >> 6) & 0x03;

            if version_bits == 3 && layer_bits == 1 && bitrate_index > 0 && bitrate_index < 15 && samplerate_index < 3 {
                let bitrates = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
                let sample_rates = [44100, 48000, 32000];
                let br_kbps = bitrates[bitrate_index as usize];
                let sr = sample_rates[samplerate_index as usize];
                let channels = if channel_mode == 3 { 1 } else { 2 };

                let xing_offset = if channels == 1 { i + 21 } else { i + 36 };
                if xing_offset + 12 <= bytes_read {
                    let tag = &scan_buf[xing_offset..xing_offset + 4];
                    if tag == b"Xing" || tag == b"Info" {
                        let flags = u32::from_be_bytes(scan_buf[xing_offset + 4..xing_offset + 8].try_into().unwrap_or([0; 4]));
                        if (flags & 0x01) != 0 && xing_offset + 12 <= bytes_read {
                            let total_frames = u32::from_be_bytes(scan_buf[xing_offset + 8..xing_offset + 12].try_into().unwrap_or([0; 4]));
                            if total_frames > 0 {
                                let dur = (total_frames as f64 * 1152.0) / sr as f64;
                                return Some((dur, Some(sr), Some(channels)));
                            }
                        }
                    }
                }

                let audio_bytes = file_len.saturating_sub(start_offset);
                if br_kbps > 0 && audio_bytes > 0 {
                    let dur = (audio_bytes as f64 * 8.0) / (br_kbps as f64 * 1000.0);
                    return Some((dur, Some(sr), Some(channels)));
                }
            }
        }
    }

    None
}

pub fn probe_file<P: AsRef<Path>>(file_path: P, target_fps: f64) -> AppResult<ProbeResult> {
    let path_ref = file_path.as_ref();
    let ext = path_ref
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. Static images (default 5-second duration in project timebase)
    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp") {
        let (real_width, real_height) = probe_image_dimensions(path_ref).unwrap_or((1920, 1080));
        return Ok(ProbeResult {
            media_type: MediaType::Image,
            duration_seconds: 5.0,
            duration_frames: (5.0 * target_fps).round() as u64,
            width: Some(real_width),
            height: Some(real_height),
            fps: None,
            codec_name: ext,
            sample_rate: None,
            channels: None,
        });
    }

    let mut cmd = crate::ffmpeg::binary::get_ffprobe_cmd();

    let output = cmd
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path_ref.as_os_str())
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            if let Ok(parsed) = serde_json::from_slice::<FfprobeOutput>(&out.stdout) {
                let mut media_type = MediaType::Video;
                let mut width = None;
                let mut height = None;
                let mut native_fps = None;
                let mut codec_name = "unknown".to_string();
                let mut sample_rate = None;
                let mut channels = None;
                let mut extracted_duration: Option<f64> = None;

                if let Some(streams) = parsed.streams {
                    for stream in streams {
                        if stream.codec_type.as_deref() == Some("video") && width.is_none() {
                            media_type = MediaType::Video;
                            width = stream.width;
                            height = stream.height;
                            if let Some(c) = stream.codec_name {
                                codec_name = c;
                            }
                            let rate_str = stream.r_frame_rate.as_ref().or(stream.avg_frame_rate.as_ref());
                            if let Some(r_fps) = rate_str {
                                if let Some((num, den)) = r_fps.split_once('/') {
                                    if let (Ok(n), Ok(d)) = (num.parse::<f64>(), den.parse::<f64>()) {
                                        if d > 0.0 && (n / d) > 1.0 {
                                            native_fps = Some(n / d);
                                        }
                                    }
                                }
                            }
                            if let Some(d_str) = stream.duration {
                                if let Ok(d) = d_str.parse::<f64>() {
                                    if d > 0.0 {
                                        extracted_duration = Some(d);
                                    }
                                }
                            }
                            if extracted_duration.is_none() {
                                if let Some(tags) = stream.tags {
                                    for (k, v) in tags {
                                        if k.to_uppercase().contains("DURATION") {
                                            if let Some(secs) = parse_timecode_to_seconds(&v) {
                                                extracted_duration = Some(secs);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            if extracted_duration.is_none() {
                                if let (Some(nb), Some(curr_fps)) = (stream.nb_frames, native_fps) {
                                    if let Ok(frames) = nb.parse::<f64>() {
                                        if frames > 0.0 && curr_fps > 0.0 {
                                            extracted_duration = Some(frames / curr_fps);
                                        }
                                    }
                                }
                            }
                        } else if stream.codec_type.as_deref() == Some("audio") && channels.is_none() {
                            if width.is_none() {
                                media_type = MediaType::Audio;
                                codec_name = stream.codec_name.unwrap_or_else(|| "audio".to_string());
                            }
                            channels = stream.channels;
                            sample_rate = stream.sample_rate.and_then(|s| s.parse::<u32>().ok());
                            if extracted_duration.is_none() {
                                if let Some(d_str) = stream.duration {
                                    if let Ok(d) = d_str.parse::<f64>() {
                                        if d > 0.0 {
                                            extracted_duration = Some(d);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let format_dur = parsed
                    .format
                    .as_ref()
                    .and_then(|f| f.duration.as_ref())
                    .and_then(|d| d.parse::<f64>().ok())
                    .filter(|d| *d > 0.0);

                let tag_dur = parsed.format.as_ref().and_then(|f| f.tags.as_ref()).and_then(|tags| {
                    for (k, v) in tags {
                        if k.to_uppercase().contains("DURATION") {
                            if let Some(secs) = parse_timecode_to_seconds(v) {
                                return Some(secs);
                            }
                        }
                    }
                    None
                });

                let is_audio = media_type == MediaType::Audio
                    || matches!(
                        ext.as_str(),
                        "mp3" | "wav" | "aac" | "flac" | "ogg" | "m4a" | "wma" | "opus" | "aiff"
                    );

                let native_audio = if is_audio {
                    probe_audio_native(path_ref, &ext)
                } else {
                    None
                };

                if let Some((_, n_sr, n_ch)) = native_audio {
                    if sample_rate.is_none() {
                        sample_rate = n_sr;
                    }
                    if channels.is_none() {
                        channels = n_ch;
                    }
                }

                let dur_secs = format_dur
                    .or(extracted_duration)
                    .or(tag_dur)
                    .or_else(|| native_audio.map(|(d, _, _)| d))
                    .unwrap_or(10.0);

                // duration_frames in project timebase
                let timeline_frames = (dur_secs * target_fps).round() as u64;

                return Ok(ProbeResult {
                    media_type: if is_audio { MediaType::Audio } else { media_type },
                    duration_seconds: dur_secs,
                    duration_frames: timeline_frames,
                    width,
                    height,
                    fps: native_fps,
                    codec_name,
                    sample_rate,
                    channels,
                });
            }
        }
    }

    let is_audio = matches!(
        ext.as_str(),
        "mp3" | "wav" | "aac" | "flac" | "ogg" | "m4a" | "wma" | "opus" | "aiff"
    );

    // Fallback 1: Native audio extraction from file bytes (independent of ffmpeg/ffprobe)
    if is_audio {
        if let Some((secs, native_sr, native_ch)) = probe_audio_native(path_ref, &ext) {
            if secs > 0.0 {
                return Ok(ProbeResult {
                    media_type: MediaType::Audio,
                    duration_seconds: secs,
                    duration_frames: (secs * target_fps).round() as u64,
                    width: None,
                    height: None,
                    fps: None,
                    codec_name: ext,
                    sample_rate: native_sr.or(Some(48000)),
                    channels: native_ch.or(Some(2)),
                });
            }
        }
    }

    // Fallback 2: Via ffmpeg stderr
    let mut ffmpeg_cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
    if let Ok(out) = ffmpeg_cmd.arg("-i").arg(path_ref.as_os_str()).output() {
        let stderr_text = String::from_utf8_lossy(&out.stderr);
        if let Some(dur_idx) = stderr_text.find("Duration: ") {
            let after = &stderr_text[dur_idx + 10..];
            if let Some(comma_idx) = after.find(',') {
                if let Some(secs) = parse_timecode_to_seconds(after[..comma_idx].trim()) {
                    return Ok(ProbeResult {
                        media_type: if is_audio { MediaType::Audio } else { MediaType::Video },
                        duration_seconds: secs,
                        duration_frames: (secs * target_fps).round() as u64,
                        width: if is_audio { None } else { Some(1920) },
                        height: if is_audio { None } else { Some(1080) },
                        fps: Some(target_fps),
                        codec_name: ext,
                        sample_rate: Some(48000),
                        channels: Some(2),
                    });
                }
            }
        }
    }

    Ok(ProbeResult {
        media_type: if is_audio { MediaType::Audio } else { MediaType::Video },
        duration_seconds: 10.0,
        duration_frames: (10.0 * target_fps).round() as u64,
        width: if is_audio { None } else { Some(1920) },
        height: if is_audio { None } else { Some(1080) },
        fps: Some(target_fps),
        codec_name: ext,
        sample_rate: Some(48000),
        channels: Some(2),
    })
}
