use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub hotkeys: HotkeyConfig,
    pub stt: SttConfig,
    pub ui: UiConfig,
    pub audio: AudioConfig,
    pub mic_preset: MicPreset,
    pub timing: TimingConfig,
    pub first_run: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct TimingConfig {
    pub silence_timeout_ms: u64,
    pub min_speech_ms: u64,
    pub max_utterance_secs: u64,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            silence_timeout_ms: 350,
            min_speech_ms: 250,
            max_utterance_secs: 30,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    pub start_stop: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SttConfig {
    pub model_path: String,
    pub language: String,
    pub beam_size: i32,
    pub patience: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub opacity: u8,
    pub size: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub microphone_only: bool,
    pub preferred_input_device: String,
    pub worker_sleep_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct MicPreset {
    pub name: String,
    pub energy_threshold: f32,
    pub silence_frames_needed: usize,
    pub min_speech_samples: usize,
    pub beam_size: i32,
    pub patience: f32,
    pub no_speech_thold: f32,
    pub entropy_thold: f32,
}

impl MicPreset {
    pub fn presets() -> Vec<Self> {
        vec![
            Self {
                name: "Laptop built-in mic".to_string(),
                energy_threshold: 0.008,
                silence_frames_needed: 90,
                min_speech_samples: 16000,
                beam_size: 3,
                patience: 0.8,
                no_speech_thold: 0.5,
                entropy_thold: 2.0,
            },
            Self {
                name: "Headset / USB mic".to_string(),
                energy_threshold: 0.015,
                silence_frames_needed: 60,
                min_speech_samples: 8000,
                beam_size: 5,
                patience: 1.0,
                no_speech_thold: 0.6,
                entropy_thold: 2.4,
            },
            Self {
                name: "Studio / condenser mic".to_string(),
                energy_threshold: 0.025,
                silence_frames_needed: 50,
                min_speech_samples: 8000,
                beam_size: 7,
                patience: 1.2,
                no_speech_thold: 0.6,
                entropy_thold: 2.4,
            },
            Self {
                name: "Noisy environment".to_string(),
                energy_threshold: 0.035,
                silence_frames_needed: 100,
                min_speech_samples: 16000,
                beam_size: 5,
                patience: 1.0,
                no_speech_thold: 0.4,
                entropy_thold: 1.8,
            },
        ]
    }
}

impl Default for MicPreset {
    fn default() -> Self {
        Self::presets()[1].clone()
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            start_stop: "F9".to_string(),
        }
    }
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            model_path: std::fs::read_dir("models")
                .ok()
                .and_then(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().is_some_and(|ext| ext == "bin"))
                        .filter_map(|e| e.path().to_str().map(String::from))
                        .next()
                })
                .unwrap_or_else(|| "models/ggml-large-v3-turbo.bin".to_string()),
            language: "es".to_string(),
            beam_size: 5,
            patience: 1.0,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            opacity: 220,
            size: 48,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            microphone_only: true,
            preferred_input_device: String::new(),
            worker_sleep_ms: 2,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkeys: HotkeyConfig::default(),
            stt: SttConfig::default(),
            ui: UiConfig::default(),
            audio: AudioConfig::default(),
            mic_preset: MicPreset::default(),
            timing: TimingConfig::default(),
            first_run: true,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "simplestt", "simplestt")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

impl AppConfig {
    pub fn load() -> Self {
        let path = match config_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        if !path.exists() {
            let default = Self::default();
            let _ = default.save();
            return default;
        }

        let contents = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        toml::from_str(&contents).unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let path = config_path().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not determine config directory",
            )
        })?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        fs::write(&path, contents)
    }
}
