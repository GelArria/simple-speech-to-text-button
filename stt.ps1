param(
    [Parameter(Position=0)]
    [ValidateSet("install", "uninstall", "start", "stop", "restart", "status", "run", "config")]
    [string]$Action,

    [switch]$Cuda,
    [switch]$Vulkan,
    [switch]$Cpu
)

$ErrorActionPreference = "Stop"
$ExeName = "simplestt.exe"
$ExePath = "$PSScriptRoot\target\release\$ExeName"
$StartMenu = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs"
$LnkPath = "$StartMenu\simpleSTT.lnk"

function Resolve-BuildFeatures {
    if ($Cpu) {
        Write-Host "  Backend: CPU (forced)" -ForegroundColor DarkGray
        return @()
    }
    if ($Cuda) {
        Write-Host "  Backend: CUDA (forced)" -ForegroundColor DarkGray
        return @("cuda")
    }
    if ($Vulkan) {
        Write-Host "  Backend: Vulkan (forced)" -ForegroundColor DarkGray
        return @("vulkan")
    }

    $hasNvidia = Test-Path "C:\Windows\System32\nvcuda.dll"
    $hasVulkan = Test-Path "C:\Windows\System32\vulkan-1.dll"

    if ($hasNvidia) {
        Write-Host "  Backend: CUDA (auto-detected nvcuda.dll)" -ForegroundColor Green
        return @("cuda")
    }
    if ($hasVulkan) {
        Write-Host "  Backend: Vulkan (auto-detected vulkan-1.dll)" -ForegroundColor Green
        return @("vulkan")
    }

    Write-Host "  Backend: CPU (no GPU driver detected)" -ForegroundColor DarkGray
    return @()
}

function Build-Exe {
    $ucrt = "${env:ProgramFiles(x86)}\Windows Kits\10\Include\10.0.26100.0\ucrt"
    $um   = "${env:ProgramFiles(x86)}\Windows Kits\10\Include\10.0.26100.0\um"
    $shared = "${env:ProgramFiles(x86)}\Windows Kits\10\Include\10.0.26100.0\shared"
    $vsEdition = @("Community","BuildTools","Professional","Enterprise") | ForEach-Object { "C:\Program Files\Microsoft Visual Studio\2022\$_\VC\Tools\MSVC" } | Where-Object { Test-Path $_ } | Select-Object -First 1
    $msvc = Get-ChildItem $vsEdition -Directory | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
    $msvcInclude = Join-Path $msvc "include"

    $env:BINDGEN_EXTRA_CLANG_ARGS = "-I`"$ucrt`" -I`"$um`" -I`"$shared`" -I`"$msvcInclude`""
    $env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;C:\Program Files\CMake\bin;$env:PATH"
    $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"

    $features = Resolve-BuildFeatures
    $featuresArg = if ($features.Count -gt 0) { "--features $($features -join ',')" } else { "" }

    $label = if ($features.Count -gt 0) { " ($($features -join ', '))" } else { " (CPU)" }
    Write-Host "Building...$label" -ForegroundColor Yellow

    $buildOutput = & cmd /c "cargo build --release --manifest-path `"$PSScriptRoot\Cargo.toml`" $featuresArg 2>&1"
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        Write-Host ($buildOutput | Out-String) -ForegroundColor Red
        throw "Build failed"
    }

    $warnings = ($buildOutput | Where-Object { $_ -match "warning:" }) | Measure-Object | Select-Object -ExpandProperty Count
    Write-Host "Build succeeded.$label" -ForegroundColor Green
    if ($warnings -gt 0) {
        Write-Host "  ($warnings warning(s))" -ForegroundColor DarkGray
    }
    if (-not (Test-Path $ExePath)) { throw "Build failed - no executable produced" }

    $exeBytes = [System.IO.File]::ReadAllBytes($ExePath)
    $exeText = [System.Text.Encoding]::ASCII.GetString($exeBytes)
    $hasCudaSym = $exeText -match "nvcuda"
    $hasVulkanSym = $exeText -match "vulkan"

    if ($features -contains "cuda" -and -not $hasCudaSym) {
        Write-Host "  WARNING: CUDA feature requested but binary does not reference nvcuda" -ForegroundColor Yellow
    } elseif ($features -contains "vulkan" -and -not $hasVulkanSym) {
        Write-Host "  WARNING: Vulkan feature requested but binary does not reference vulkan" -ForegroundColor Yellow
    }

    $gpuTag = if ($hasCudaSym) { "CUDA" } elseif ($hasVulkanSym) { "Vulkan" } else { "CPU-only" }
    $size = [math]::Round((Get-Item $ExePath).Length / 1MB, 1)
    Write-Host "  Output: $ExePath ($size MB, $gpuTag)" -ForegroundColor DarkGray
}

