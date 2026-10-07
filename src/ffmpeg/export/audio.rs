use crate::error::{AppError, AppResult};
use super::types::{AudioRenderItem, ExportSettings};

fn append_atempo_filters(chain: &mut Vec<String>, mut speed: f64) {
    if (speed - 1.0).abs() < 0.001 || speed <= 0.0 {
        return;
    }
    while speed > 2.0 {
        chain.push("atempo=2.0".to_string());
        speed /= 2.0;
    }
    while speed < 0.5 {
        chain.push("atempo=0.5".to_string());
        speed /= 0.5;
    }
    if (speed - 1.0).abs() > 0.001 {
        chain.push(format!("atempo={:.4}", speed));
    }
}

pub fn render_audio_pipeline(
    settings: &ExportSettings,
    target_path: &str,
    audio_items: &[AudioRenderItem],
    project_fps: f64,
    total_duration_sec: f64,
    cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    on_pid: Option<std::sync::Arc<dyn Fn(u32) + Send + Sync>>,
) -> AppResult<()> {
    if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(AppError::InvalidOperation("Exportação cancelada pelo usuário.".into()));
    }

    let mut cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
    cmd.args(["-hide_banner", "-y"]);

    for item in audio_items {
        let in_sec = item.in_point_frames as f64 / project_fps;
        let dur_sec = (item.duration_frames as f64 / project_fps) * item.speed;
        cmd.args([
            "-ss",
            &format!("{:.4}", in_sec),
            "-t",
            &format!("{:.4}", dur_sec + 0.5),
            "-i",
        ])
        .arg(&item.file_path);
    }

    let mut filter_complex = String::new();
    let final_a_label = if !audio_items.is_empty() {
        let mut audio_labels = Vec::new();
        for j in 0..audio_items.len() {
            let item = &audio_items[j];
            let start_ms = ((item.start_frame as f64 / project_fps) * 1000.0).round() as u64;
            let clip_dur_sec = item.duration_frames as f64 / project_fps;
            let mut chain = Vec::new();
            if item.reversed {
                chain.push("areverse".to_string());
            }
            append_atempo_filters(&mut chain, item.speed);
            chain.push(format!("atrim=duration={:.6}", clip_dur_sec));
            chain.push("asetpts=PTS-STARTPTS".to_string());
            if item.fade_in_frames > 0 {
                let in_sec = (item.fade_in_frames as f64 / project_fps).min(clip_dur_sec / 2.0);
                chain.push(format!("afade=t=in:st=0:d={:.4}:curve=hsin", in_sec));
            }
            if item.fade_out_frames > 0 {
                let out_sec = (item.fade_out_frames as f64 / project_fps).min(clip_dur_sec / 2.0);
                let out_start = (clip_dur_sec - out_sec).max(0.0);
                chain.push(format!("afade=t=out:st={:.4}:d={:.4}:curve=hsin", out_start, out_sec));
            }
            chain.push("aresample=48000:async=1".to_string());
            chain.push(format!("adelay={}|{}", start_ms, start_ms));
            chain.push(format!("volume={:.2}", item.volume));

            filter_complex.push_str(&format!(
                "[{}:a]{}[a_delayed{}];",
                j,
                chain.join(","),
                j
            ));
            audio_labels.push(format!("[a_delayed{}]", j));
        }

        filter_complex.push_str(&format!(
            "aevalsrc=0:d={:.4}:s=48000:c=stereo[abase];",
            total_duration_sec
        ));
        audio_labels.insert(0, "[abase]".to_string());

        filter_complex.push_str(&format!(
            "{}amix=inputs={}:duration=first:dropout_transition=0:normalize=0[outa];",
            audio_labels.join(""),
            audio_labels.len()
        ));
        "[outa]"
    } else {
        filter_complex.push_str(&format!(
            "aevalsrc=0:d={:.4}:s=48000:c=stereo[outa];",
            total_duration_sec
        ));
        "[outa]"
    };

    let temp_script_path = std::env::temp_dir().join(format!("sniplic_audio_filter_{}.txt", uuid::Uuid::new_v4().simple()));
    if let Err(e) = std::fs::write(&temp_script_path, &filter_complex) {
        return Err(AppError::Io(e));
    }

    let filter_flag = crate::ffmpeg::export::get_filter_complex_script_flag();
    cmd.args([filter_flag, temp_script_path.to_string_lossy().as_ref()]);
    cmd.arg("-vn");
    cmd.args(["-map", final_a_label]);

    let acodec = match settings.audio_format.as_str() {
        "wav" => "pcm_s16le",
        "mp3" => "libmp3lame",
        _ => "aac",
    };

    cmd.args(["-c:a", acodec]);
    if settings.audio_format != "wav" {
        cmd.args(["-b:a", &format!("{}k", settings.audio_bitrate_kbps)]);
    }

    cmd.args(["-t", &format!("{:.4}", total_duration_sec)]);
    cmd.arg(target_path);

    if !crate::ffmpeg::binary::is_ffmpeg_available() {
        let _ = std::fs::remove_file(&temp_script_path);
        return Err(AppError::InvalidOperation(
            "FFMPEG_NOT_FOUND: O FFmpeg não foi encontrado no sistema operacional. Clique em 'Instalar FFmpeg' para configurar os componentes de exportação automaticamente.".into()
        ));
    }

    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        let _ = std::fs::remove_file(&temp_script_path);
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Falha ao invocar FFmpeg para áudio: {}", e),
        ))
    })?;

    if let Some(ref cb) = on_pid {
        cb(child.id());
    }

    let stderr = child.stderr.take();
    let stderr_handle = std::thread::spawn(move || {
        let mut err_str = String::new();
        if let Some(mut err) = stderr {
            use std::io::Read;
            let _ = err.read_to_string(&mut err_str);
        }
        err_str
    });

    let status_res = child.wait();
    let stderr_content = stderr_handle.join().unwrap_or_default();
    let _ = std::fs::remove_file(&temp_script_path);

    if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
        if std::path::Path::new(target_path).exists() {
            let _ = std::fs::remove_file(target_path);
        }
        return Err(AppError::InvalidOperation("Exportação cancelada pelo usuário.".into()));
    }

    let status = status_res.map_err(|e| {
        if std::path::Path::new(target_path).exists() {
            let _ = std::fs::remove_file(target_path);
        }
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Falha ao aguardar FFmpeg para áudio: {}", e),
        ))
    })?;

    if !status.success() {
        if std::path::Path::new(target_path).exists() {
            let _ = std::fs::remove_file(target_path);
        }
        return Err(AppError::InvalidOperation(format!(
            "Erro na renderização de áudio:\n{}",
            stderr_content.lines().rev().take(6).collect::<Vec<_>>().join("\n")
        )));
    }

    Ok(())
}
