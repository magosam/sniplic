use super::super::types::AudioRenderItem;

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

/// Builds audio filters for filter_complex and returns (audio_filter_str, final_audio_label)
pub fn build_audio_filter_complex(
    audio_items: &[AudioRenderItem],
    visual_count: usize,
    project_fps: f64,
    total_duration_sec: f64,
) -> (String, String) {
    if !audio_items.is_empty() {
        let mut filter_str = String::new();
        let mut audio_labels = Vec::new();
        for j in 0..audio_items.len() {
            let item = &audio_items[j];
            let input_idx = visual_count + j;
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
            
            // Audio Noise Reduction (Denoise)
            if item.denoise > 0.0 {
                let nf = -10.0 - (item.denoise * 0.4); // Maps 0-100 to -10 to -50dB
                chain.push(format!("afftdn=nf={:.1}", nf));
            }
            
            // Audio Equalizer (Bass/Mid/Treble)
            if item.eq_bass != 0.0 || item.eq_mid != 0.0 || item.eq_treble != 0.0 {
                let bass = item.eq_bass;
                let mid = item.eq_mid;
                let treble = item.eq_treble;
                chain.push(format!("anequalizer=c0 f=200 w=100 g={:.1}|c0 f=1000 w=500 g={:.1}|c0 f=8000 w=2000 g={:.1}|c1 f=200 w=100 g={:.1}|c1 f=1000 w=500 g={:.1}|c1 f=8000 w=2000 g={:.1}", bass, mid, treble, bass, mid, treble));
            }

            chain.push(format!("volume={:.2}", item.volume));

            filter_str.push_str(&format!(
                "[{}:a]{}[a_delayed{}];",
                input_idx,
                chain.join(","),
                j
            ));
            audio_labels.push(format!("[a_delayed{}]", j));
        }

        filter_str.push_str(&format!(
            "aevalsrc=0:d={:.4}:s=48000:c=stereo[abase];",
            total_duration_sec
        ));
        audio_labels.insert(0, "[abase]".to_string());

        filter_str.push_str(&format!(
            "{}amix=inputs={}:duration=first:dropout_transition=0:normalize=0[outa];",
            audio_labels.join(""),
            audio_labels.len()
        ));
        (filter_str, "[outa]".to_string())
    } else {
        let filter_str = format!(
            "aevalsrc=0:d={:.4}:s=48000:c=stereo[outa];",
            total_duration_sec
        );
        (filter_str, "[outa]".to_string())
    }
}
