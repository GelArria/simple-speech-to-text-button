use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn install_dir() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
        format!("{}\\AppData\\Local", home)
    });
    PathBuf::from(local_app_data)
        .join("Programs")
        .join("simplestt")
}

fn current_exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("simplestt.exe"))
}

fn source_models_dir() -> PathBuf {
    let exe_dir = current_exe_path()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    let cwd_models = std::env::current_dir().unwrap_or_default().join("models");

    if cwd_models.join("ggml-base.bin").exists() || cwd_models.join("ggml-tiny.bin").exists() {
        cwd_models
    } else if exe_dir.join("models").exists() {
        exe_dir.join("models")
    } else {
        cwd_models
    }
}

#[cfg(target_os = "windows")]
fn add_to_path(dir: &std::path::Path) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|e| format!("cannot open registry: {}", e))?;

    let mut current_path: String = env.get_value("Path").unwrap_or_else(|_| String::new());

    let dir_str = dir.to_str().ok_or("invalid path")?;
    if current_path.contains(dir_str) {
        return Ok(());
    }

    if !current_path.is_empty() && !current_path.ends_with(';') {
        current_path.push(';');
    }
    current_path.push_str(dir_str);

    env.set_value("Path", &current_path)
        .map_err(|e| format!("cannot set Path: {}", e))?;

    broadcast_env_change();

    Ok(())
}

#[cfg(target_os = "windows")]
fn remove_from_path(dir: &std::path::Path) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|e| format!("cannot open registry: {}", e))?;

    let current_path: String = env.get_value("Path").unwrap_or_else(|_| String::new());

    let dir_str = dir.to_str().ok_or("invalid path")?;
    let new_path: String = current_path
        .split(';')
        .filter(|p| {
            let normalized = p.replace('/', "\\").trim_matches('\\').to_lowercase();
            let target = dir_str.replace('/', "\\").trim_matches('\\').to_lowercase();
            normalized != target
        })
        .collect::<Vec<_>>()
        .join(";");

    env.set_value("Path", &new_path)
        .map_err(|e| format!("cannot set Path: {}", e))?;

    broadcast_env_change();

    Ok(())
}

#[cfg(target_os = "windows")]
fn broadcast_env_change() {
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };

    let wide: Vec<u16> = "Environment\0".encode_utf16().collect();
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(wide.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5000,
            None,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn add_to_path(_dir: &std::path::Path) -> Result<(), String> {
    Err("PATH management not implemented for this OS".to_string())
}

#[cfg(not(target_os = "windows"))]
fn remove_from_path(_dir: &std::path::Path) -> Result<(), String> {
    Err("PATH management not implemented for this OS".to_string())
}

fn create_shortcut(
    target: &std::path::Path,
    shortcut_path: &std::path::Path,
) -> Result<(), String> {
    let target_str = target.to_str().ok_or("invalid target path")?;
    let lnk_str = shortcut_path.to_str().ok_or("invalid shortcut path")?;

    let ps_cmd = format!(
        "$ws = New-Object -ComObject WScript.Shell; $sc = $ws.CreateShortcut('{}'); $sc.TargetPath = '{}'; $sc.WorkingDirectory = '{}'; $sc.Description = 'simpleSTT - Speech to Text'; $sc.Save()",
        lnk_str.replace('\'', "''"),
        target_str.replace('\'', "''"),
        target
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_str()
            .unwrap_or(".")
            .replace('\'', "''"),
    );

    let result = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .output();

    match result {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(format!(
            "powershell error: {}",
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(e) => Err(format!("failed to run powershell: {}", e)),
    }
}

fn create_start_menu_shortcut(target: &std::path::Path) -> Result<(), String> {
    let start_menu = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string())
        + r"\Microsoft\Windows\Start Menu\Programs";
    let lnk_path = PathBuf::from(start_menu).join("simpleSTT.lnk");
    create_shortcut(target, &lnk_path)
}

fn create_startup_shortcut(target: &std::path::Path) -> Result<(), String> {
    let startup = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string())
        + r"\Microsoft\Windows\Start Menu\Programs\Startup";
    let lnk_path = PathBuf::from(startup).join("simpleSTT.lnk");
    create_shortcut(target, &lnk_path)
}

fn remove_shortcuts() {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let paths = [
        format!(
            r"{}\Microsoft\Windows\Start Menu\Programs\simpleSTT.lnk",
            appdata
        ),
        format!(
            r"{}\Microsoft\Windows\Start Menu\Programs\Startup\simpleSTT.lnk",
            appdata
        ),
    ];
    for p in &paths {
        if fs::metadata(p).is_ok() {
            let _ = fs::remove_file(p);
        }
    }
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|e| format!("create dir: {}", e))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read dir: {}", e))? {
        let entry = entry.map_err(|e| format!("entry: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| format!("copy: {}", e))?;
        }
    }
    Ok(())
}

