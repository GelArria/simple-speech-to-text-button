use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait as _, HostTrait as _};
use log::{error, info, warn};

use crate::audio;
use crate::config;
use crate::hotkey;
use crate::injector;
use crate::overlay;
use crate::stt;

const HOTKEY_ID: u32 = 1;

fn needs_space_before(text: &str) -> bool {
    text.chars().rev().take_while(|c| c.is_whitespace()).count() == 0
}

fn list_models() -> Vec<String> {
    let search_dirs = crate::config::model_search_dirs();
    for dir in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let models: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "bin"))
                .filter_map(|e| e.path().to_str().map(String::from))
                .collect();
            if !models.is_empty() {
                return models;
            }
        }
    }
    Vec::new()
}

fn model_size_mb(path: &str) -> f64 {
    std::fs::metadata(path)
        .map(|m| m.len() as f64 / (1024.0 * 1024.0))
        .unwrap_or(0.0)
}

fn resolve_model_path(configured: &str) -> String {
    if Path::new(configured).exists() {
        return configured.to_string();
    }
    let search_dirs = crate::config::model_search_dirs();
    let filename = Path::new(configured)
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or("");
    for dir in &search_dirs {
        let full = Path::new(dir).join(filename);
        if full.exists() {
            return full.to_string_lossy().to_string();
        }
    }
    configured.to_string()
}

fn select_model(configured: &str) -> Option<String> {
    let models = list_models();
    if models.is_empty() {
        error!("no .bin models found - download one first (simplestt config)");
        return None;
    }

    let resolved = resolve_model_path(configured);
    if Path::new(&resolved).exists() && models.len() == 1 {
        return Some(resolved);
    }

    let configured_exists = Path::new(&resolved).exists();

    println!("\n  Available models:");
    println!("  ─────────────────────────────────────────────────");
    for (i, m) in models.iter().enumerate() {
        let size = model_size_mb(m);
        let name = Path::new(m)
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("?");
        let marker = if m == &resolved || (!configured_exists && i == 0) {
            " (default)"
        } else {
            ""
        };
        println!("  [{}] {} ({:.0} MB){}", i + 1, name, size, marker);
    }
    println!("  ─────────────────────────────────────────────────");

    let default_idx = if configured_exists {
        models.iter().position(|m| m == &resolved).unwrap_or(0)
    } else {
        0
    };

    print!(
        "\n  Select model [1-{}] (Enter for {}): ",
        models.len(),
        default_idx + 1
    );
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    let input = input.trim();

    let idx = if input.is_empty() {
        default_idx
    } else {
        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= models.len() => n - 1,
            _ => {
                println!("  Invalid selection, using default.");
                default_idx
            }
        }
    };

    let selected = models[idx].clone();
    let name = Path::new(&selected)
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or("?");
    println!("  Selected: {}\n", name);
    Some(selected)
}

fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut devices: Vec<String> = Vec::new();
    if let Ok(inputs) = host.input_devices() {
        for d in inputs {
            if let Ok(name) = d.name() {
                if !audio::looks_like_loopback(&name) {
                    devices.push(name);
                }
            }
        }
    }
    devices
}

fn select_device(configured: &str) -> Option<String> {
    let devices = list_input_devices();
    if devices.is_empty() {
        error!("no input devices found");
        return None;
    }

    if devices.len() == 1 {
        println!("  Microphone: {}", devices[0]);
        return Some(devices[0].clone());
    }

    let default_device = cpal::default_host()
        .default_input_device()
        .and_then(|d| d.name().ok());

    println!("  Input devices:");
    println!("  ─────────────────────────────────────────────────");
    for (i, name) in devices.iter().enumerate() {
        let is_default = default_device.as_deref() == Some(name);
        let is_configured =
            name.to_lowercase().contains(&configured.to_lowercase()) && !configured.is_empty();
        let marker = if is_configured {
            " (configured)"
        } else if is_default {
            " (system default)"
        } else {
            ""
        };
        println!("  [{}] {}{}", i + 1, name, marker);
    }
    println!("  ─────────────────────────────────────────────────");

    let default_idx = if !configured.is_empty() {
        devices
            .iter()
            .position(|d| d.to_lowercase().contains(&configured.to_lowercase()))
            .unwrap_or_else(|| {
                devices
                    .iter()
                    .position(|d| default_device.as_deref() == Some(d))
                    .unwrap_or(0)
            })
    } else {
        devices
            .iter()
            .position(|d| default_device.as_deref() == Some(d))
            .unwrap_or(0)
    };

    print!(
        "\n  Select microphone [1-{}] (Enter for {}): ",
        devices.len(),
        default_idx + 1
    );
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    let input = input.trim();

    let idx = if input.is_empty() {
        default_idx
    } else {
        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= devices.len() => n - 1,
            _ => {
                println!("  Invalid selection, using default.");
                default_idx
            }
        }
    };

    println!("  Selected: {}\n", devices[idx]);
    Some(devices[idx].clone())
}

