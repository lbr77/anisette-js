use std::collections::HashMap;

use goblin::elf::program_header::PT_LOAD;
use goblin::elf::section_header::SHN_UNDEF;
use goblin::elf::{Elf, Reloc};
use unicorn_engine::unicorn_const::{Arch, HookType, Mode, Permission, uc_error};
use unicorn_engine::{RegisterARM64, Unicorn};

use crate::constants::{
    ARG_REGS, IMPORT_ADDRESS, IMPORT_LIBRARY_COUNT, IMPORT_LIBRARY_STRIDE, IMPORT_SIZE,
    LIB_RESERVATION_SIZE, MALLOC_ADDRESS, MALLOC_SIZE, PAGE_SIZE, RET_AARCH64, RETURN_ADDRESS,
    STACK_ADDRESS, STACK_SIZE,
};
use crate::debug::{debug_print, trace_mem_invalid_hook};
use crate::errors::VmError;
use crate::runtime::{LoadedLibrary, RuntimeState, SymbolEntry};
use crate::stub::dispatch_import_stub;
use crate::util::{add_i64, align_down, align_up, as_usize};

pub struct EmuCore {
    uc: Unicorn<'static, RuntimeState>,
}

impl EmuCore {
    pub fn new_arm64() -> Result<Self, VmError> {
        let mut uc = Unicorn::new_with_data(Arch::ARM64, Mode::ARM, RuntimeState::new())?;

        uc.mem_map(RETURN_ADDRESS, as_usize(PAGE_SIZE)?, Permission::ALL)?;
        uc.mem_map(MALLOC_ADDRESS, as_usize(MALLOC_SIZE)?, Permission::ALL)?;
        uc.mem_map(STACK_ADDRESS, as_usize(STACK_SIZE)?, Permission::ALL)?;

        for i in 0..IMPORT_LIBRARY_COUNT {
            let base = IMPORT_ADDRESS + (i as u64) * IMPORT_LIBRARY_STRIDE;
            uc.mem_map(base, as_usize(IMPORT_SIZE)?, Permission::ALL)?;

            let mut stubs = vec![0_u8; IMPORT_SIZE as usize];
            for chunk in stubs.chunks_mut(4) {
                chunk.copy_from_slice(&RET_AARCH64);
            }
            uc.mem_write(base, &stubs)?;

            uc.add_code_hook(base, base + IMPORT_SIZE - 1, |uc, address, _| {
                if let Err(err) = dispatch_import_stub(uc, address) {
                    debug_print(format!("import hook failed at 0x{address:X}: {err}"));
                    let _ = uc.emu_stop();
                }
            })?;
        }

        uc.add_mem_hook(
            HookType::MEM_READ_UNMAPPED
                | HookType::MEM_WRITE_UNMAPPED
                | HookType::MEM_FETCH_UNMAPPED,
            1,
            0,
            |uc, access, address, size, value| {
                trace_mem_invalid_hook(uc, access, address, size, value);
                false
            },
        )?;

        Ok(Self { uc })
    }

    pub fn register_library_blob(&mut self, name: impl Into<String>, data: Vec<u8>) {
        self.uc
            .get_data_mut()
            .library_blobs
            .insert(name.into(), data);
    }

    pub fn set_library_root(&mut self, path: &str) {
        let normalized = normalize_library_root(path);
        if normalized.is_empty() {
            return;
        }
        self.uc.get_data_mut().library_root = Some(normalized);
    }

    pub fn load_library(&mut self, library_name: &str) -> Result<usize, VmError> {
        load_library_by_name(&mut self.uc, library_name)
    }

    pub fn resolve_symbol_by_name(
        &self,
        library_index: usize,
        symbol_name: &str,
    ) -> Result<u64, VmError> {
        resolve_symbol_from_loaded_library_by_name(&self.uc, library_index, symbol_name)
    }

    pub fn invoke_cdecl(&mut self, address: u64, args: &[u64]) -> Result<u64, VmError> {
        if args.len() > ARG_REGS.len() {
            return Err(VmError::TooManyArguments(args.len()));
        }

        for (index, value) in args.iter().enumerate() {
            self.uc.reg_write(ARG_REGS[index], *value)?;
            debug_print(format!("X{index}: 0x{value:08X}"));
        }

        debug_print(format!("Calling 0x{address:X}"));
        self.uc
            .reg_write(RegisterARM64::SP, STACK_ADDRESS + STACK_SIZE)?;
        self.uc.reg_write(RegisterARM64::LR, RETURN_ADDRESS)?;
        self.uc.emu_start(address, RETURN_ADDRESS, 0, 0)?;
        Ok(self.uc.reg_read(RegisterARM64::X0)?)
    }

    pub fn alloc_data(&mut self, data: &[u8]) -> Result<u64, VmError> {
        alloc_temp_bytes(&mut self.uc, data, 0xCC)
    }

    pub fn alloc_temporary(&mut self, length: usize) -> Result<u64, VmError> {
        let data = vec![0xAA; length.max(1)];
        alloc_temp_bytes(&mut self.uc, &data, 0xAA)
    }

