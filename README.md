# simpleSTT (simple-speech-to-text) button

A single floating button that wraps [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (via [whisper-rs](https://github.com/tazz4843/whisper-rs)) to give you instant speech-to-text on Windows (Linux and Mac, especially for mac its not tested yet).

Click the button or press **F9** -> speak -> your words are typed wherever your cursor is. That's it.

No Electron, no frameworks, no bloat. ~3 MB binary and it can be GPU-accelerated primarily.

And dont worry, this thing is vibecoded. i just wanted a button that takes my voice to type stuff asap.

## What It Does

Press **F9** (or click the overlay button) to start recording. Speak naturally. When you pause, whisper.cpp transcribes the audio and the text is typed directly into whatever field is focused -- your browser, IDE, chat app, anything.

The overlay is a small draggable circle that sits on top of everything:
- **Gray** = idle
- **Red pulsing** = recording

## Requirements

### Windows

- **Windows 10/11** (x64)
- **Rust** (latest stable) -- [install](https://rustup.rs/)
- **Visual Studio Build Tools** with C++ workload
- **LLVM** (for bindgen) -- `winget install LLVM.LLVM`
- **CMake** -- `winget install Kitware.CMake`
- **NVIDIA GPU + CUDA Toolkit** (recommended) -- or use CPU/Vulkan

### Linux (Ubuntu/Debian)

```bash
sudo apt update
sudo apt install -y build-essential llvm-dev libclang-dev clang libasound2-dev pkg-config cmake
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**For CUDA (NVIDIA GPU):**
```bash
# Follow https://developer.nvidia.com/cuda-downloads for your distro
sudo apt install nvidia-cuda-toolkit
```

**For Vulkan (AMD/Intel GPU):**
```bash
sudo apt install libvulkan-dev mesa-vulkan-drivers
```

### macOS

```bash
xcode-select --install
brew install llvm cmake
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

> macOS uses Metal via CoreML -- no extra GPU setup needed. The `whisper-rs` crate handles it automatically.

> **Note:** The current overlay, global hotkey, and text injection modules are Windows-only (Win32 API). On Linux/macOS the core engine (audio capture + whisper transcription) compiles and works, but you'd need to implement platform-specific UI/injection. Contributions welcome.

## Quick Start

### 1. Clone and download a model

**Windows (PowerShell):**

```powershell
git clone https://github.com/GelArria/simpleSTT.git
cd simpleSTT
mkdir models
Invoke-WebRequest -Uri "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin" -OutFile "models\ggml-large-v3-turbo.bin"
```

**Linux / macOS:**

```bash
git clone https://github.com/GelArria/simpleSTT.git
cd simpleSTT
mkdir -p models
curl -L -o models/ggml-large-v3-turbo.bin "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin"
```

Models (best to worst accuracy):

| Model | Size | Accuracy | Notes |
|-------|------|----------|-------|
| `ggml-large-v3-turbo.bin` | ~800 MB | Best | Default, recommended |
| `ggml-medium.bin` | ~1.5 GB | Very good | Slower |
| `ggml-base.bin` | ~140 MB | Good | Fast |
| `ggml-tiny.bin` | ~75 MB | Basic | Quick testing |

### 2. Build and install

**Windows (PowerShell):**

```powershell
.\stt.ps1 install
```

This builds the binary (auto-detects CUDA/Vulkan/CPU) and installs it globally to your PATH.

**Or build manually:**

**With CUDA (NVIDIA GPU -- recommended):**
```bash
cargo build --release --features cuda
```

**CPU only:**
```bash
cargo build --release
```

**With Vulkan (AMD/Intel GPU):**
```bash
cargo build --release --features vulkan
```

### 3. Use

After `.\stt.ps1 install`, open a **new terminal** and use:

```powershell
simplestt              # Run overlay (default)
simplestt run          # Same as above
simplestt start        # Start in background
simplestt stop         # Stop running instance
simplestt restart      # Stop + start
simplestt status       # Show if running
simplestt config       # Interactive configuration menu
simplestt install      # Install globally to PATH
simplestt uninstall    # Uninstall from system
```

GPU flags for build (Windows only):
```powershell
.\stt.ps1 build -Cuda     # Force CUDA
.\stt.ps1 build -Vulkan   # Force Vulkan
.\stt.ps1 build -Cpu      # Force CPU-only
```

## Usage

| Action | How |
|--------|-----|
| Toggle recording | Press **F9** or click the overlay |
| Move overlay | Click and drag the circle |
| Context menu | Right-click the overlay |
| Quit | Right-click -> Quit |

## Configuration

### First-Run Wizard

On first launch, simpleSTT walks you through:

1. **Model selection** -- pick from installed `.bin` models in `models/`
2. **Microphone selection** -- choose your input device
3. **Microphone preset** -- choose a preset tuned for your setup

### Config File

Stored at `%APPDATA%\simplestt\simplestt\config\config.toml`:

```toml
[hotkeys]
start_stop = "F9"

[stt]
model_path = "models/ggml-base.bin"
language = "es"
beam_size = 5
patience = 1.0

[ui]
opacity = 220
size = 48

[audio]
microphone_only = true
preferred_input_device = ""
worker_sleep_ms = 10

[mic_preset]
name = "Headset / USB mic"
energy_threshold = 0.015
silence_frames_needed = 60
min_speech_samples = 8000
beam_size = 5
patience = 1.0
no_speech_thold = 0.6
entropy_thold = 2.4
```

### Settings Reference

| Setting | Default | Description |
|---------|---------|-------------|
| `hotkeys.start_stop` | `"F9"` | Global hotkey. Supports combos like `"Ctrl+F9"`, `"Alt+R"` |
| `stt.model_path` | Auto-detect | Path to whisper `.bin` model file |
| `stt.language` | `"es"` | Language code (`"es"`, `"en"`, `"fr"`, `"de"`, `"auto"`, etc.) |
| `stt.beam_size` | `5` | Beam search width (higher = more accurate, slower) |
| `stt.patience` | `1.0` | Beam search patience (0.0--2.0) |
| `ui.opacity` | `220` | Overlay opacity (0--255) |
| `ui.size` | `48` | Overlay circle size in pixels |
| `audio.microphone_only` | `true` | Skip loopback/system audio devices |
| `audio.preferred_input_device` | `""` | Partial name match for mic selection |
| `audio.worker_sleep_ms` | `10` | Audio polling interval in ms |

### Microphone Presets

| Preset | Best for | Energy threshold | Silence frames | Beam size |
|--------|----------|-----------------|----------------|-----------|
| Laptop built-in mic | Laptop mics, quiet room | 0.008 | 90 | 3 |
| Headset / USB mic | Headsets, USB mics | 0.015 | 60 | 5 |
| Studio / condenser mic | Professional mics | 0.025 | 50 | 7 |
| Noisy environment | Background noise | 0.035 | 100 | 5 |

The preset controls: `energy_threshold` (VAD sensitivity), `silence_frames_needed` (pause length before transcription), `min_speech_samples` (minimum audio to transcribe), `beam_size`, `patience`, `no_speech_thold`, and `entropy_thold`.

### Interactive Config Menu

```powershell
simplestt config
```

Opens an interactive menu to change model, microphone preset, language, hotkey, and timing without editing the TOML file directly. The model selector lists all 22 known whisper models with download-on-demand for uninstalled ones.

### Languages

Whisper supports 99 languages. Set `language` in config or via the config menu:

- `"es"` -- Spanish
- `"en"` -- English
- `"fr"` -- French
- `"de"` -- German
- `"auto"` -- Auto-detect (less accurate)
- [Full list](https://github.com/openai/whisper/blob/main/whisper/tokenizer.py)

## How It Works Under the Hood

```
[F9 / Click] -> cpal captures mic audio at 16 kHz
             -> VAD detects speech vs silence
             -> On pause (~0.7s silence), whisper.cpp transcribes via whisper-rs
             -> SendInput types the text into the focused window
```

```
src/
  main.rs              -- Entry point, CLI dispatch
  cli.rs               -- CLI definition (clap)
  commands/
    mod.rs             -- Command dispatcher
    run.rs             -- Overlay, worker thread, pipeline
    manage.rs          -- start/stop/restart/status
    config_menu.rs     -- Interactive configuration
    install.rs         -- Global install/uninstall
  stt.rs               -- Whisper engine (whisper-rs), VAD, transcription
  audio.rs             -- Mic capture + resampling (cpal + ringbuf)
  injector.rs          -- Text injection via SendInput (Unicode)
  hotkey.rs            -- Global hotkey registration (Win32)
  overlay.rs           -- Draggable overlay button (Win32 + GDI)
  config.rs            -- TOML config load/save, path resolution
```

## Building from Scratch (Troubleshooting)

### Windows (bindgen errors)

```powershell
$env:BINDGEN_EXTRA_CLANG_ARGS = "-I`"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt`" -I`"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\um`" -I`"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\shared`" -I`"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.xx.xxxxx\include`""
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:PATH"

cargo build --release --features cuda
```

### Linux (common issues)

```bash
# If clang not found:
export LIBCLANG_PATH=/usr/lib/llvm-17/lib  # adjust version as needed

# If ALSA errors:
sudo apt install libasound2-dev

# If linking errors with CUDA:
export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH
```

### macOS (common issues)

```bash
# If clang not found:
export LDFLAGS="-L$(brew --prefix llvm)/lib"
export CPPFLAGS="-I$(brew --prefix llvm)/include"
export PATH="$(brew --prefix llvm)/bin:$PATH"

# If SDK errors:
xcode-select --install
sudo xcodebuild -license accept
```

## License

MIT
