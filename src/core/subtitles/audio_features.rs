//! Native extraction of audio acoustic features (128-channel Log-Mel Filterbanks)
//! 100% compatible with the NeMo Conformer / Parakeet v3 preprocessor.
//!
//! Free of any external Python dependencies or C audio libraries.

use std::f32::consts::PI;

pub const SAMPLE_RATE: usize = 16_000;
pub const N_FFT: usize = 512;
pub const WIN_LENGTH: usize = 400;
pub const HOP_LENGTH: usize = 160;
pub const N_MELS: usize = 128;
pub const PREEMPH_COEFF: f32 = 0.97;
pub const LOG_ZERO_GUARD: f32 = 5.9604644775390625e-8; // 2^-24

// Load precomputed NeMo Mel filterbanks (257 frequency bins x 128 mel channels)
pub static NEMO128_FBANKS: &[u8] = include_bytes!("nemo128_fbanks.bin");

/// Executes in-place Radix-2 FFT on a 512 complex numbers buffer.
pub fn fft_512(re: &mut [f32; 512], im: &mut [f32; 512]) {
    // 1. Bit-reversal permutation (9 bits for N=512)
    for i in 0..512usize {
        let rev = i.reverse_bits() >> (usize::BITS - 9);
        if i < rev {
            re.swap(i, rev);
            im.swap(i, rev);
        }
    }

    // 2. Cooley-Tukey Butterflies
    let mut len = 2;
    while len <= 512 {
        let half = len / 2;
        let angle = -2.0 * PI / (len as f32);
        let w_step_re = angle.cos();
        let w_step_im = angle.sin();

        let mut i = 0;
        while i < 512 {
            let mut w_re = 1.0f32;
            let mut w_im = 0.0f32;

            for j in 0..half {
                let u_re = re[i + j];
                let u_im = im[i + j];

                let v_re = re[i + j + half];
                let v_im = im[i + j + half];

                let t_re = v_re * w_re - v_im * w_im;
                let t_im = v_re * w_im + v_im * w_re;

                re[i + j] = u_re + t_re;
                im[i + j] = u_im + t_im;

                re[i + j + half] = u_re - t_re;
                im[i + j + half] = u_im - t_im;

                let next_w_re = w_re * w_step_re - w_im * w_step_im;
                let next_w_im = w_re * w_step_im + w_im * w_step_re;
                w_re = next_w_re;
                w_im = next_w_im;
            }
            i += len;
        }
        len *= 2;
    }
}

/// Computes the power spectrum of a 512-sample frame with centered 400-sample Hann window.
#[inline]
pub fn compute_power_spectrum_512(frame_samples: &[f32; 512], hann_window: &[f32; 512], out_power: &mut [f32; 257]) {
    let mut re = [0.0f32; 512];
    let mut im = [0.0f32; 512];

    for i in 0..512 {
        re[i] = frame_samples[i] * hann_window[i];
    }

    fft_512(&mut re, &mut im);

    for k in 0..257 {
        out_power[k] = re[k] * re[k] + im[k] * im[k];
    }
}

/// Creates a 400-sample Hann window centered in a 512-element buffer with zero padding at edges.
pub fn create_padded_hann_window() -> [f32; 512] {
    let mut win = [0.0f32; 512];
    let pad = (N_FFT - WIN_LENGTH) / 2; // 56
    for i in 0..WIN_LENGTH {
        win[pad + i] = 0.5 * (1.0 - (2.0 * PI * i as f32 / WIN_LENGTH as f32).cos());
    }
    win
}

/// Gets the Mel filterbank matrix (257 x 128) in f32.
pub fn get_mel_fbanks() -> Vec<f32> {
    assert_eq!(NEMO128_FBANKS.len(), 257 * 128 * 4, "Invalid filterbank size");
    let mut fbanks = Vec::with_capacity(257 * 128);
    for chunk in NEMO128_FBANKS.chunks_exact(4) {
        let val = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        fbanks.push(val);
    }
    fbanks
}

