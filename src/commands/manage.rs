use std::process::Command;

fn find_pids() -> Vec<u32> {
    let output = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq simplestt.exe", "/FO", "CSV", "/NH"])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() >= 2 {
                        let pid_str = parts[1].trim_matches('"');
                        pid_str.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

pub fn status() {
    let pids = find_pids();
    if pids.is_empty() {
        println!("Not running.");
        return;
    }
    for pid in &pids {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
            .output();
        let mem = match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout
                    .lines()
                    .next()
                    .and_then(|line| {
                        let parts: Vec<&str> = line.split(',').collect();
                        if parts.len() >= 5 {
                            let mem_str = parts[4].trim_matches('"').replace(" K", "");
                            mem_str.trim().parse::<u64>().ok()
                        } else {
                            None
                        }
                    })
                    .map(|kb| format!("{:.1} MB", kb as f64 / 1024.0))
                    .unwrap_or_else(|| "unknown".to_string())
            }
            Err(_) => "unknown".to_string(),
        };
        println!("Running (PID {}, {})", pid, mem);
    }
}

pub fn stop() {
    let pids = find_pids();
    if pids.is_empty() {
        println!("Not running.");
        return;
    }
    for pid in &pids {
        let _ = Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output();
    }
    println!("Stopped.");
}

pub fn start() {
    let pids = find_pids();
    if !pids.is_empty() {
        println!("Already running (PID {}).", pids[0]);
        return;
    }

    let current_exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("Error: cannot find executable: {}", e);
            std::process::exit(1);
        }
    };

    #[cfg(target_os = "windows")]
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    #[cfg(target_os = "windows")]
    const DETACHED_PROCESS: u32 = 0x00000008;
    #[cfg(target_os = "windows")]
    const CREATION_FLAGS: u32 = DETACHED_PROCESS | CREATE_NO_WINDOW;

    #[cfg(not(target_os = "windows"))]
    const CREATION_FLAGS: u32 = 0;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        Command::new(&current_exe)
            .arg("run")
            .creation_flags(CREATION_FLAGS)
            .spawn()
            .ok();
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new(&current_exe).arg("run").spawn().ok();
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

    let new_pids: Vec<u32> = find_pids()
        .into_iter()
        .filter(|p| !pids.contains(p))
        .collect();

    if !new_pids.is_empty() {
        println!(
            "Started (PID {}). Press F9 to toggle recording.",
            new_pids[0]
        );
    } else {
        println!("Failed to start. Run manually: simplestt run");
    }
}

pub fn restart() {
    stop();
    std::thread::sleep(std::time::Duration::from_millis(500));
    start();
}
