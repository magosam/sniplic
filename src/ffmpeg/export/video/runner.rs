use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{AppError, AppResult};
use crate::ffmpeg::export::types::ExportProgressUpdate;

pub fn run_ffmpeg_export(
    mut cmd: Command,
    target_path: &str,
    total_duration_sec: f64,
    temp_files: Vec<PathBuf>,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<Arc<dyn Fn(ExportProgressUpdate) + Send + Sync>>,
    on_pid: Option<Arc<dyn Fn(u32) + Send + Sync>>,
) -> AppResult<()> {
    cmd.args(["-progress", "pipe:1", "-stats_period", "0.1"]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    if cancel_flag.load(Ordering::SeqCst) {
        for path in &temp_files {
            let _ = std::fs::remove_file(path);
        }
        return Err(AppError::InvalidOperation("Export cancelled by user.".into()));
    }

    if !crate::ffmpeg::binary::is_ffmpeg_available() {
        for path in &temp_files {
            let _ = std::fs::remove_file(path);
        }
        return Err(AppError::InvalidOperation(
            "FFMPEG_NOT_FOUND: FFmpeg was not found on the operating system. Click 'Install FFmpeg' to automatically configure export components.".into()
        ));
    }

    let mut child = cmd.spawn().map_err(|e| {
        for path in &temp_files {
            let _ = std::fs::remove_file(path);
        }
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to invoke FFmpeg: {}", e),
        ))
    })?;

    if let Some(ref cb) = on_pid {
        cb(child.id());
    }

    if let Some(ref cb) = progress_callback {
        cb(ExportProgressUpdate {
            percentage: 0.0,
            stage: "starting".to_string(),
            message: "Starting encoder...".to_string(),
            fps: None,
            frame: None,
            speed: None,
        });
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let progress_cb_clone = progress_callback.clone();
    let dur = total_duration_sec;
    let stdout_handle = std::thread::spawn(move || {
        if let Some(out) = stdout {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(out);

            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("out_time_us=") {
                    if let Ok(us) = rest.trim().parse::<f64>() {
                        let sec = us / 1_000_000.0;
                        if dur > 0.0 {
                            let raw_ratio = (sec / dur).clamp(0.0, 1.0);
                            let pct = (raw_ratio * 99.0).clamp(0.0, 99.0);
                            if let Some(ref cb) = progress_cb_clone {
                                cb(ExportProgressUpdate {
                                    percentage: pct,
                                    stage: "encoding".to_string(),
                                    message: "Rendering project...".to_string(),
                                    fps: None,
                                    frame: None,
                                    speed: None,
                                });
                            }
                        }
                    }
                } else if trimmed == "progress=end" {
                    if let Some(ref cb) = progress_cb_clone {
                        cb(ExportProgressUpdate {
                            percentage: 99.0,
                            stage: "finishing".to_string(),
                            message: "Finalizing media container and metadata...".to_string(),
                            fps: None,
                            frame: None,
                            speed: None,
                        });
                    }
                }
            }
        }
    });

    let stderr_handle = std::thread::spawn(move || {
        let mut err_str = String::new();
        if let Some(mut err) = stderr {
            use std::io::Read;
            let _ = err.read_to_string(&mut err_str);
        }
        err_str
    });

    let status_res = child.wait();

    let _ = stdout_handle.join();
    let stderr_content = stderr_handle.join().unwrap_or_default();

    for path in &temp_files {
        let _ = std::fs::remove_file(path);
    }

    // If the user requested cancellation during rendering
    if cancel_flag.load(Ordering::SeqCst) {
        if Path::new(target_path).exists() {
            let _ = std::fs::remove_file(target_path);
        }
        return Err(AppError::InvalidOperation("Export cancelled by user.".into()));
    }

    let status = status_res.map_err(|e| {
        if Path::new(target_path).exists() {
            let _ = std::fs::remove_file(target_path);
        }
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to wait for FFmpeg: {}", e),
        ))
    })?;

    if !status.success() {
        if Path::new(target_path).exists() {
            let _ = std::fs::remove_file(target_path);
        }
        let meaningful_errors: Vec<&str> = stderr_content
            .lines()
            .filter(|line| {
                let l = line.trim();
                !l.is_empty()
                    && !l.starts_with("frame=")
                    && !l.starts_with("fps=")
                    && !l.starts_with("size=")
                    && !l.starts_with("video:")
                    && !l.starts_with("audio:")
                    && !l.starts_with("subtitle:")
                    && !l.starts_with("[swscaler @")
                    && !l.starts_with("[libx264 @")
                    && !l.starts_with("[libx265 @")
                    && !l.starts_with("configuration:")
                    && !l.starts_with("ffmpeg version")
                    && !l.starts_with("built with")
                    && !l.starts_with("libav")
                    && !l.starts_with("libsw")
                    && !l.starts_with("--enable-")
                    && !l.starts_with("--disable-")
            })
            .collect();
        let display_lines: Vec<&str> = if meaningful_errors.is_empty() {
            let all: Vec<&str> = stderr_content.lines().collect();
            let start = all.len().saturating_sub(10);
            all[start..].to_vec()
        } else {
            let start = meaningful_errors.len().saturating_sub(10);
            meaningful_errors[start..].to_vec()
        };
        let err_snippet = display_lines.join("\n");
        return Err(AppError::InvalidOperation(format!(
            "Video rendering error:\n{}",
            err_snippet
        )));
    }

    if let Some(ref cb) = progress_callback {
        cb(ExportProgressUpdate {
            percentage: 100.0,
            stage: "completed".to_string(),
            message: "Export completed successfully!".to_string(),
            fps: None,
            frame: None,
            speed: None,
        });
    }

    Ok(())
}