/// Processes raw 16kHz mono audio samples and generates normalized Log-Mel features
/// with format `[1, 128, n_frames]` and effective frame length `n_frames`.
pub fn extract_log_mel_features(samples: &[f32]) -> (Vec<f32>, i64) {
    let num_samples = samples.len();
    if num_samples < HOP_LENGTH {
        return (vec![0.0f32; N_MELS], 1);
    }

    // 1. Pre-emphasis filter: y[t] = x[t] - 0.97 * x[t-1]
    let mut preemphed = Vec::with_capacity(num_samples);
    preemphed.push(samples[0]);
    for i in 1..num_samples {
        preemphed.push(samples[i] - PREEMPH_COEFF * samples[i - 1]);
    }

    // 2. Symmetric padding: N_FFT / 2 (256) on each edge
    let pad = N_FFT / 2;
    let mut padded = Vec::with_capacity(num_samples + 2 * pad);
    padded.resize(pad, 0.0f32);
    padded.extend_from_slice(&preemphed);
    padded.resize(num_samples + 2 * pad, 0.0f32);

    // 3. Windowing and STFT
    let hann_window = create_padded_hann_window();
    let mel_fbanks = get_mel_fbanks();

    let num_frames = num_samples / HOP_LENGTH;
    let total_frames = if padded.len() >= N_FFT {
        (padded.len() - N_FFT) / HOP_LENGTH + 1
    } else {
        0
    };

    let mut log_mel_spectrogram = vec![0.0f32; total_frames * N_MELS];
    let mut frame_buf = [0.0f32; 512];
    let mut power = [0.0f32; 257];

    for t in 0..total_frames {
        let start = t * HOP_LENGTH;
        if start + N_FFT <= padded.len() {
            frame_buf.copy_from_slice(&padded[start..start + N_FFT]);
        } else {
            frame_buf.fill(0.0);
        }

        compute_power_spectrum_512(&frame_buf, &hann_window, &mut power);

        // Mel filterbank matrix multiplication: mel[m] = sum_k power[k] * fbank[k, m]
        for m in 0..N_MELS {
            let mut mel_energy = 0.0f32;
            for k in 0..257 {
                mel_energy += power[k] * mel_fbanks[k * N_MELS + m];
            }
            let log_energy = (mel_energy + LOG_ZERO_GUARD).ln();
            log_mel_spectrogram[t * N_MELS + m] = log_energy;
        }
    }

    // 4. Per-utterance normalization (Mean and Variance Normalization over valid frames < num_frames)
    let valid_frames = num_frames.max(1);
    let mut means = vec![0.0f32; N_MELS];
    let mut vars = vec![0.0f32; N_MELS];

    for m in 0..N_MELS {
        let mut sum = 0.0f32;
        for t in 0..valid_frames {
            sum += log_mel_spectrogram[t * N_MELS + m];
        }
        let mean = sum / (valid_frames as f32);
        means[m] = mean;

        let mut var_sum = 0.0f32;
        for t in 0..valid_frames {
            let diff = log_mel_spectrogram[t * N_MELS + m] - mean;
            var_sum += diff * diff;
        }
        let var = if valid_frames > 1 {
            var_sum / ((valid_frames - 1) as f32)
        } else {
            1.0
        };
        vars[m] = var;
    }

    // 5. Transpose to [1, 128, total_frames] format expected by the NeMo model
    let mut features = vec![0.0f32; N_MELS * total_frames];
    for m in 0..N_MELS {
        let std_dev = vars[m].max(0.0).sqrt() + 1e-5;
        for t in 0..total_frames {
            if t < valid_frames {
                let norm = (log_mel_spectrogram[t * N_MELS + m] - means[m]) / std_dev;
                features[m * total_frames + t] = norm;
            } else {
                features[m * total_frames + t] = 0.0;
            }
        }
    }

    (features, num_frames as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_padded_hann_window() {
        let win = create_padded_hann_window();
        assert_eq!(win.len(), 512);
        assert_eq!(win[0], 0.0);
        assert_eq!(win[55], 0.0);
        assert!(win[256] > 0.99); // Central peak of Hann window
        assert_eq!(win[456], 0.0);
        assert_eq!(win[511], 0.0);
    }

    #[test]
    fn test_fft_impulse() {
        let mut re = [0.0f32; 512];
        let mut im = [0.0f32; 512];
        re[0] = 1.0; // Unit impulse delta(t)
        fft_512(&mut re, &mut im);
        for k in 0..512 {
            assert!((re[k] - 1.0).abs() < 1e-4);
            assert!(im[k].abs() < 1e-4);
        }
    }

    #[test]
    fn test_feature_extraction_shape() {
        // Generate 1 second of silence/sine wave
        let mut samples = vec![0.0f32; 16000];
        for i in 0..16000 {
            samples[i] = (2.0 * PI * 440.0 * i as f32 / 16000.0).sin();
        }
        let (feats, flens) = extract_log_mel_features(&samples);
        assert_eq!(flens, 100);
        // total frames = 101, mel channels = 128
        assert_eq!(feats.len(), 128 * 101);
    }
}
