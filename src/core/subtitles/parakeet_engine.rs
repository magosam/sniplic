//! Native inference engine for the Parakeet v3 (TDT) model.
//!
//! Responsible for:
//! - Locating local ONNX models in `%LOCALAPPDATA%/com.sniplic.editor/models/parakeet_v3/`
//! - Unpacking and verifying encrypted and signed `.snip` packages
//! - Executing transcription with tokens and durations (word-level timestamps).

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub const PARAKEET_MODEL_PUBLIC_KEY_HEX: &str =
    "cf37feb614b9a5286285f26ac54670d587badef2fd0dbc6c5a60ba857d8780a1";

pub const PARAKEET_AES_KEY: [u8; 32] = [
    0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0x6f, 0x70, 0x81,
    0x92, 0xa3, 0xb4, 0xc5, 0xd6, 0xe7, 0xf8, 0x09,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPackageHeader {
    pub magic: String,
    pub version: u32,
    pub model_id: String,
    pub model_version: String,
    pub created_at: u64,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub signature: String,
    pub nonce: String,
    pub files: Vec<String>,
}

pub fn get_parakeet_model_dir() -> PathBuf {
    let local_data = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    local_data
        .join("com.sniplic.editor")
        .join("models")
        .join("parakeet_v3")
}

pub fn is_parakeet_model_installed() -> bool {
    let dir = get_parakeet_model_dir();
    let encoder = dir.join("encoder-model.int8.onnx");
    let decoder = dir.join("decoder_joint-model.int8.onnx");
    let vocab = dir.join("vocab.txt");

    encoder.exists() && decoder.exists() && vocab.exists()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err("Hex string with odd length".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex character: {}", e))
        })
        .collect()
}

