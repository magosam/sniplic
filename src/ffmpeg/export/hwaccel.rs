use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareEncoderType {
    Nvenc,
    Qsv,
    Amf,
    MediaFoundation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitrateTargets {
    pub base_kbps: u32,
    pub maxrate_kbps: u32,
    pub bufsize_kbps: u32,
}

/// Calculates dynamic bitrate limits based on pixels per second count.
/// Ensures high-motion scenes receive sufficient bandwidth (high peaks)
/// without inflating static scenes.
pub fn calculate_bitrate_targets(width: u32, height: u32, fps: f64) -> BitrateTargets {
    let effective_fps = if fps > 0.0 { fps } else { 30.0 };
    let pixels_per_sec = (width as f64) * (height as f64) * effective_fps;

    // Reference: 1080p 30fps = 1920 * 1080 * 30 ≈ 62,208,000 pixels/second
    let ref_pps = 1920.0 * 1080.0 * 30.0;
    let ratio = (pixels_per_sec / ref_pps).max(0.25).min(8.0);

    // For 1080p 30fps: base 16 Mbps, max peak 45 Mbps, bufsize 90 Mbps
    let base_kbps = ((16_000.0 * ratio).round() as u32).max(4_000).min(90_000);
    let maxrate_kbps = ((45_000.0 * ratio).round() as u32).max(12_000).min(150_000);
    let bufsize_kbps = maxrate_kbps * 2;

    BitrateTargets {
        base_kbps,
        maxrate_kbps,
        bufsize_kbps,
    }
}

/// Tests whether a given video encoder can successfully initialize on the current GPU/driver.
fn test_encoder_available(encoder_name: &str) -> bool {
    let mut cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();

    let status = cmd
        .args([
            "-v", "quiet",
            "-f", "lavfi",
            "-i", "color=c=black:s=64x64",
            "-frames:v", "1",
            "-c:v", encoder_name,
            "-f", "null",
            "-",
        ])
        .status();

    match status {
        Ok(s) => s.success(),
        Err(_) => false,
    }
}

static CACHED_H264_HW: OnceLock<Option<HardwareEncoderType>> = OnceLock::new();
static CACHED_HEVC_HW: OnceLock<Option<HardwareEncoderType>> = OnceLock::new();

/// Detects the best available hardware encoder option for H.264
pub fn detect_h264_hardware_encoder() -> Option<HardwareEncoderType> {
    *CACHED_H264_HW.get_or_init(|| {
        if test_encoder_available("h264_nvenc") {
            tracing::info!("Hardware encoder detected: NVIDIA NVENC (h264_nvenc)");
            return Some(HardwareEncoderType::Nvenc);
        }
        if test_encoder_available("h264_qsv") {
            tracing::info!("Hardware encoder detected: Intel QuickSync (h264_qsv)");
            return Some(HardwareEncoderType::Qsv);
        }
        if test_encoder_available("h264_amf") {
            tracing::info!("Hardware encoder detected: AMD AMF (h264_amf)");
            return Some(HardwareEncoderType::Amf);
        }
        if test_encoder_available("h264_mf") {
            tracing::info!("Hardware encoder detected: Windows Media Foundation (h264_mf)");
            return Some(HardwareEncoderType::MediaFoundation);
        }
        tracing::info!("No H.264 hardware encoder available. Using optimized CPU (libx264).");
        None
    })
}

/// Detects the best available hardware encoder option for HEVC (H.265)
pub fn detect_hevc_hardware_encoder() -> Option<HardwareEncoderType> {
    *CACHED_HEVC_HW.get_or_init(|| {
        if test_encoder_available("hevc_nvenc") {
            tracing::info!("Hardware encoder detected: NVIDIA NVENC (hevc_nvenc)");
            return Some(HardwareEncoderType::Nvenc);
        }
        if test_encoder_available("hevc_qsv") {
            tracing::info!("Hardware encoder detected: Intel QuickSync (hevc_qsv)");
            return Some(HardwareEncoderType::Qsv);
        }
        if test_encoder_available("hevc_amf") {
            tracing::info!("Hardware encoder detected: AMD AMF (hevc_amf)");
            return Some(HardwareEncoderType::Amf);
        }
        tracing::info!("No HEVC hardware encoder available. Using optimized CPU (libx265).");
        None
    })
}

pub fn is_hardware_encoder_used(codec: &str) -> bool {
    match codec.to_lowercase().as_str() {
        "hevc" => detect_hevc_hardware_encoder().is_some(),
        "vp9" | "av1" => false,
        _ => detect_h264_hardware_encoder().is_some(),
    }
}

/// Builds encoding arguments exclusively for CPU (software).
/// Used as automatic safety fallback if any GPU encoder fails.
pub fn build_software_encoder_args(
    codec: &str,
    width: u32,
    height: u32,
    fps: f64,
) -> Vec<String> {
    let targets = calculate_bitrate_targets(width, height, fps);
    let mut args = Vec::new();

    match codec.to_lowercase().as_str() {
        "hevc" => {
            args.extend([
                "-c:v".into(), "libx265".into(),
                "-preset".into(), "medium".into(),
                "-crf".into(), "21".into(),
                "-maxrate".into(), format!("{}k", targets.maxrate_kbps),
                "-bufsize".into(), format!("{}k", targets.bufsize_kbps),
                "-x265-params".into(), "no-sao=1:aq-mode=2:rd=3".into(),
                "-pix_fmt".into(), "yuv420p".into(),
            ]);
        }
        "vp9" => {
            args.extend([
                "-c:v".into(), "libvpx-vp9".into(),
                "-b:v".into(), "0".into(),
                "-crf".into(), "24".into(),
                "-pix_fmt".into(), "yuv420p".into(),
            ]);
        }
        "av1" => {
            args.extend([
                "-c:v".into(), "libsvtav1".into(),
                "-preset".into(), "6".into(),
                "-crf".into(), "24".into(),
                "-pix_fmt".into(), "yuv420p".into(),
            ]);
        }
        _ => {
            args.extend([
                "-c:v".into(), "libx264".into(),
                "-preset".into(), "medium".into(),
                "-crf".into(), "19".into(),
                "-maxrate".into(), format!("{}k", targets.maxrate_kbps),
                "-bufsize".into(), format!("{}k", targets.bufsize_kbps),
                "-x264-params".into(), "me=hex:subme=7:me_range=24:rc-lookahead=20:b-adapt=1:bframes=2:aq-mode=2:ref=2".into(),
                "-pix_fmt".into(), "yuv420p".into(),
            ]);
        }
    }

    args
}

/// Builds the complete list of video encoding arguments for FFmpeg,
/// applying dynamic VBR, high-quality presets, and safe hardware acceleration.
pub fn build_video_encoder_args(
    codec: &str,
    width: u32,
    height: u32,
    fps: f64,
) -> Vec<String> {
    let targets = calculate_bitrate_targets(width, height, fps);
    let mut args = Vec::new();

    match codec.to_lowercase().as_str() {
        "hevc" => {
            let hw = detect_hevc_hardware_encoder();
            match hw {
                Some(HardwareEncoderType::Nvenc) => {
                    args.extend([
                        "-c:v".into(), "hevc_nvenc".into(),
                        "-preset".into(), "p5".into(),
                        "-tune".into(), "hq".into(),
                        "-rc".into(), "vbr".into(),
                        "-cq".into(), "21".into(),
                        "-b:v".into(), "0".into(),
                        "-maxrate".into(), format!("{}k", targets.maxrate_kbps),
                        "-bufsize".into(), format!("{}k", targets.bufsize_kbps),
                        "-surfaces".into(), "8".into(),
                        "-pix_fmt".into(), "yuv420p".into(),
                    ]);
                }
                Some(HardwareEncoderType::Qsv) => {
                    args.extend([
                        "-c:v".into(), "hevc_qsv".into(),
                        "-preset".into(), "medium".into(),
                        "-global_quality".into(), "21".into(),
                        "-maxrate".into(), format!("{}k", targets.maxrate_kbps),
                        "-bufsize".into(), format!("{}k", targets.bufsize_kbps),
                        "-pix_fmt".into(), "nv12".into(),
                    ]);
                }
                Some(HardwareEncoderType::Amf) => {
                    args.extend([
                        "-c:v".into(), "hevc_amf".into(),
                        "-quality".into(), "quality".into(),
                        "-rc".into(), "cqp".into(),
                        "-qp_i".into(), "21".into(),
                        "-qp_p".into(), "22".into(),
                        "-pix_fmt".into(), "nv12".into(),
                    ]);
                }
                _ => {
                    args.extend(build_software_encoder_args("hevc", width, height, fps));
                }
            }
        }
        "vp9" => {
            args.extend(build_software_encoder_args("vp9", width, height, fps));
        }
        "av1" => {
            args.extend(build_software_encoder_args("av1", width, height, fps));
        }
        _ => {
            // Default: H.264
            let hw = detect_h264_hardware_encoder();
            match hw {
                Some(HardwareEncoderType::Nvenc) => {
                    args.extend([
                        "-c:v".into(), "h264_nvenc".into(),
                        "-preset".into(), "p5".into(),
                        "-tune".into(), "hq".into(),
                        "-rc".into(), "vbr".into(),
                        "-cq".into(), "19".into(),
                        "-b:v".into(), "0".into(),
                        "-maxrate".into(), format!("{}k", targets.maxrate_kbps),
                        "-bufsize".into(), format!("{}k", targets.bufsize_kbps),
                        "-surfaces".into(), "8".into(),
                        "-pix_fmt".into(), "yuv420p".into(),
                    ]);
                }
                Some(HardwareEncoderType::Qsv) => {
                    args.extend([
                        "-c:v".into(), "h264_qsv".into(),
                        "-preset".into(), "medium".into(),
                        "-global_quality".into(), "19".into(),
                        "-maxrate".into(), format!("{}k", targets.maxrate_kbps),
                        "-bufsize".into(), format!("{}k", targets.bufsize_kbps),
                        "-pix_fmt".into(), "nv12".into(),
                    ]);
                }
                Some(HardwareEncoderType::Amf) => {
                    args.extend([
                        "-c:v".into(), "h264_amf".into(),
                        "-quality".into(), "quality".into(),
                        "-rc".into(), "cqp".into(),
                        "-qp_i".into(), "19".into(),
                        "-qp_p".into(), "20".into(),
                        "-pix_fmt".into(), "nv12".into(),
                    ]);
                }
                Some(HardwareEncoderType::MediaFoundation) => {
                    args.extend([
                        "-c:v".into(), "h264_mf".into(),
                        "-rate_control".into(), "quality".into(),
                        "-quality".into(), "100".into(),
                        "-b:v".into(), format!("{}k", targets.base_kbps),
                        "-maxrate".into(), format!("{}k", targets.maxrate_kbps),
                        "-pix_fmt".into(), "yuv420p".into(),
                    ]);
                }
                None => {
                    args.extend(build_software_encoder_args("h264", width, height, fps));
                }
            }
        }
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_bitrate_targets_1080p_30fps() {
        let targets = calculate_bitrate_targets(1920, 1080, 30.0);
        assert_eq!(targets.base_kbps, 16_000);
        assert_eq!(targets.maxrate_kbps, 45_000);
        assert_eq!(targets.bufsize_kbps, 90_000);
    }

    #[test]
    fn test_calculate_bitrate_targets_4k_60fps() {
        let targets = calculate_bitrate_targets(3840, 2160, 60.0);
        // 4K 60fps is 8x more pixels than 1080p 30fps, capped at 90 Mbps base / 150 Mbps max
        assert!(targets.maxrate_kbps >= 90_000);
        assert!(targets.bufsize_kbps >= 180_000);
    }

    #[test]
    fn test_calculate_bitrate_targets_720p() {
        let targets = calculate_bitrate_targets(1280, 720, 30.0);
        assert!(targets.base_kbps < 16_000);
        assert!(targets.maxrate_kbps < 45_000);
    }

    #[test]
    fn test_build_video_encoder_args_returns_valid_flags() {
        let args = build_video_encoder_args("h264", 1920, 1080, 30.0);
        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"-maxrate".to_string()));
        assert!(args.contains(&"-bufsize".to_string()));
        assert!(args.contains(&"45000k".to_string()));
    }
}
