use std::fs;
use std::io::{self, Write};

use crate::config;

const KNOWN_MODELS: &[(&str, &str, &str)] = &[
    ("ggml-tiny.bin", "75 MB", "Tiny"),
    ("ggml-tiny.en.bin", "75 MB", "Tiny (English only)"),
    ("ggml-tiny-q5_0.bin", "31 MB", "Tiny Q5"),
    ("ggml-tiny.en-q5_0.bin", "31 MB", "Tiny Q5 (English only)"),
    ("ggml-base.bin", "148 MB", "Base"),
    ("ggml-base.en.bin", "148 MB", "Base (English only)"),
    ("ggml-base-q5_0.bin", "57 MB", "Base Q5"),
    ("ggml-base.en-q5_0.bin", "57 MB", "Base Q5 (English only)"),
    ("ggml-small.bin", "488 MB", "Small"),
    ("ggml-small.en.bin", "488 MB", "Small (English only)"),
    ("ggml-small-q5_0.bin", "181 MB", "Small Q5"),
    (
        "ggml-small.en-q5_0.bin",
        "181 MB",
        "Small Q5 (English only)",
    ),
    ("ggml-medium.bin", "1.5 GB", "Medium"),
    ("ggml-medium.en.bin", "1.5 GB", "Medium (English only)"),
    ("ggml-medium-q5_0.bin", "533 MB", "Medium Q5"),
    (
        "ggml-medium.en-q5_0.bin",
        "533 MB",
        "Medium Q5 (English only)",
    ),
    ("ggml-large-v1.bin", "3.1 GB", "Large v1"),
    ("ggml-large-v2.bin", "3.1 GB", "Large v2"),
    ("ggml-large-v3.bin", "3.1 GB", "Large v3"),
    ("ggml-large-v3-q5_0.bin", "1.1 GB", "Large v3 Q5"),
    ("ggml-large-v3-turbo.bin", "1.6 GB", "Large v3 Turbo"),
    (
        "ggml-large-v3-turbo-q5_0.bin",
        "536 MB",
        "Large v3 Turbo Q5",
    ),
];

fn models_dir() -> std::path::PathBuf {
    crate::config::first_models_dir().unwrap_or_else(|| "models".into())
}

fn model_installed(filename: &str) -> bool {
    let dir = models_dir();
    let full = dir.join(filename);
    full.exists()
}