fn select_mic_preset(configured: &str) -> Option<config::MicPreset> {
    let presets = config::MicPreset::presets();
    println!("  Microphone preset:");
    println!("  ─────────────────────────────────────────────────");
    for (i, p) in presets.iter().enumerate() {
        let marker = if p.name.to_lowercase().contains(&configured.to_lowercase())
            && !configured.is_empty()
        {
            " (configured)"
        } else if i == 1 {
            " (recommended)"
        } else {
            ""
        };
        println!("  [{}] {}{}", i + 1, p.name, marker);
    }
    println!("  ─────────────────────────────────────────────────");

    let default_idx = if !configured.is_empty() {
        presets
            .iter()
            .position(|p| p.name.to_lowercase().contains(&configured.to_lowercase()))
            .unwrap_or(1)
    } else {
        1
    };

    print!(
        "\n  Select preset [1-{}] (Enter for {}): ",
        presets.len(),
        default_idx + 1
    );
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    let input = input.trim();

    let idx = if input.is_empty() {
        default_idx
    } else {
        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= presets.len() => n - 1,
            _ => {
                println!("  Invalid selection, using default.");
                default_idx
            }
        }
    };

    println!("  Selected: {}\n", presets[idx].name);
    Some(presets[idx].clone())
}

fn first_run_wizard() {
    println!();
    println!("  ╔══════════════════════════════════════════════════╗");
    println!("  ║  Let's configure a few things (Enter = default): ║");
    println!("  ║                                                  ║");
    println!("  ║  Press F9 to start/stop recording.               ║");
    println!("  ║  Speak naturally - pauses trigger transcription. ║");
    println!("  ║  Text is typed directly into your active app.    ║");
    println!("  ╚══════════════════════════════════════════════════╝");
    println!();
}