    pub fn read_data(&self, address: u64, length: usize) -> Result<Vec<u8>, VmError> {
        Ok(self.uc.mem_read_as_vec(address, length)?)
    }

    pub fn write_data(&mut self, address: u64, data: &[u8]) -> Result<(), VmError> {
        self.uc.mem_write(address, data)?;
        Ok(())
    }

    pub fn read_u32(&self, address: u64) -> Result<u32, VmError> {
        let mut bytes = [0_u8; 4];
        self.uc.mem_read(address, &mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub fn read_u64(&self, address: u64) -> Result<u64, VmError> {
        let mut bytes = [0_u8; 8];
        self.uc.mem_read(address, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub fn write_u32(&mut self, address: u64, value: u32) -> Result<(), VmError> {
        self.uc.mem_write(address, &value.to_le_bytes())?;
        Ok(())
    }

    pub fn write_u64(&mut self, address: u64, value: u64) -> Result<(), VmError> {
        self.uc.mem_write(address, &value.to_le_bytes())?;
        Ok(())
    }


    pub fn read_c_string(&self, address: u64, max_len: usize) -> Result<String, VmError> {
        read_c_string(&self.uc, address, max_len)
    }
}

pub(crate) fn alloc_c_string(core: &mut EmuCore, value: &str) -> Result<u64, VmError> {
    let mut bytes = Vec::with_capacity(value.len() + 1);
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(0);
    core.alloc_data(&bytes)
}

pub(crate) fn ensure_zero_return(name: &'static str, value: u64) -> Result<(), VmError> {
    let code = value as u32 as i32;
    if code == 0 {
        Ok(())
    } else {
        Err(VmError::AdiCallFailed { name, code })
    }
}

fn normalize_library_root(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(trimmed.len());
    let mut prev_slash = false;
    for ch in trimmed.chars() {
        if ch == '/' {
            if prev_slash {
                continue;
            }
            prev_slash = true;
        } else {
            prev_slash = false;
        }
        out.push(ch);
    }

    while out.len() > 1 && out.ends_with('/') {
        out.pop();
    }

    out
}

fn alloc_temp_bytes(
    uc: &mut Unicorn<'_, RuntimeState>,
    data: &[u8],
    padding_byte: u8,
) -> Result<u64, VmError> {
    let request = data.len().max(1) as u64;
    let length = align_up(request, PAGE_SIZE);
    let address = {
        let state = uc.get_data_mut();
        state.temp_allocator.alloc(length)?
    };

    debug_print(format!(
        "Allocating at 0x{address:X}; bytes 0x{:X}/0x{length:X}",
        data.len()
    ));
    uc.mem_map(address, as_usize(length)?, Permission::ALL)?;

    let mut buffer = vec![padding_byte; length as usize];
    if !data.is_empty() {
        buffer[..data.len()].copy_from_slice(data);
    }
    uc.mem_write(address, &buffer)?;

    Ok(address)
}

pub(crate) fn set_errno(uc: &mut Unicorn<'_, RuntimeState>, value: u32) -> Result<(), VmError> {
    let errno_address = ensure_errno_address(uc)?;
    uc.mem_write(errno_address, &value.to_le_bytes())?;
    Ok(())
}

pub(crate) fn ensure_errno_address(uc: &mut Unicorn<'_, RuntimeState>) -> Result<u64, VmError> {
    if let Some(address) = uc.get_data().errno_address {
        return Ok(address);
    }

    let address = alloc_temp_bytes(uc, &[0, 0, 0, 0], 0)?;
    uc.get_data_mut().errno_address = Some(address);
    Ok(address)
}

pub(crate) fn load_library_by_name(
    uc: &mut Unicorn<'_, RuntimeState>,
    library_name: &str,
) -> Result<usize, VmError> {
    for (index, library) in uc.get_data().loaded_libraries.iter().enumerate() {
        if library.name == library_name {
            debug_print("Library already loaded");
            return Ok(index);
        }
    }

    let (library_index, elf_data) = {
        let state = uc.get_data();
        let data = state
            .library_blobs
            .get(library_name)
            .cloned()
            .ok_or_else(|| VmError::LibraryNotRegistered(library_name.to_string()))?;
        (state.loaded_libraries.len(), data)
    };

    let elf = Elf::parse(&elf_data)?;
    let base = {
        let state = uc.get_data_mut();
        state.library_allocator.alloc(LIB_RESERVATION_SIZE)?
    };

    let mut symbols = Vec::with_capacity(elf.dynsyms.len());
    let mut symbols_by_name = HashMap::new();

    for (index, sym) in elf.dynsyms.iter().enumerate() {
        let name = elf.dynstrtab.get_at(sym.st_name).unwrap_or("").to_string();
        let resolved = if sym.st_shndx == SHN_UNDEF as usize {
            IMPORT_ADDRESS + (library_index as u64) * IMPORT_LIBRARY_STRIDE + (index as u64) * 4
        } else {
            base.wrapping_add(sym.st_value)
        };

        if !name.is_empty() {
            symbols_by_name.entry(name.clone()).or_insert(resolved);
        }

        symbols.push(SymbolEntry { name, resolved });
    }

    for ph in &elf.program_headers {
        let seg_addr = base.wrapping_add(ph.p_vaddr);
        let map_start = align_down(seg_addr, PAGE_SIZE);
        let map_end = align_up(seg_addr.wrapping_add(ph.p_memsz), PAGE_SIZE);
        let map_len = map_end.saturating_sub(map_start);

        if map_len == 0 {
            continue;
        }

        debug_print(format!(
            "Mapping at 0x{map_start:X}-0x{map_end:X} (0x{seg_addr:X}-0x{:X}); bytes 0x{map_len:X}",
            seg_addr + map_len.saturating_sub(1)
        ));

        if ph.p_type != PT_LOAD || ph.p_memsz == 0 {
            debug_print(format!(
                "- Skipping p_type={} offset=0x{:X} vaddr=0x{:X}",
                ph.p_type, ph.p_offset, ph.p_vaddr
            ));
            continue;
        }
        match uc.mem_map(map_start, as_usize(map_len)?, Permission::ALL) {
            Ok(()) => {}
            Err(uc_error::MAP) => {}
            Err(err) => return Err(err.into()),
        }

        let file_offset = ph.p_offset as usize;
        let file_len = ph.p_filesz as usize;
        let file_end = file_offset
            .checked_add(file_len)
            .ok_or(VmError::InvalidElfRange)?;

        if file_end > elf_data.len() {
            return Err(VmError::InvalidElfRange);
        }

        let mut bytes = vec![0_u8; map_len as usize];
        let start_offset = (seg_addr - map_start) as usize;

        if file_len > 0 {
            let dest_end = start_offset
                .checked_add(file_len)
                .ok_or(VmError::InvalidElfRange)?;
            if dest_end > bytes.len() {
                return Err(VmError::InvalidElfRange);
            }
            bytes[start_offset..dest_end].copy_from_slice(&elf_data[file_offset..file_end]);
        }

        uc.mem_write(map_start, &bytes)?;
    }

    for rela in elf.dynrelas.iter() {
        apply_relocation(uc, base, &rela, library_name, &symbols)?;
    }

    for rela in elf.pltrelocs.iter() {
        apply_relocation(uc, base, &rela, library_name, &symbols)?;
    }

    let loaded = LoadedLibrary {
        name: library_name.to_string(),
        symbols,
        symbols_by_name,
    };

    uc.get_data_mut().loaded_libraries.push(loaded);

    Ok(library_index)
}

fn apply_relocation(
    uc: &mut Unicorn<'_, RuntimeState>,
    base: u64,
    relocation: &Reloc,
    library_name: &str,
    symbols: &[SymbolEntry],
) -> Result<(), VmError> {
    if relocation.r_type == 0 {
        return Ok(());
    }

    let relocation_addr = base.wrapping_add(relocation.r_offset);
    let addend = relocation.r_addend.unwrap_or(0);

    let symbol_address = if relocation.r_sym < symbols.len() {
        symbols[relocation.r_sym].resolved
    } else {
        return Err(VmError::SymbolIndexOutOfRange {
            library: library_name.to_string(),
            index: relocation.r_sym,
        });
    };

    let value = match relocation.r_type {
        goblin::elf64::reloc::R_AARCH64_ABS64 | goblin::elf64::reloc::R_AARCH64_GLOB_DAT => {
            add_i64(symbol_address, addend)
        }
        goblin::elf64::reloc::R_AARCH64_JUMP_SLOT => symbol_address,
        goblin::elf64::reloc::R_AARCH64_RELATIVE => add_i64(base, addend),
        other => return Err(VmError::UnsupportedRelocation(other)),
    };

    uc.mem_write(relocation_addr, &value.to_le_bytes())?;
    Ok(())
}

pub(crate) fn resolve_symbol_from_loaded_library_by_name(
    uc: &Unicorn<'_, RuntimeState>,
    library_index: usize,
    symbol_name: &str,
) -> Result<u64, VmError> {
    let library = uc
        .get_data()
        .loaded_libraries
        .get(library_index)
        .ok_or(VmError::LibraryNotLoaded(library_index))?;

    library
        .symbols_by_name
        .get(symbol_name)
        .copied()
        .ok_or_else(|| VmError::SymbolNotFound {
            library: library.name.clone(),
            symbol: symbol_name.to_string(),
        })
}

pub(crate) fn read_c_string(
    uc: &Unicorn<'_, RuntimeState>,
    address: u64,
    max_len: usize,
) -> Result<String, VmError> {
    let bytes = uc.mem_read_as_vec(address, max_len)?;
    let len = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(VmError::UnterminatedCString(address))?;
    Ok(String::from_utf8_lossy(&bytes[..len]).into_owned())
}