function Get-ProcessStt { Get-Process -Name "simplestt" -ErrorAction SilentlyContinue }

switch ($Action) {
    "install" {
        Build-Exe
        $WshShell = New-Object -ComObject WScript.Shell
        $Shortcut = $WshShell.CreateShortcut($LnkPath)
        $Shortcut.TargetPath = $ExePath
        $Shortcut.WorkingDirectory = Split-Path $ExePath
        $Shortcut.Description = "simpleSTT - Speech to Text"
        $Shortcut.Save()
        Write-Host "Installed. Shortcut in Start Menu. Auto-starts on login if you copy the shortcut to Startup." -ForegroundColor Green
        Write-Host "  Startup folder: $env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup" -ForegroundColor Cyan
    }

    "uninstall" {
        Get-ProcessStt | Stop-Process -Force
        Start-Sleep -Milliseconds 500

        if (Test-Path $LnkPath) { Remove-Item $LnkPath -Force; Write-Host "  Removed Start Menu shortcut" -ForegroundColor DarkGray }
        $startupLnk = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup\simpleSTT.lnk"
        if (Test-Path $startupLnk) { Remove-Item $startupLnk -Force; Write-Host "  Removed Startup shortcut" -ForegroundColor DarkGray }

        $configDir = "$env:APPDATA\simplestt"
        if (Test-Path $configDir) { Remove-Item $configDir -Recurse -Force; Write-Host "  Removed config ($configDir)" -ForegroundColor DarkGray }

        if (Test-Path $ExePath) { Remove-Item $ExePath -Force; Write-Host "  Removed executable" -ForegroundColor DarkGray }

        Write-Host ""
        Write-Host "  Full uninstall complete." -ForegroundColor Green
        Write-Host "  Source code and models folder left untouched." -ForegroundColor DarkGray
        Write-Host "  To also remove build artifacts:  Remove-Item target -Recurse -Force" -ForegroundColor DarkGray
    }

    "start" {
        Build-Exe
        if (Get-ProcessStt) { Write-Host "Already running (PID $((Get-ProcessStt).Id))." -ForegroundColor Yellow; return }
        Start-Process $ExePath -WindowStyle Hidden
        Start-Sleep -Milliseconds 500
        if (Get-ProcessStt) {
            Write-Host "Started (PID $((Get-ProcessStt).Id)). Press F9 to toggle recording." -ForegroundColor Green
        } else {
            Write-Host "Failed to start. Run manually: $ExePath" -ForegroundColor Red
        }
    }

    "stop" {
        $proc = Get-ProcessStt
        if ($proc) {
            $proc | Stop-Process -Force
            Write-Host "Stopped." -ForegroundColor Green
        } else {
            Write-Host "Not running." -ForegroundColor Yellow
        }
    }

    "restart" {
        Get-ProcessStt | Stop-Process -Force
        Start-Sleep -Milliseconds 500
        Start-Process $ExePath -WindowStyle Hidden
        Start-Sleep -Milliseconds 500
        if (Get-ProcessStt) {
            Write-Host "Restarted (PID $((Get-ProcessStt).Id))." -ForegroundColor Green
        }
    }

    "run" {
        Build-Exe
        & $ExePath
    }

    "status" {
        $proc = Get-ProcessStt
        if ($proc) {
            Write-Host "Running (PID $($proc.Id), $([math]::Round($proc.WorkingSet64/1MB,1)) MB)" -ForegroundColor Green
        } else {
            Write-Host "Not running." -ForegroundColor Yellow
        }
    }

    "config" {
        $configDir = "$env:APPDATA\simplestt\simplestt\config"
        $configFile = "$configDir\config.toml"
        if (-not (Test-Path $configDir)) { New-Item -ItemType Directory -Path $configDir -Force | Out-Null }
        if (-not (Test-Path $configFile)) {
            $defaultContent = @"
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
worker_sleep_ms = 2

[timing]
silence_timeout_ms = 350
min_speech_ms = 250
max_utterance_secs = 30

[mic_preset]
name = "Headset / USB mic"
energy_threshold = 0.015
silence_frames_needed = 60
min_speech_samples = 8000
beam_size = 5
patience = 1.0
no_speech_thold = 0.6
entropy_thold = 2.4
"@
            Set-Content -Path $configFile -Value $defaultContent -Encoding UTF8
            Write-Host "Created default config." -ForegroundColor Green
        }

        $content = Get-Content $configFile -Raw

        $pModel = '(?m)^model_path\s*=\s*"(.+?)"'
        $currentModel = if ($content -match $pModel) { $Matches[1] } else { "not set" }
        $pName = '(?m)^name\s*=\s*"(.+?)"'
        $currentPreset = if ($content -match $pName) { $Matches[1] } else { "not set" }
        $pLang = '(?m)^language\s*=\s*"(.+?)"'
        $currentLang = if ($content -match $pLang) { $Matches[1] } else { "es" }
        $pHotkey = '(?m)^start_stop\s*=\s*"(.+?)"'
        $currentHotkey = if ($content -match $pHotkey) { $Matches[1] } else { "F9" }

        $pSilence = '(?m)^silence_timeout_ms\s*=\s*(\d+)'
        $currentSilence = if ($content -match $pSilence) { $Matches[1] } else { "350" }
        $pMinSpeech = '(?m)^min_speech_ms\s*=\s*(\d+)'
        $currentMinSpeech = if ($content -match $pMinSpeech) { $Matches[1] } else { "250" }
        $pMaxUtt = '(?m)^max_utterance_secs\s*=\s*(\d+)'
        $currentMaxUtt = if ($content -match $pMaxUtt) { $Matches[1] } else { "30" }

        $changed = $false

        while ($true) {
            Clear-Host
            Write-Host ""
            Write-Host "  simpleSTT Configuration" -ForegroundColor Cyan
            Write-Host "  -----------------------------------------------" -ForegroundColor DarkGray
            Write-Host "  Model:    $currentModel" -ForegroundColor White
            Write-Host "  Preset:   $currentPreset" -ForegroundColor White
            Write-Host "  Language: $currentLang" -ForegroundColor White
            Write-Host "  Hotkey:   $currentHotkey" -ForegroundColor White
            Write-Host "  Silence:  ${currentSilence}ms  Min speech: ${currentMinSpeech}ms  Max utterance: ${currentMaxUtt}s" -ForegroundColor White
            if ($changed) {
                Write-Host "  -----------------------------------------------" -ForegroundColor DarkGray
                Write-Host "  * Unsaved changes" -ForegroundColor Yellow
            }
            Write-Host "  -----------------------------------------------" -ForegroundColor DarkGray
            Write-Host ""
            Write-Host "  [1] Model"
            Write-Host "  [2] Microphone preset"
            Write-Host "  [3] Language"
            Write-Host "  [4] Hotkey"
            Write-Host "  [5] Silence & timing"
            Write-Host "  [6] Save & exit"
            Write-Host "  [0] Exit without saving"
            Write-Host ""
            $choice = Read-Host "  Select [0-6]"

            switch ($choice) {
                "1" {
                    $knownModels = @(
                        @{ File = "ggml-tiny.bin"; Size = "75 MB"; Desc = "Tiny" },
                        @{ File = "ggml-tiny.en.bin"; Size = "75 MB"; Desc = "Tiny (English only)" },
                        @{ File = "ggml-tiny-q5_0.bin"; Size = "31 MB"; Desc = "Tiny Q5" },
                        @{ File = "ggml-tiny.en-q5_0.bin"; Size = "31 MB"; Desc = "Tiny Q5 (English only)" },
                        @{ File = "ggml-base.bin"; Size = "148 MB"; Desc = "Base" },
                        @{ File = "ggml-base.en.bin"; Size = "148 MB"; Desc = "Base (English only)" },
                        @{ File = "ggml-base-q5_0.bin"; Size = "57 MB"; Desc = "Base Q5" },
                        @{ File = "ggml-base.en-q5_0.bin"; Size = "57 MB"; Desc = "Base Q5 (English only)" },
                        @{ File = "ggml-small.bin"; Size = "488 MB"; Desc = "Small" },
                        @{ File = "ggml-small.en.bin"; Size = "488 MB"; Desc = "Small (English only)" },
                        @{ File = "ggml-small-q5_0.bin"; Size = "181 MB"; Desc = "Small Q5" },
                        @{ File = "ggml-small.en-q5_0.bin"; Size = "181 MB"; Desc = "Small Q5 (English only)" },
                        @{ File = "ggml-medium.bin"; Size = "1.5 GB"; Desc = "Medium" },
                        @{ File = "ggml-medium.en.bin"; Size = "1.5 GB"; Desc = "Medium (English only)" },
                        @{ File = "ggml-medium-q5_0.bin"; Size = "533 MB"; Desc = "Medium Q5" },
                        @{ File = "ggml-medium.en-q5_0.bin"; Size = "533 MB"; Desc = "Medium Q5 (English only)" },
                        @{ File = "ggml-large-v1.bin"; Size = "3.1 GB"; Desc = "Large v1" },
                        @{ File = "ggml-large-v2.bin"; Size = "3.1 GB"; Desc = "Large v2" },
                        @{ File = "ggml-large-v3.bin"; Size = "3.1 GB"; Desc = "Large v3" },
                        @{ File = "ggml-large-v3-q5_0.bin"; Size = "1.1 GB"; Desc = "Large v3 Q5" },
                        @{ File = "ggml-large-v3-turbo.bin"; Size = "1.6 GB"; Desc = "Large v3 Turbo" },
                        @{ File = "ggml-large-v3-turbo-q5_0.bin"; Size = "536 MB"; Desc = "Large v3 Turbo Q5" }
                    )
                    $modelSelected = $false
                    while (-not $modelSelected) {
                        Clear-Host
                        Write-Host ""
                        Write-Host "  Whisper models:" -ForegroundColor Cyan
                        Write-Host "  -----------------------------------------------" -ForegroundColor DarkGray
                        for ($i = 0; $i -lt $knownModels.Count; $i++) {
                            $m = $knownModels[$i]
                            $installed = Test-Path "$PSScriptRoot\models\$($m.File)"
                            $isSelected = "models/$($m.File)" -eq $currentModel
                            if ($isSelected) {
                                Write-Host "  [$($i+1)] $($m.Desc) ($($m.Size))  *" -ForegroundColor Yellow -BackgroundColor DarkGray
                            } elseif ($installed) {
                                Write-Host "  [$($i+1)] $($m.Desc) ($($m.Size))  installed" -ForegroundColor White
                            } else {
                                Write-Host "  [$($i+1)] $($m.Desc) ($($m.Size))" -ForegroundColor DarkGray
                            }
                        }
                        Write-Host "  -----------------------------------------------" -ForegroundColor DarkGray
                        Write-Host "  [0] Back to main menu" -ForegroundColor DarkGray
                        $sel = Read-Host "  Select model [1-$($knownModels.Count)]"
                        if ($sel -eq "0" -or $sel -eq "") {
                            break
                        }
                        if ($sel -match '^\d+$' -and [int]$sel -ge 1 -and [int]$sel -le $knownModels.Count) {
                            $chosen = $knownModels[[int]$sel - 1]
                            $chosenPath = "models/$($chosen.File)"
                            if (-not (Test-Path "$PSScriptRoot\models\$($chosen.File)")) {
                                Write-Host ""
                                Write-Host "  Model '$($chosen.File)' is not installed." -ForegroundColor Yellow
                                Write-Host ""
                                Write-Host "  [1] Download and install now"
                                Write-Host "  [2] Go back to model selection"
                                $dl = Read-Host "  Choose [1-2]"
                                if ($dl -eq "1") {
                                    $url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$($chosen.File)"
                                    $dest = "$PSScriptRoot\models\$($chosen.File)"
                                    Write-Host "  Downloading $($chosen.File) ($($chosen.Size))..." -ForegroundColor Cyan
                                    try {
                                        Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing
                                        Write-Host "  Download complete." -ForegroundColor Green
                                    } catch {
                                        Write-Host "  Download failed: $($_.Exception.Message)" -ForegroundColor Red
                                        Write-Host "  Press Enter to go back..." -ForegroundColor DarkGray
                                        Read-Host
                                        continue
                                    }
                                } else {
                                    continue
                                }
                            }
                            $currentModel = $chosenPath
                            $content = $content -replace 'model_path\s*=\s*".+?"', "model_path = `"$chosenPath`""
                            Write-Host "  Model set to: $($chosen.File)" -ForegroundColor Green
                            $changed = $true
                            $modelSelected = $true
                        } else {
                            Write-Host "  Invalid selection." -ForegroundColor Red
                            Start-Sleep -Seconds 1
                        }
                    }
                }
                "2" {
                    $presets = @(
                        @{ Name = "Laptop built-in mic"; energy = "0.008"; silence = "90"; minspeech = "16000"; beam = "3"; patience = "0.8"; nospeech = "0.5"; entropy = "2.0" },
                        @{ Name = "Headset / USB mic"; energy = "0.015"; silence = "60"; minspeech = "8000"; beam = "5"; patience = "1.0"; nospeech = "0.6"; entropy = "2.4" },
                        @{ Name = "Studio / condenser mic"; energy = "0.025"; silence = "50"; minspeech = "8000"; beam = "7"; patience = "1.2"; nospeech = "0.6"; entropy = "2.4" },
                        @{ Name = "Noisy environment"; energy = "0.035"; silence = "100"; minspeech = "16000"; beam = "5"; patience = "1.0"; nospeech = "0.4"; entropy = "1.8" }
                    )
                    Write-Host ""
                    Write-Host "  Microphone presets:" -ForegroundColor Cyan
                    for ($i = 0; $i -lt $presets.Count; $i++) {
                        $marker = if ($presets[$i].Name -eq $currentPreset) { " (current)" } elseif ($i -eq 1) { " (recommended)" } else { "" }
                        Write-Host "  [$($i+1)] $($presets[$i].Name)$marker"
                    }
                    $sel = Read-Host "  Select preset [1-$($presets.Count)]"
                    if ($sel -match '^\d+$' -and [int]$sel -ge 1 -and [int]$sel -le $presets.Count) {
                        $p = $presets[[int]$sel - 1]
                        $currentPreset = $p.Name
                        $content = $content -replace '(?m)^name\s*=\s*".+?"\s*\r?\nenergy_threshold.*', "name = `"$($p.Name)`"`nenergy_threshold = $($p.energy)`nsilence_frames_needed = $($p.silence)`nmin_speech_samples = $($p.minspeech)`nbeam_size = $($p.beam)`npatience = $($p.patience)`nno_speech_thold = $($p.nospeech)`nentropy_thold = $($p.entropy)"
                        Write-Host "  Preset set to: $($p.Name)" -ForegroundColor Green
                        $changed = $true
                    } else {
                        Write-Host "  Invalid selection." -ForegroundColor Red
                    }
                }
                "3" {
                    Write-Host ""
                    Write-Host "  Languages: es, en, fr, de, it, pt, ja, ko, zh, auto" -ForegroundColor Cyan
                    $lang = Read-Host "  Language (current: $currentLang)"
                    if ($lang -and $lang -ne $currentLang) {
                        $currentLang = $lang
                        $content = $content -replace 'language\s*=\s*".+?"', "language = `"$lang`""
                        Write-Host "  Language set to: $lang" -ForegroundColor Green
                        $changed = $true
                    }
                }
                "4" {
                    Write-Host ""
                    Write-Host "  Examples: F9, F10, Ctrl+F12, Alt+R" -ForegroundColor Cyan
                    $hk = Read-Host "  Hotkey (current: $currentHotkey)"
                    if ($hk -and $hk -ne $currentHotkey) {
                        $currentHotkey = $hk
                        $content = $content -replace 'start_stop\s*=\s*".+?"', "start_stop = `"$hk`""
                        Write-Host "  Hotkey set to: $hk" -ForegroundColor Green
                        $changed = $true
                    }
                }
                "5" {
                    Write-Host ""
                    Write-Host "  Timing configuration (milliseconds / seconds)" -ForegroundColor Cyan
                    Write-Host "  Lower silence = faster response but may cut words on pauses" -ForegroundColor DarkGray
                    Write-Host ""

                    $val = Read-Host "  Silence timeout in ms (current: $currentSilence, default: 350)"
                    if ($val -and $val -match '^\d+$') {
                        $currentSilence = $val
                        if ($content -match '(?m)^\[timing\]') {
                            $content = $content -replace '(?m)^silence_timeout_ms\s*=\s*\d+', "silence_timeout_ms = $val"
                        } else {
                            $content += "`n[timing]`nsilence_timeout_ms = $val`nmin_speech_ms = $currentMinSpeech`nmax_utterance_secs = $currentMaxUtt`n"
                        }
                        $changed = $true
                    }

                    $val = Read-Host "  Min speech duration in ms (current: $currentMinSpeech, default: 250)"
                    if ($val -and $val -match '^\d+$') {
                        $currentMinSpeech = $val
                        if ($content -match '(?m)^\[timing\]') {
                            $content = $content -replace '(?m)^min_speech_ms\s*=\s*\d+', "min_speech_ms = $val"
                        } else {
                            $content += "`n[timing]`nsilence_timeout_ms = $currentSilence`nmin_speech_ms = $val`nmax_utterance_secs = $currentMaxUtt`n"
                        }
                        $changed = $true
                    }

                    $val = Read-Host "  Max utterance length in seconds (current: $currentMaxUtt, default: 30)"
                    if ($val -and $val -match '^\d+$') {
                        $currentMaxUtt = $val
                        if ($content -match '(?m)^\[timing\]') {
                            $content = $content -replace '(?m)^max_utterance_secs\s*=\s*\d+', "max_utterance_secs = $val"
                        } else {
                            $content += "`n[timing]`nsilence_timeout_ms = $currentSilence`nmin_speech_ms = $currentMinSpeech`nmax_utterance_secs = $val`n"
                        }
                        $changed = $true
                    }

                    Write-Host "  Timing updated." -ForegroundColor Green
                }
                "6" {
                    if ($changed) {
                        Set-Content -Path $configFile -Value $content -Encoding UTF8
                        Clear-Host
                        Write-Host ""
                        Write-Host "  simpleSTT Configuration" -ForegroundColor Cyan
                        Write-Host "  -----------------------------------------------" -ForegroundColor DarkGray
                        Write-Host "  Model:   $currentModel" -ForegroundColor Green
                        Write-Host "  Preset:  $currentPreset" -ForegroundColor Green
                        Write-Host "  Language: $currentLang" -ForegroundColor Green
                        Write-Host "  Hotkey:  $currentHotkey" -ForegroundColor Green
                        Write-Host "  -----------------------------------------------" -ForegroundColor DarkGray
                        Write-Host ""
                        Write-Host "  Config saved." -ForegroundColor Green
                        Write-Host ""
                    }
                    return
                }
                "0" {
                    if ($changed) {
                        Write-Host ""
                        Write-Host "  Changes not saved." -ForegroundColor Yellow
                    }
                    return
                }
                default {
                    return
                }
            }
        }
    }

    default {
        Write-Host @"
simpleSTT commands:
  .\stt.ps1 run         Run in foreground (see logs)
  .\stt.ps1 start       Build if needed + start in background
  .\stt.ps1 stop        Stop running process
  .\stt.ps1 restart     Stop + start
  .\stt.ps1 status      Show if running
  .\stt.ps1 config      Change model, preset, language, hotkey
  .\stt.ps1 install     Build + create Start Menu shortcut
  .\stt.ps1 uninstall   Stop + remove shortcuts

GPU flags (append to build/run/start/install/restart):
  -Cuda       Force CUDA build
  -Vulkan     Force Vulkan build
  -Cpu        Force CPU-only build (no GPU)

Auto-detect: by default uses CUDA if nvcuda.dll found, Vulkan if vulkan-1.dll found.
"@ -ForegroundColor Cyan
    }
}