pub fn execute() {
    simplelog::SimpleLogger::init(simplelog::LevelFilter::Info, simplelog::Config::default())
        .expect("logger init failed");

    info!("simpleSTT starting...");

    let mut config = config::AppConfig::load();
    let is_first_run = config.first_run;

    if is_first_run {
        first_run_wizard();
    }

    let resolved_model = resolve_model_path(&config.stt.model_path);
    let needs_model_select = is_first_run || !Path::new(&resolved_model).exists();

    if needs_model_select {
        let model_path = match select_model(&resolved_model) {
            Some(p) => p,
            None => std::process::exit(1),
        };
        if model_path != config.stt.model_path {
            config.stt.model_path = model_path;
            if let Err(e) = config.save() {
                warn!("could not save config: {}", e);
            }
        }
    }

    if is_first_run {
        let device_name = match select_device(&config.audio.preferred_input_device) {
            Some(d) => d,
            None => std::process::exit(1),
        };
        if device_name != config.audio.preferred_input_device {
            config.audio.preferred_input_device = device_name;
            if let Err(e) = config.save() {
                warn!("could not save config: {}", e);
            }
        }
    }

    if is_first_run {
        let mic_preset = match select_mic_preset(&config.mic_preset.name) {
            Some(p) => p,
            None => std::process::exit(1),
        };
        config.mic_preset = mic_preset;
        config.stt.beam_size = config.mic_preset.beam_size;
        config.stt.patience = config.mic_preset.patience;
    }

    let stt = Arc::new(Mutex::new(None::<stt::SttEngine>));
    let recording = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));

    let model_to_load = resolve_model_path(&config.stt.model_path);

    match stt::SttEngine::new(
        &model_to_load,
        &config.stt.language,
        config.mic_preset.clone(),
        &config.timing,
    ) {
        Ok(engine) => {
            *stt.lock().unwrap() = Some(engine);
            info!("STT engine loaded");
        }
        Err(e) => {
            error!("failed to load STT engine: {}", e);
            std::process::exit(1);
        }
    }

    config.first_run = false;
    if let Err(e) = config.save() {
        warn!("could not save config: {}", e);
    }

    let stt_clone = stt.clone();
    let recording_clone = recording.clone();
    let running_clone = running.clone();
    let last_text = Arc::new(Mutex::new(String::new()));
    let last_text_clone = last_text.clone();
    let mic_only = config.audio.microphone_only;
    let preferred_input_device = if config.audio.preferred_input_device.trim().is_empty() {
        None
    } else {
        Some(config.audio.preferred_input_device.clone())
    };
    let worker_sleep_ms = config.audio.worker_sleep_ms.max(1);

    let worker = thread::spawn(move || {
        let mut audio_capture: Option<audio::AudioCapture> = None;
        let mut was_recording = false;

        while running_clone.load(Ordering::SeqCst) {
            let is_recording = recording_clone.load(Ordering::SeqCst);

            if is_recording && !was_recording {
                info!("recording started - speak naturally, I'll transcribe after each pause");
                match audio::AudioCapture::new(16000, mic_only, preferred_input_device.clone()) {
                    Ok(capture) => {
                        if let Some(eng) = stt_clone.lock().unwrap().as_mut() {
                            eng.reset();
                        }
                        audio_capture = Some(capture);
                    }
                    Err(e) => error!("failed to start audio: {}", e),
                }
            }

            if !is_recording && was_recording {
                info!("recording stopped");
                if let Some(eng) = stt_clone.lock().unwrap().as_mut() {
                    if let Some(text) = eng.flush() {
                        info!("final transcript: {:?}", text);
                        let mut to_inject = String::new();
                        {
                            let last = last_text_clone.lock().unwrap();
                            if needs_space_before(&last) && !text.starts_with(char::is_whitespace) {
                                to_inject.push(' ');
                            }
                        }
                        to_inject.push_str(&text);
                        if let Err(e) = injector::inject_text(&to_inject) {
                            error!("inject failed: {}", e);
                        }
                        *last_text_clone.lock().unwrap() = to_inject;
                    }
                }
                audio_capture = None;
            }

            if is_recording {
                if let Some(capture) = &audio_capture {
                    let mut buf = Vec::new();
                    let n = capture.read_samples(&mut buf);
                    if n > 0 {
                        if let Some(eng) = stt_clone.lock().unwrap().as_mut() {
                            if let Some(text) = eng.process_audio(&buf) {
                                info!("utterance: {:?}", text);
                                let mut to_inject = String::new();
                                {
                                    let last = last_text_clone.lock().unwrap();
                                    if needs_space_before(&last)
                                        && !text.starts_with(char::is_whitespace)
                                    {
                                        to_inject.push(' ');
                                    }
                                }
                                to_inject.push_str(&text);
                                if let Err(e) = injector::inject_text(&to_inject) {
                                    error!("inject failed: {}", e);
                                }
                                *last_text_clone.lock().unwrap() = to_inject;
                            }
                        }
                    }
                }
            }

            was_recording = is_recording;
            thread::sleep(Duration::from_millis(worker_sleep_ms));
        }
    });

    let _hotkey = match hotkey::GlobalHotkey::register(&config.hotkeys.start_stop, HOTKEY_ID) {
        Ok(h) => {
            info!("hotkey registered: {}", config.hotkeys.start_stop);
            Some(h)
        }
        Err(e) => {
            warn!(
                "failed to register hotkey '{}': {}",
                config.hotkeys.start_stop, e
            );
            None
        }
    };

    if let Err(e) = overlay::create_overlay(config.ui.opacity, config.ui.size, recording.clone()) {
        error!("failed to create overlay: {}", e);
        std::process::exit(1);
    }

    info!(
        "simpleSTT ready - press {} to toggle, speak naturally, pauses trigger transcription",
        config.hotkeys.start_stop
    );

    let result = overlay::run_message_loop();

    info!("shutting down...");
    recording.store(false, Ordering::SeqCst);
    running.store(false, Ordering::SeqCst);

    if let Some(mut hk) = _hotkey {
        hk.unregister();
    }

    let _ = worker.join();
    info!("audio worker stopped");

    info!("simpleSTT exited with code {}", result);
}
