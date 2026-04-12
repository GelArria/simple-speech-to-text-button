param(
    [Parameter(Position=0)]
    [ValidateSet("build", "install", "help")]
    [string]$Action = "build",

    [switch]$Cuda,
    [switch]$Vulkan,
    [switch]$Cpu
)

$ErrorActionPreference = "Stop"
$ExeName = "simplestt.exe"
$ExePath = "$PSScriptRoot\target\release\$ExeName"

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

switch ($Action) {
    "build" {
        Build-Exe
        Write-Host ""
        Write-Host "  Now use the built-in CLI:" -ForegroundColor Cyan
        Write-Host "  .\target\release\simplestt.exe install   # install globally" -ForegroundColor White
        Write-Host "  .\target\release\simplestt.exe run       # start overlay" -ForegroundColor White
        Write-Host "  .\target\release\simplestt.exe config    # configure" -ForegroundColor White
        Write-Host "  .\target\release\simplestt.exe --help    # all commands" -ForegroundColor White
    }

    "install" {
        Build-Exe
        & $ExePath install
    }

    "help" {
        Write-Host @"
stt.ps1 - Build helper for simpleSTT
====================================
This script only handles building. All other commands are in the binary.

Usage:
  .\stt.ps1 build        Build the binary (default)
  .\stt.ps1 install      Build + install globally to PATH
  .\stt.ps1 build -Cuda  Build with CUDA
  .\stt.ps1 build -Vulkan Build with Vulkan
  .\stt.ps1 build -Cpu   Build CPU-only

After building, use the binary CLI:
  simplestt                Run overlay (default)
  simplestt run            Run overlay
  simplestt start          Start in background
  simplestt stop           Stop running instance
  simplestt restart        Restart
  simplestt status         Show status
  simplestt config         Interactive configuration
  simplestt install        Install globally to PATH
  simplestt uninstall      Uninstall from system
"@ -ForegroundColor Cyan
    }
}