/// Unpacks, decrypts, and validates the Ed25519 signature of a `.snip` model file.
/// Extracts ONNX files directly to Sniplic's model directory with granular telemetry.
pub fn unpack_and_install_parakeet_model_with_progress<F>(
    snip_path: &Path,
    mut on_progress: F,
) -> Result<PathBuf, String>
where
    F: FnMut(u32, &str),
{
    if !snip_path.exists() {
        return Err(format!("Model file not found: {}", snip_path.display()));
    }

    on_progress(90, "Verifying package integrity and encryption...");

    let mut file = File::open(snip_path)
        .map_err(|e| format!("Failed to open .snip package: {}", e))?;

    // 1. Validate Magic b"SNIP"
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .map_err(|e| format!("Failed to read .snip header: {}", e))?;

    if &magic != b"SNIP" {
        return Err("Invalid format: File is not a Sniplic .snip package.".to_string());
    }

    // 2. Read header length
    let mut header_len_bytes = [0u8; 4];
    file.read_exact(&mut header_len_bytes)
        .map_err(|e| format!("Failed to read header length: {}", e))?;
    let header_len = u32::from_le_bytes(header_len_bytes) as usize;

    if header_len > 1_000_000 {
        return Err("Corrupted or excessively large header.".to_string());
    }

    let mut header_buf = vec![0u8; header_len];
    file.read_exact(&mut header_buf)
        .map_err(|e| format!("Failed to read header data: {}", e))?;

    let header: ModelPackageHeader = serde_json::from_slice(&header_buf)
        .map_err(|e| format!("Failed to deserialize package header: {}", e))?;

    if header.magic != "SNIP_MODEL" {
        return Err(format!("Invalid magic in header: {}", header.magic));
    }

    // 3. Read remaining encrypted payload
    let mut ciphertext = Vec::new();
    file.read_to_end(&mut ciphertext)
        .map_err(|e| format!("Failed to read encrypted payload: {}", e))?;

    on_progress(91, "Decrypting AES-256-GCM payload in memory...");

    // 4. Decrypt with AES-256-GCM
    let nonce_vec = hex_decode(&header.nonce)?;
    if nonce_vec.len() != 12 {
        return Err("Invalid nonce (expected 12 bytes).".to_string());
    }
    let nonce = Nonce::from_slice(&nonce_vec);

    let cipher = Aes256Gcm::new_from_slice(&PARAKEET_AES_KEY)
        .map_err(|e| format!("Invalid AES key: {}", e))?;

    let decrypted_zip = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| "AES-GCM authentication failure: Incorrect key or corrupted/tampered file.".to_string())?;

    // 5. Validate Ed25519 signature
    let pub_key_bytes = hex_decode(PARAKEET_MODEL_PUBLIC_KEY_HEX)?;
    if pub_key_bytes.len() != 32 {
        return Err("Invalid Ed25519 public key.".to_string());
    }
    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pub_key_bytes);
    let verifying_key = VerifyingKey::from_bytes(&pk_arr)
        .map_err(|e| format!("Corrupted Ed25519 public key: {}", e))?;

    let sig_bytes = hex_decode(&header.signature)?;
    if sig_bytes.len() != 64 {
        return Err("Invalid Ed25519 signature (expected 64 bytes).".to_string());
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    verifying_key
        .verify(&decrypted_zip, &signature)
        .map_err(|_| "Invalid digital signature! The model file was modified or is not official Sniplic release.".to_string())?;

    // 6. Extract ZIP files to destination directory with progress from 91% to 99%
    let target_dir = get_parakeet_model_dir();
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create model directory: {}", e))?;

    let total_uncompressed = header.uncompressed_size.max(1);
    let mut extracted_bytes: u64 = 0;
    let mut last_reported_pct = 91u32;

    let cursor = Cursor::new(decrypted_zip);
    let mut zip = ZipArchive::new(cursor)
        .map_err(|e| format!("Internal ZIP file corrupted: {}", e))?;

    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("Failed to read file {} from ZIP: {}", i, e))?;

        let entry_name = entry.name().to_string();
        // Prevent Path Traversal
        let clean_name = Path::new(&entry_name)
            .file_name()
            .ok_or_else(|| "Invalid filename in package".to_string())?;

        let dest_path = target_dir.join(clean_name);
        let mut outfile = File::create(&dest_path)
            .map_err(|e| format!("Failed to create destination file {}: {}", dest_path.display(), e))?;

        // 1 MB buffer copy to emit continuous progress from 91% to 99%
        let mut buffer = [0u8; 1024 * 1024];
        loop {
            let bytes_read = entry.read(&mut buffer)
                .map_err(|e| format!("Failed to read buffer from {}: {}", entry_name, e))?;
            if bytes_read == 0 {
                break;
            }
            outfile.write_all(&buffer[..bytes_read])
                .map_err(|e| format!("Failed to write data to {}: {}", dest_path.display(), e))?;

            extracted_bytes += bytes_read as u64;
            let ratio = (extracted_bytes as f64 / total_uncompressed as f64).clamp(0.0, 1.0);
            let current_pct = 91 + (ratio * 8.0) as u32; // smooth linear scale from 91% to 99%

            if current_pct > last_reported_pct && current_pct < 100 {
                last_reported_pct = current_pct;
                let mb_done = extracted_bytes as f64 / 1_000_000.0;
                let mb_total = total_uncompressed as f64 / 1_000_000.0;
                on_progress(
                    current_pct,
                    &format!("Installing model files... {:.0}/{:.0} MB ({}%)", mb_done, mb_total, current_pct),
                );
            }
        }
    }

    tracing::info!(dir = %target_dir.display(), "Parakeet v3 model successfully installed");
    Ok(target_dir)
}

/// Unpacks, decrypts, and validates the Ed25519 signature of a `.snip` model file.
pub fn unpack_and_install_parakeet_model(snip_path: &Path) -> Result<PathBuf, String> {
    unpack_and_install_parakeet_model_with_progress(snip_path, |_, _| {})
}

/// Loads the Parakeet model vocab.txt vocabulary file
pub fn load_vocabulary(vocab_path: &Path) -> Result<(std::collections::HashMap<i32, String>, i32), String> {
    if !vocab_path.exists() {
        return Err(format!("vocab.txt not found at: {}", vocab_path.display()));
    }

    let content = fs::read_to_string(vocab_path)
        .map_err(|e| format!("Failed to read vocab.txt: {}", e))?;

    let mut vocab = std::collections::HashMap::new();
    let mut blank_idx = 8192;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((tok, id_str)) = trimmed.rsplit_once(' ') {
            if let Ok(id) = id_str.parse::<i32>() {
                let clean_tok = tok.replace('\u{2581}', " ");
                if tok == "<blk>" {
                    blank_idx = id;
                }
                vocab.insert(id, clean_tok);
            }
        }
    }

    Ok((vocab, blank_idx))
}