pub fn install() {
    let exe = current_exe_path();
    let dest_dir = install_dir();
    let dest_exe = dest_dir.join("simplestt.exe");
    let dest_models = dest_dir.join("models");

    println!("Installing simpleSTT...");

    fs::create_dir_all(&dest_dir).unwrap_or_else(|e| {
        eprintln!("Failed to create {}: {}", dest_dir.display(), e);
        std::process::exit(1);
    });

    println!("  Copying executable...");
    fs::copy(&exe, &dest_exe).unwrap_or_else(|e| {
        eprintln!("Failed to copy executable: {}", e);
        std::process::exit(1);
    });

    let src_models = source_models_dir();
    if src_models.exists() {
        println!("  Copying models...");
        if let Err(e) = copy_dir_recursive(&src_models, &dest_models) {
            println!("  Warning: could not copy models: {}", e);
            println!(
                "  You can manually copy models to: {}",
                dest_models.display()
            );
        } else {
            let count = fs::read_dir(&dest_models)
                .map(|entries| entries.filter_map(|e| e.ok()).count())
                .unwrap_or(0);
            println!("  Copied {} model(s).", count);
        }
    } else {
        println!("  No models directory found at source. Skipping model copy.");
        println!(
            "  You can manually place models in: {}",
            dest_models.display()
        );
    }

    match add_to_path(&dest_dir) {
        Ok(()) => println!("  Added to user PATH."),
        Err(e) => {
            println!("  Warning: could not add to PATH: {}", e);
            println!("  Add '{}' to your PATH manually.", dest_dir.display());
        }
    }

    match create_start_menu_shortcut(&dest_exe) {
        Ok(()) => println!("  Created Start Menu shortcut."),
        Err(e) => println!("  Warning: could not create Start Menu shortcut: {}", e),
    }

    match create_startup_shortcut(&dest_exe) {
        Ok(()) => println!("  Created Startup shortcut (auto-starts on login)."),
        Err(e) => println!("  Warning: could not create Startup shortcut: {}", e),
    }

    let size_mb = fs::metadata(&dest_exe)
        .map(|m| m.len() as f64 / (1024.0 * 1024.0))
        .map(|s| format!("{:.1} MB", s))
        .unwrap_or_else(|_| "unknown".to_string());

    println!();
    println!("  Installation complete.");
    println!("  Location: {}", dest_dir.display());
    println!("  Executable: {} ({})", dest_exe.display(), size_mb);
    println!();
    println!("  Open a NEW terminal to use: simplestt run");
    println!("  (current terminal won't see the PATH change)");
}

pub fn uninstall() {
    let dest_dir = install_dir();

    println!("Uninstalling simpleSTT...");

    let exe = dest_dir.join("simplestt.exe");

    if fs::metadata(&exe).is_err() {
        println!("  Not installed (no executable at {}).", dest_dir.display());
        return;
    }

    println!("  Stopping running instances...");
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "simplestt.exe"])
        .output();
    std::thread::sleep(std::time::Duration::from_millis(500));

    match remove_from_path(&dest_dir) {
        Ok(()) => println!("  Removed from user PATH."),
        Err(e) => println!("  Warning: could not remove from PATH: {}", e),
    }

    remove_shortcuts();
    println!("  Removed shortcuts.");

    match fs::remove_dir_all(&dest_dir) {
        Ok(()) => println!("  Removed installation directory."),
        Err(e) => println!("  Warning: could not remove directory: {}", e),
    }

    let config_dir = directories::ProjectDirs::from("com", "simplestt", "simplestt")
        .map(|d| d.config_dir().to_path_buf());

    if let Some(config) = config_dir {
        if config.exists() {
            println!();
            print!("  Remove config directory? ({}): ", config.display());
            std::io::stdout().flush().ok();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            if input.trim().to_lowercase() == "y" || input.trim().is_empty() {
                match fs::remove_dir_all(&config) {
                    Ok(()) => println!("  Removed config directory."),
                    Err(e) => println!("  Warning: could not remove config: {}", e),
                }
            } else {
                println!("  Config directory left intact.");
            }
        }
    }

    println!();
    println!("  Uninstall complete.");
}
