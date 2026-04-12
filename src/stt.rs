use crate::config::MicPreset;
use log::{error, info};
use std::time::Instant;

struct AccelerationProbe {
    use_gpu: bool,
}

fn detect_acceleration() -> AccelerationProbe {
    let cuda_driver = std::path::Path::new("C:\\Windows\\System32\\nvcuda.dll").exists();
    let vulkan_driver = std::path::Path::new("C:\\Windows\\System32\\vulkan-1.dll").exists();

    let mut build_tags = vec!["OpenMP"];
    if cfg!(feature = "cuda") {
        build_tags.push("CUDA");
    }
    if cfg!(feature = "vulkan") {
        build_tags.push("Vulkan");
    }

    let gpu_built = cfg!(feature = "cuda") || cfg!(feature = "vulkan");
    let gpu_driver =
        (cfg!(feature = "cuda") && cuda_driver) || (cfg!(feature = "vulkan") && vulkan_driver);
    let use_gpu = gpu_built && gpu_driver;

    info!(
        "build features: {} | drivers: CUDA={}, Vulkan={}",
        build_tags.join(", "),
        cuda_driver,
        vulkan_driver,
    );

    if gpu_built && gpu_driver {
        info!(
            "GPU acceleration: ENABLED ({})",
            if cfg!(feature = "cuda") {
                "CUDA"
            } else {
                "Vulkan"
            }
        );
    } else if gpu_built && !gpu_driver {
        info!(
            "GPU acceleration: DISABLED (built with {} but driver not found)",
            if cfg!(feature = "cuda") {
                "CUDA"
            } else {
                "Vulkan"
            }
        );
    } else {
        info!("GPU acceleration: DISABLED (CPU-only build)");
    }

    AccelerationProbe { use_gpu }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VadState {
    Idle,
    Speech,
    SilenceAfterSpeech,
}

const HALLUCINATIONS: &[&str] = &[
    "gracias",
    "gracias.",
    "gracias,",
    "gracias punto",
    "thank you",
    "thank you.",
    "thanks",
    "thanks.",
    "subs",
    "subs.",
    "subscribe",
    "subscribe.",
    "please",
    "please.",
    "ok",
    "ok.",
    "okay",
    "okay.",
    "punto",
    "punto.",
    "punto,",
    "si",
    "si.",
    "no",
    "no.",
    "bien",
    "bien.",
    "sí",
    "sí.",
    "yes",
    "yes.",
    "no.",
    "the",
    "the.",
    "a",
    "i",
    "um",
    "uh",
    "hmm",
    "mhm",
];

fn is_hallucination(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    let cleaned = lower
        .trim_end_matches(|c: char| c == '.' || c == ',' || c == '!' || c == '?')
        .trim();

    if HALLUCINATIONS.contains(&lower.as_str()) || HALLUCINATIONS.contains(&cleaned) {
        return true;
    }

    let word_count = cleaned.split_whitespace().count();
    if word_count <= 2 && cleaned.chars().count() <= 15 {
        for word in cleaned.split_whitespace() {
            if HALLUCINATIONS.contains(&word) {
                return true;
            }
        }
    }

    false
}

pub struct SttEngine {
    _ctx: whisper_rs::WhisperContext,
    state: whisper_rs::WhisperState,
    buffer: Vec<f32>,
    language: String,
    n_threads: i32,
    beam_size: i32,
    patience: f32,
    vad_state: VadState,
    energy_threshold: f32,
    silence_frames_needed: usize,
    silence_frames_count: usize,
    min_speech_samples: usize,
    max_utterance_samples: usize,
    no_speech_thold: f32,
    entropy_thold: f32,
}

impl SttEngine {
    pub fn new(model_path: &str, language: &str, preset: MicPreset) -> Result<Self, String> {
        info!("loading whisper model from {}", model_path);
        let accel = detect_acceleration();
        let mut params = whisper_rs::WhisperContextParameters::default();
        params.use_gpu(accel.use_gpu);
        let ctx = whisper_rs::WhisperContext::new_with_params(model_path, params)
            .map_err(|e: whisper_rs::WhisperError| format!("{}", e))?;
        let state = ctx
            .create_state()
            .map_err(|e: whisper_rs::WhisperError| format!("{}", e))?;

        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get().min(8) as i32)
            .unwrap_or(4)
            .max(1);

        info!(
            "engine config: lang={}, threads={}, beam_size={}, patience={:.2}, gpu={}",
            language,
            n_threads,
            preset.beam_size.max(1),
            preset.patience,
            accel.use_gpu
        );

        Ok(Self {
            _ctx: ctx,
            state,
            buffer: Vec::new(),
            language: language.to_string(),
            n_threads,
            beam_size: preset.beam_size.max(1),
            patience: if preset.patience <= 0.0 {
                1.0
            } else {
                preset.patience
            },
            vad_state: VadState::Idle,
            energy_threshold: preset.energy_threshold,
            silence_frames_needed: preset.silence_frames_needed,
            silence_frames_count: 0,
            min_speech_samples: preset.min_speech_samples,
            max_utterance_samples: 16000 * 30,
            no_speech_thold: preset.no_speech_thold,
            entropy_thold: preset.entropy_thold,
        })
    }

    fn rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = samples.iter().map(|s| (*s as f64) * (*s as f64)).sum();
        (sum / samples.len() as f64).sqrt() as f32
    }

    pub fn process_audio(&mut self, samples: &[f32]) -> Option<String> {
        let energy = Self::rms(samples);

        let prev_state = self.vad_state;

        match self.vad_state {
            VadState::Idle => {
                if energy > self.energy_threshold {
                    self.vad_state = VadState::Speech;
                    self.silence_frames_count = 0;
                    self.buffer.extend_from_slice(samples);
                    if prev_state != self.vad_state {
                        info!("VAD: speech detected");
                    }
                }
            }
            VadState::Speech => {
                self.buffer.extend_from_slice(samples);
                if energy < self.energy_threshold {
                    self.silence_frames_count += 1;
                } else {
                    self.silence_frames_count = 0;
                }

                if self.silence_frames_count >= self.silence_frames_needed {
                    self.vad_state = VadState::SilenceAfterSpeech;
                }

                if self.buffer.len() >= self.max_utterance_samples {
                    self.vad_state = VadState::SilenceAfterSpeech;
                }
            }
            VadState::SilenceAfterSpeech => {}
        }

        if self.vad_state == VadState::SilenceAfterSpeech {
            let result = self.transcribe_buffer();
            self.vad_state = VadState::Idle;
            return result;
        }

        None
    }

    fn transcribe_buffer(&mut self) -> Option<String> {
        if self.buffer.len() < self.min_speech_samples {
            info!(
                "VAD: utterance too short ({} samples, need {}), skipping",
                self.buffer.len(),
                self.min_speech_samples
            );
            self.buffer.clear();
            return None;
        }

        let samples = self.buffer.clone();
        self.buffer.clear();

        let started = Instant::now();
        let strategy = whisper_rs::SamplingStrategy::BeamSearch {
            beam_size: self.beam_size,
            patience: self.patience,
        };

        let mut params = whisper_rs::FullParams::new(strategy);
        params.set_n_threads(self.n_threads);
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_no_timestamps(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_temperature(0.0);
        params.set_temperature_inc(0.2);
        params.set_single_segment(false);
        params.set_no_speech_thold(self.no_speech_thold);
        params.set_entropy_thold(self.entropy_thold);

        if self.language != "auto" {
            params.set_language(Some(&self.language));
            params.set_detect_language(false);
        } else {
            params.set_language(None);
            params.set_detect_language(true);
        }

        if let Err(e) = self.state.full(params, &samples) {
            error!("transcription failed: {}", e);
            return None;
        }

        let n = self.state.full_n_segments();
        let mut text = String::new();
        for i in 0..n {
            if let Some(seg) = self.state.get_segment(i) {
                if let Ok(s) = seg.to_str() {
                    let t = s.trim();
                    if !t.is_empty() {
                        if !text.is_empty() {
                            text.push(' ');
                        }
                        text.push_str(t);
                    }
                }
            }
        }

        let elapsed = started.elapsed().as_millis();
        info!(
            "transcribed {} samples ({:.1}s) in {}ms -> {:?}",
            samples.len(),
            samples.len() as f32 / 16000.0,
            elapsed,
            text
        );

        let trimmed = text.trim();
        if trimmed.is_empty() {
            info!("empty transcription, skipping");
            return None;
        }

        if is_hallucination(trimmed) {
            info!("hallucination detected: {:?}, skipping", trimmed);
            return None;
        }

        Some(text)
    }

    pub fn flush(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            return None;
        }
        if self.buffer.len() < self.min_speech_samples {
            info!(
                "flush: buffer too short ({} samples), skipping",
                self.buffer.len()
            );
            self.buffer.clear();
            return None;
        }
        info!(
            "flush: transcribing remaining buffer ({} samples)",
            self.buffer.len()
        );
        self.transcribe_buffer()
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.vad_state = VadState::Idle;
        self.silence_frames_count = 0;
    }
}