/// Executes high-precision full transcription on 16kHz mono WAV audio via Parakeet v3 (TDT).
/// Returns balanced clauses with word-level timestamps.
pub fn transcribe_parakeet(
    wav_path: &Path,
    _language: &str,
    style: &str,
    max_chars: Option<u32>,
) -> Result<Vec<crate::core::subtitles::balancer::SubtitleChunk>, String> {
    let model_dir = get_parakeet_model_dir();
    let encoder_path = model_dir.join("encoder-model.int8.onnx");
    let decoder_path = model_dir.join("decoder_joint-model.int8.onnx");
    let vocab_path = model_dir.join("vocab.txt");

    if !encoder_path.exists() || !decoder_path.exists() || !vocab_path.exists() {
        return Err("MODEL_NOT_INSTALLED: Parakeet v3 model is not installed. Install the AI subtitles package to continue.".to_string());
    }

    let (vocab, blank_idx) = load_vocabulary(&vocab_path)?;

    // Read samples from WAV file (expected 16kHz mono)
    let mut reader = hound::WavReader::open(wav_path)
        .map_err(|e| format!("Failed to open WAV audio: {}", e))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
    };

    if samples.is_empty() {
        return Ok(Vec::new());
    }

    // Extract 128-channel log-mel filterbanks
    let (features, features_lens) = crate::core::subtitles::audio_features::extract_log_mel_features(&samples);
    let total_frames = features.len() / 128;

    // 1. Execute ONNX Encoder
    let mut enc_session = ort::session::Session::builder()
        .map_err(|e| format!("Failed to build Encoder session: {}", e))?
        .commit_from_file(&encoder_path)
        .map_err(|e| format!("Failed to load Encoder: {}", e))?;

    let signal_tensor = ort::value::Tensor::from_array(([1, 128, total_frames], features.into_boxed_slice()))
        .map_err(|e| e.to_string())?;
    let len_tensor = ort::value::Tensor::from_array(([1], vec![features_lens].into_boxed_slice()))
        .map_err(|e| e.to_string())?;

    let enc_outputs = enc_session
        .run(ort::inputs![
            "audio_signal" => signal_tensor,
            "length" => len_tensor,
        ])
        .map_err(|e| format!("Failed to run Encoder: {}", e))?;

    let (enc_shape, enc_data) = enc_outputs["outputs"]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract Encoder output: {}", e))?;

    let enc_len = if enc_shape.len() >= 3 {
        enc_shape[2] as usize
    } else {
        0
    };

    if enc_len == 0 {
        return Ok(Vec::new());
    }

    // 2. Execute Decoder Joint with greedy TDT decoding
    let mut dec_session = ort::session::Session::builder()
        .map_err(|e| format!("Failed to build Decoder session: {}", e))?
        .commit_from_file(&decoder_path)
        .map_err(|e| format!("Failed to load Decoder: {}", e))?;

    let mut t: usize = 0;
    let mut emitted_tokens: usize = 0;
    let max_tokens_per_step: usize = 10;
    let mut state1_buf = vec![0.0f32; 2 * 640];
    let mut state2_buf = vec![0.0f32; 2 * 640];
    let mut last_token: i32 = blank_idx;

    let mut words: Vec<crate::core::subtitles::balancer::TimedWord> = Vec::new();
    let mut current_word = String::new();
    let mut current_word_start: f64 = 0.0;
    let mut current_word_end: f64 = 0.0;
    let frame_duration_sec = 0.08f64; // 80ms per FastConformer frame (hop=160, subsample=8)

    while t < enc_len {
        let mut enc_frame = vec![0.0f32; 1024];
        for c in 0..1024 {
            enc_frame[c] = enc_data[c * enc_len + t];
        }

        let enc_slice = ort::value::Tensor::from_array(([1, 1024, 1], enc_frame.into_boxed_slice()))
            .map_err(|e| e.to_string())?;
        let targets = ort::value::Tensor::from_array(([1, 1], vec![last_token].into_boxed_slice()))
            .map_err(|e| e.to_string())?;
        let target_len = ort::value::Tensor::from_array(([1], vec![1i32].into_boxed_slice()))
            .map_err(|e| e.to_string())?;
        let state1 = ort::value::Tensor::from_array(([2, 1, 640], state1_buf.clone().into_boxed_slice()))
            .map_err(|e| e.to_string())?;
        let state2 = ort::value::Tensor::from_array(([2, 1, 640], state2_buf.clone().into_boxed_slice()))
            .map_err(|e| e.to_string())?;

        let dec_outputs = dec_session
            .run(ort::inputs![
                "encoder_outputs" => enc_slice,
                "targets" => targets,
                "target_length" => target_len,
                "input_states_1" => state1,
                "input_states_2" => state2,
            ])
            .map_err(|e| format!("Failed in Decoder Joint at frame {}: {}", t, e))?;

        let (_, out_logits) = dec_outputs["outputs"]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        let (_, out_s1) = dec_outputs["output_states_1"]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        let (_, out_s2) = dec_outputs["output_states_2"]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;

        // 0..8193 are token logits
        let mut best_token = 0i32;
        let mut best_token_logit = f32::NEG_INFINITY;
        for i in 0..8193 {
            if out_logits[i] > best_token_logit {
                best_token_logit = out_logits[i];
                best_token = i as i32;
            }
        }

        // 8193..8198 are the 5 duration logits (0, 1, 2, 3, 4)
        let mut best_duration = 0usize;
        let mut best_dur_logit = f32::NEG_INFINITY;
        for d in 0..5 {
            if out_logits[8193 + d] > best_dur_logit {
                best_dur_logit = out_logits[8193 + d];
                best_duration = d;
            }
        }

        if best_token != blank_idx {
            last_token = best_token;
            state1_buf.copy_from_slice(out_s1);
            state2_buf.copy_from_slice(out_s2);
            emitted_tokens += 1;

            if let Some(tok_str) = vocab.get(&best_token) {
                if !tok_str.starts_with("<|") && !tok_str.starts_with('<') {
                    let tok_time_start = (t as f64) * frame_duration_sec;
                    let tok_time_end = ((t + best_duration.max(1)) as f64) * frame_duration_sec;

                    if tok_str.starts_with(' ') {
                        if !current_word.trim().is_empty() {
                            words.push(crate::core::subtitles::balancer::TimedWord {
                                text: current_word.trim().to_string(),
                                start: current_word_start,
                                end: current_word_end,
                            });
                        }
                        current_word = tok_str.trim_start().to_string();
                        current_word_start = tok_time_start;
                        current_word_end = tok_time_end;
                    } else {
                        if current_word.is_empty() {
                            current_word_start = tok_time_start;
                        }
                        current_word.push_str(tok_str);
                        current_word_end = tok_time_end;
                    }
                }
            }
        }

        if best_duration > 0 {
            t += best_duration;
            emitted_tokens = 0;
        } else if best_token == blank_idx || emitted_tokens >= max_tokens_per_step {
            t += 1;
            emitted_tokens = 0;
        }
    }

    if !current_word.trim().is_empty() {
        words.push(crate::core::subtitles::balancer::TimedWord {
            text: current_word.trim().to_string(),
            start: current_word_start,
            end: current_word_end,
        });
    }

    // 3. Balance words into ideal clauses according to style and size
    let effective_max_chars = if style == "reels_short" {
        max_chars.unwrap_or(22).max(10) as usize
    } else {
        max_chars.unwrap_or(36).max(10) as usize
    };

    let clauses = crate::core::subtitles::balancer::group_words_into_balanced_clauses(
        &words,
        effective_max_chars,
        0.5,
    );

    Ok(clauses)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_installed_check() {
        let _ = is_parakeet_model_installed();
    }

    #[test]
    fn test_snip_package_header_validity() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let snip_path = workspace_root.join("dist_plugins").join("subtitles.auto.parakeet.snip");
        if snip_path.exists() {
            let mut file = File::open(&snip_path).unwrap();
            let mut magic = [0u8; 4];
            file.read_exact(&mut magic).unwrap();
            assert_eq!(&magic, b"SNIP");

            let mut header_len_bytes = [0u8; 4];
            file.read_exact(&mut header_len_bytes).unwrap();
            let header_len = u32::from_le_bytes(header_len_bytes) as usize;
            assert!(header_len > 0 && header_len < 100_000);

            let mut header_buf = vec![0u8; header_len];
            file.read_exact(&mut header_buf).unwrap();
            let header: ModelPackageHeader = serde_json::from_slice(&header_buf).unwrap();
            assert_eq!(header.magic, "SNIP_MODEL");
            assert_eq!(header.model_id, "subtitles.auto.parakeet");
            assert!(header.files.contains(&"encoder-model.int8.onnx".to_string()));
            assert!(header.files.contains(&"decoder_joint-model.int8.onnx".to_string()));
        }
    }

    #[test]
    fn test_inspect_onnx_model_metadata() {
        let hf_cache = std::env::var("USERPROFILE").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("C:\\")).join(".cache\\huggingface\\hub\\models--istupakov--parakeet-tdt-0.6b-v3-onnx\\snapshots\\8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce");
        let encoder_path = hf_cache.join("encoder-model.int8.onnx");
        let decoder_path = hf_cache.join("decoder_joint-model.int8.onnx");

        if encoder_path.exists() && decoder_path.exists() {
            println!("Testing ONNX inference pipeline...");
            let mut enc_session = ort::session::Session::builder().unwrap().commit_from_file(&encoder_path).unwrap();
            let mut dec_session = ort::session::Session::builder().unwrap().commit_from_file(&decoder_path).unwrap();

            let (features, features_lens) = crate::core::subtitles::audio_features::extract_log_mel_features(&vec![0.0f32; 16000]);
            let total_frames = features.len() / 128;

            let signal_tensor = ort::value::Tensor::from_array(([1, 128, total_frames], features.into_boxed_slice())).unwrap();
            let len_tensor = ort::value::Tensor::from_array(([1], vec![features_lens].into_boxed_slice())).unwrap();

            let enc_outputs = enc_session.run(ort::inputs![
                "audio_signal" => signal_tensor,
                "length" => len_tensor,
            ]).unwrap();

            let (enc_shape, enc_data) = enc_outputs["outputs"].try_extract_tensor::<f32>().unwrap();
            println!("Encoder outputs shape: {:?}, data len: {}", enc_shape, enc_data.len());

            let (lens_shape, lens_data) = enc_outputs["encoded_lengths"].try_extract_tensor::<i64>().unwrap();
            println!("Encoder lengths: {:?}, data: {:?}", lens_shape, lens_data);

            // Test Decoder Joint with 1 step
            let enc_slice = ort::value::Tensor::from_array(([1, 1024, 1], vec![0.0f32; 1024].into_boxed_slice())).unwrap();
            let targets = ort::value::Tensor::from_array(([1, 1], vec![8192i32].into_boxed_slice())).unwrap();
            let target_len = ort::value::Tensor::from_array(([1], vec![1i32].into_boxed_slice())).unwrap();
            let state1 = ort::value::Tensor::from_array(([2, 1, 640], vec![0.0f32; 2 * 640].into_boxed_slice())).unwrap();
            let state2 = ort::value::Tensor::from_array(([2, 1, 640], vec![0.0f32; 2 * 640].into_boxed_slice())).unwrap();

            let dec_outputs = dec_session.run(ort::inputs![
                "encoder_outputs" => enc_slice,
                "targets" => targets,
                "target_length" => target_len,
                "input_states_1" => state1,
                "input_states_2" => state2,
            ]).unwrap();

            let (logits_shape, logits_data) = dec_outputs["outputs"].try_extract_tensor::<f32>().unwrap();
            println!("Decoder output logits shape: {:?}, data len: {}", logits_shape, logits_data.len());
        }
    }

    #[test]
    fn test_unpack_and_transcribe_end_to_end() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let snip_path = workspace_root.join("dist_plugins").join("subtitles.auto.parakeet.snip");

        if snip_path.exists() && !is_parakeet_model_installed() {
            println!("Unpacking .snip package for test...");
            let res = unpack_and_install_parakeet_model(&snip_path);
            assert!(res.is_ok(), "Unpack failed: {:?}", res);
        }

        if is_parakeet_model_installed() {
            println!("Testing transcribe_parakeet on a synthesized 16kHz WAV file...");
            let temp_wav = std::env::temp_dir().join("test_sniplic_transcribe.wav");
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 16000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::create(&temp_wav, spec).unwrap();
            for i in 0..16000 {
                let sample = (0.2 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin() * 32767.0) as i16;
                writer.write_sample(sample).unwrap();
            }
            writer.finalize().unwrap();

            let clauses = transcribe_parakeet(&temp_wav, "pt-BR", "standard", Some(36));
            assert!(clauses.is_ok(), "Transcription failed: {:?}", clauses);
            println!("Transcription result: {:?}", clauses.unwrap());
            let _ = std::fs::remove_file(&temp_wav);
        }
    }
}