fn download_model(filename: &str) -> Result<(), String> {
    let dir = models_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(filename);

    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        filename
    );

    println!("  Downloading {}...", filename);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;

    let mut response = client
        .get(&url)
        .send()
        .map_err(|e| format!("request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = fs::File::create(&dest).map_err(|e| format!("cannot create file: {}", e))?;

    use std::io::Read;
    let mut buffer = [0u8; 8192];

    loop {
        let n = response
            .read(&mut buffer)
            .map_err(|e| format!("read error: {}", e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])
            .map_err(|e| format!("write error: {}", e))?;
        downloaded += n as u64;

        if total_size > 0 {
            let pct = (downloaded as f64 / total_size as f64) * 100.0;
            let downloaded_mb = downloaded as f64 / (1024.0 * 1024.0);
            let total_mb = total_size as f64 / (1024.0 * 1024.0);
            print!(
                "\r  Progress: {:.0}% ({:.1} / {:.1} MB)",
                pct, downloaded_mb, total_mb
            );
            io::stdout().flush().ok();
        }
    }

    println!();
    println!("  Download complete.");
    Ok(())
}

fn read_line() -> String {
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    input.trim().to_string()
}

fn menu_model(config: &mut config::AppConfig) {
    loop {
        println!();
        println!("  Whisper models:");
        println!("  ─────────────────────────────────────────────────");
        for (i, (file, size, desc)) in KNOWN_MODELS.iter().enumerate() {
            let installed = model_installed(file);
            let selected = config.stt.model_path.ends_with(file);
            if selected {
                println!("  [{}] {} ({})  *", i + 1, desc, size);
            } else if installed {
                println!("  [{}] {} ({})  installed", i + 1, desc, size);
            } else {
                println!("  [{}] {} ({})", i + 1, desc, size);
            }
        }
        println!("  ─────────────────────────────────────────────────");
        println!("  [0] Back");
        print!("  Select model [1-{}]: ", KNOWN_MODELS.len());
        io::stdout().flush().ok();

        let sel = read_line();
        if sel == "0" || sel.is_empty() {
            break;
        }
        if let Ok(n) = sel.parse::<usize>() {
            if n >= 1 && n <= KNOWN_MODELS.len() {
                let (file, _size, desc) = KNOWN_MODELS[n - 1];
                if !model_installed(file) {
                    println!();
                    println!("  '{}' is not installed.", file);
                    println!("  [1] Download and install now");
                    println!("  [2] Go back");
                    print!("  Choose [1-2]: ");
                    io::stdout().flush().ok();
                    let dl = read_line();
                    if dl == "1" {
                        if let Err(e) = download_model(file) {
                            println!("  Download failed: {}", e);
                            println!("  Press Enter to go back...");
                            read_line();
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                config.stt.model_path =
                    models_dir().join(file).to_str().unwrap_or(file).to_string();
                println!("  Model set to: {}", desc);
                break;
            }
        }
        println!("  Invalid selection.");
    }
}

fn menu_preset(config: &mut config::AppConfig) {
    let presets = config::MicPreset::presets();
    println!();
    println!("  Microphone presets:");
    for (i, p) in presets.iter().enumerate() {
        let marker = if p.name == config.mic_preset.name {
            " (current)"
        } else if i == 1 {
            " (recommended)"
        } else {
            ""
        };
        println!("  [{}] {}{}", i + 1, p.name, marker);
    }
    print!("  Select preset [1-{}]: ", presets.len());
    io::stdout().flush().ok();

    let sel = read_line();
    if let Ok(n) = sel.parse::<usize>() {
        if n >= 1 && n <= presets.len() {
            config.mic_preset = presets[n - 1].clone();
            config.stt.beam_size = config.mic_preset.beam_size;
            config.stt.patience = config.mic_preset.patience;
            println!("  Preset set to: {}", presets[n - 1].name);
        } else {
            println!("  Invalid selection.");
        }
    }
}

fn menu_language(config: &mut config::AppConfig) {
    println!();
    println!("  Languages: es, en, fr, de, it, pt, ja, ko, zh, auto");
    print!("  Language (current: {}): ", config.stt.language);
    io::stdout().flush().ok();

    let lang = read_line();
    if !lang.is_empty() && lang != config.stt.language {
        config.stt.language = lang;
        println!("  Language set to: {}", config.stt.language);
    }
}

fn menu_hotkey(config: &mut config::AppConfig) {
    println!();
    println!("  Examples: F9, F10, Ctrl+F12, Alt+R");
    print!("  Hotkey (current: {}): ", config.hotkeys.start_stop);
    io::stdout().flush().ok();

    let hk = read_line();
    if !hk.is_empty() && hk != config.hotkeys.start_stop {
        config.hotkeys.start_stop = hk;
        println!("  Hotkey set to: {}", config.hotkeys.start_stop);
    }
}

fn menu_timing(config: &mut config::AppConfig) {
    println!();
    println!("  Timing configuration (milliseconds / seconds)");
    println!("  Lower silence = faster response but may cut words on pauses");
    println!();

    print!(
        "  Silence timeout ms (current: {}, default: 350): ",
        config.timing.silence_timeout_ms
    );
    io::stdout().flush().ok();
    let val = read_line();
    if let Ok(n) = val.parse::<u64>() {
        config.timing.silence_timeout_ms = n;
    }

    print!(
        "  Min speech ms (current: {}, default: 250): ",
        config.timing.min_speech_ms
    );
    io::stdout().flush().ok();
    let val = read_line();
    if let Ok(n) = val.parse::<u64>() {
        config.timing.min_speech_ms = n;
    }

    print!(
        "  Max utterance secs (current: {}, default: 30): ",
        config.timing.max_utterance_secs
    );
    io::stdout().flush().ok();
    let val = read_line();
    if let Ok(n) = val.parse::<u64>() {
        config.timing.max_utterance_secs = n;
    }

    println!("  Timing updated.");
}

pub fn execute() {
    let mut config = config::AppConfig::load();
    let mut changed = false;

    loop {
        println!();
        println!("  simpleSTT Configuration");
        println!("  ─────────────────────────────────────────────────");
        println!("  Model:    {}", config.stt.model_path);
        println!("  Preset:   {}", config.mic_preset.name);
        println!("  Language: {}", config.stt.language);
        println!("  Hotkey:   {}", config.hotkeys.start_stop);
        println!(
            "  Silence:  {}ms  Min speech: {}ms  Max utterance: {}s",
            config.timing.silence_timeout_ms,
            config.timing.min_speech_ms,
            config.timing.max_utterance_secs
        );
        if changed {
            println!("  ─────────────────────────────────────────────────");
            println!("  * Unsaved changes");
        }
        println!("  ─────────────────────────────────────────────────");
        println!();
        println!("  [1] Model");
        println!("  [2] Microphone preset");
        println!("  [3] Language");
        println!("  [4] Hotkey");
        println!("  [5] Silence & timing");
        println!("  [6] Save & exit");
        println!("  [0] Exit without saving");
        println!();
        print!("  Select [0-6]: ");
        io::stdout().flush().ok();

        let choice = read_line();
        match choice.as_str() {
            "1" => {
                menu_model(&mut config);
                changed = true;
            }
            "2" => {
                menu_preset(&mut config);
                changed = true;
            }
            "3" => {
                menu_language(&mut config);
                changed = true;
            }
            "4" => {
                menu_hotkey(&mut config);
                changed = true;
            }
            "5" => {
                menu_timing(&mut config);
                changed = true;
            }
            "6" => {
                if changed {
                    if let Err(e) = config.save() {
                        eprintln!("  Error saving config: {}", e);
                        std::process::exit(1);
                    }
                    println!();
                    println!("  simpleSTT Configuration");
                    println!("  ─────────────────────────────────────────────────");
                    println!("  Model:    {}", config.stt.model_path);
                    println!("  Preset:   {}", config.mic_preset.name);
                    println!("  Language: {}", config.stt.language);
                    println!("  Hotkey:   {}", config.hotkeys.start_stop);
                    println!("  ─────────────────────────────────────────────────");
                    println!();
                    println!("  Config saved.");
                    println!();
                }
                return;
            }
            "0" => {
                if changed {
                    println!();
                    println!("  Changes not saved.");
                }
                return;
            }
            _ => return,
        }
    }
}
