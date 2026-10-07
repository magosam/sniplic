use std::fs;
use std::path::Path;
use crate::core::paths::clean_path_for_ffmpeg;
use crate::error::AppResult;
use super::CacheManager;

impl CacheManager {
    /// Extracts and normalizes real amplitude peaks from the audio file via FFmpeg PCM f32le
    pub fn get_or_generate_waveform<P: AsRef<Path>>(
        &self,
        media_id: &str,
        file_path: P,
    ) -> AppResult<Vec<f32>> {
        let waveform_filename = format!("{}.json", media_id);
        let waveform_path = clean_path_for_ffmpeg(self.waveform_dir.join(&waveform_filename));

        // 1. Returns from project disk cache if already calculated
        if waveform_path.exists() {
            if let Ok(data) = fs::read_to_string(&waveform_path) {
                if let Ok(peaks) = serde_json::from_str::<Vec<f32>>(&data) {
                    return Ok(peaks);
                }
            }
        }

        // Fallback: if in a project and exists in global cache, migrate to the project
        if self.project_id.is_some() {
            let global_waveform = clean_path_for_ffmpeg(crate::core::paths::get_waveforms_dir().join(&waveform_filename));
            if global_waveform.exists() {
                if let Ok(data) = fs::read_to_string(&global_waveform) {
                    if let Ok(peaks) = serde_json::from_str::<Vec<f32>>(&data) {
                        let _ = fs::write(&waveform_path, &data);
                        return Ok(peaks);
                    }
                }
            }
        }

        let clean_src = clean_path_for_ffmpeg(file_path.as_ref());
        let path_str = clean_src.to_string_lossy();

        // 2. Extracts mono PCM float32 stream at 1000Hz directly through stdout
        let mut cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
        let output = cmd
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-i",
                &path_str,
                "-vn",
                "-ac",
                "1",
                "-ar",
                "1000",
                "-f",
                "f32le",
                "-",
            ])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let bytes = out.stdout;
                let sample_count = bytes.len() / 4;
                if sample_count == 0 {
                    return Ok(Vec::new());
                }

                // Converts binary bytes to float absolute amplitude values
                let mut raw_samples = Vec::with_capacity(sample_count);
                for chunk in bytes.chunks_exact(4) {
                    let val = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]).abs();
                    raw_samples.push(val);
                }

                // Reduces and quantizes to 1200 high-fidelity points
                let target_points = 1200usize;
                let mut peaks = Vec::with_capacity(target_points);

                if sample_count <= target_points {
                    for &s in &raw_samples {
                        peaks.push(s.min(1.0));
                    }
                    while peaks.len() < target_points {
                        peaks.push(0.0);
                    }
                } else {
                    let chunk_size = sample_count as f32 / target_points as f32;
                    for i in 0..target_points {
                        let start = (i as f32 * chunk_size) as usize;
                        let end = (((i + 1) as f32 * chunk_size) as usize).min(sample_count);
                        let mut max_val = 0.0f32;
                        for &val in &raw_samples[start..end] {
                            if val > max_val {
                                max_val = val;
                            }
                        }
                        peaks.push(max_val.min(1.0));
                    }
                }

                // Normalizes with safe maximum peak to aesthetically fill the track
                let max_peak = peaks.iter().cloned().fold(0.0f32, f32::max);
                if max_peak > 0.001 {
                    for p in &mut peaks {
                        *p = (*p / max_peak).min(1.0);
                    }
                }

                // Saves to disk in project cache
                if let Ok(json_data) = serde_json::to_string(&peaks) {
                    let _ = fs::write(&waveform_path, json_data);
                }

                Ok(peaks)
            }
            _ => Ok(Vec::new()),
        }
    }
}
