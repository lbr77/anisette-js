use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use unicorn_engine::{RegisterARM64, Unicorn};

use crate::constants::{
    ENOENT, IMPORT_ADDRESS, IMPORT_LIBRARY_STRIDE, O_ACCMODE, O_CREAT, O_NOFOLLOW, O_RDWR, O_WRONLY,
};
use crate::debug::{debug_print, debug_trace};
use crate::emu::{
    ensure_errno_address, load_library_by_name, read_c_string,
    resolve_symbol_from_loaded_library_by_name, set_errno,
};
use crate::errors::VmError;
use crate::runtime::RuntimeState;
use crate::util::bytes_to_hex;

pub fn dispatch_import_stub(
    uc: &mut Unicorn<'_, RuntimeState>,
    address: u64,
) -> Result<(), VmError> {
    if address < IMPORT_ADDRESS {
        return Err(VmError::InvalidImportAddress(address));
    }

    let offset = address - IMPORT_ADDRESS;
    let library_index = (offset / IMPORT_LIBRARY_STRIDE) as usize;
    let symbol_index = ((offset % IMPORT_LIBRARY_STRIDE) / 4) as usize;

    let symbol_name =
        {
            let state = uc.get_data();
            let library = state
                .loaded_libraries
                .get(library_index)
                .ok_or(VmError::LibraryNotLoaded(library_index))?;

            let symbol = library.symbols.get(symbol_index).ok_or_else(|| {
                VmError::SymbolIndexOutOfRange {
                    library: library.name.clone(),
                    index: symbol_index,
                }
            })?;

            symbol.name.clone()
        };

    handle_stub_by_name(uc, &symbol_name)
}

fn handle_stub_by_name(
    uc: &mut Unicorn<'_, RuntimeState>,
    symbol_name: &str,
) -> Result<(), VmError> {
    match symbol_name {
        "malloc" => stub_malloc(uc),
        "free" => stub_free(uc),
        "strncpy" => stub_strncpy(uc),
        "mkdir" => stub_mkdir(uc),
        "umask" => stub_umask(uc),
        "chmod" => stub_chmod(uc),
        "lstat" => stub_lstat(uc),
        "fstat" => stub_fstat(uc),
        "open" => stub_open(uc),
        "ftruncate" => stub_ftruncate(uc),
        "read" => stub_read(uc),
        "write" => stub_write(uc),
        "close" => stub_close(uc),
        "dlopen" => stub_dlopen(uc),
        "dlsym" => stub_dlsym(uc),
        "dlclose" => stub_dlclose(uc),
        "pthread_once" => stub_return_zero(uc),
        "pthread_create" => stub_return_zero(uc),
        "pthread_mutex_lock" => stub_return_zero(uc),
        "pthread_rwlock_unlock" => stub_return_zero(uc),
        "pthread_rwlock_destroy" => stub_return_zero(uc),
        "pthread_rwlock_wrlock" => stub_return_zero(uc),
        "pthread_rwlock_init" => stub_return_zero(uc),
        "pthread_mutex_unlock" => stub_return_zero(uc),
        "pthread_rwlock_rdlock" => stub_return_zero(uc),
        "gettimeofday" => stub_gettimeofday(uc),
        "__errno" => stub_errno_location(uc),
        "__system_property_get" => stub_system_property_get(uc),
        "arc4random" => stub_arc4random(uc),
        other => {
            debug_print(other);
            Err(VmError::UnhandledImport(other.to_string()))
        }
    }
}

fn stub_return_zero(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    uc.reg_write(RegisterARM64::X0, 0)?;
    Ok(())
}

fn stub_malloc(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let request = uc.reg_read(RegisterARM64::X0)?;
    let address = {
        let state = uc.get_data_mut();
        state.malloc_allocator.alloc(request)?
    };

    debug_trace(format!("malloc(0x{request:X})=0x{address:X}"));
    uc.reg_write(RegisterARM64::X0, address)?;
    Ok(())
}

fn stub_free(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    uc.reg_write(RegisterARM64::X0, 0)?;
    Ok(())
}

fn stub_strncpy(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let dst = uc.reg_read(RegisterARM64::X0)?;
    let src = uc.reg_read(RegisterARM64::X1)?;
    let length = uc.reg_read(RegisterARM64::X2)? as usize;

    let input = uc.mem_read_as_vec(src, length)?;
    let copy_len = input
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(length)
        .min(length);

    let mut output = vec![0_u8; length];
    output[..copy_len].copy_from_slice(&input[..copy_len]);

    uc.mem_write(dst, &output)?;
    uc.reg_write(RegisterARM64::X0, dst)?;

    Ok(())
}

fn stub_mkdir(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let path_ptr = uc.reg_read(RegisterARM64::X0)?;
    let mode = uc.reg_read(RegisterARM64::X1)?;
    let path = read_c_string(uc, path_ptr, 0x1000)?;
    debug_trace(format!("mkdir('{path}', {mode:#o})"));

    // Only allow creating ./anisette directory (matches Python reference impl)
    if path != "./anisette" {
        debug_print(format!("mkdir: rejecting invalid path '{path}'"));
        set_errno(uc, ENOENT)?;
        uc.reg_write(RegisterARM64::X0, u64::MAX)?;
        return Ok(());
    }

    match fs::create_dir_all(&path) {
        Ok(()) => {
            uc.reg_write(RegisterARM64::X0, 0)?;
        }
        Err(_) => {
            set_errno(uc, ENOENT)?;
            uc.reg_write(RegisterARM64::X0, u64::MAX)?;
        }
    }

    Ok(())
}

fn stub_umask(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    uc.reg_write(RegisterARM64::X0, 0o777)?;
    Ok(())
}

fn stub_chmod(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let path_ptr = uc.reg_read(RegisterARM64::X0)?;
    let mode = uc.reg_read(RegisterARM64::X1)?;
    let path = read_c_string(uc, path_ptr, 0x1000)?;
    debug_trace(format!("chmod('{path}', {mode:#o})"));
    uc.reg_write(RegisterARM64::X0, 0)?;
    Ok(())
}

fn build_python_stat_bytes(mode: u32, size: u64) -> Vec<u8> {
    let mut stat = Vec::with_capacity(128);

    stat.extend_from_slice(&[0_u8; 8]); // st_dev
    stat.extend_from_slice(&[0_u8; 8]); // st_ino
    stat.extend_from_slice(&mode.to_le_bytes()); // st_mode
    stat.extend_from_slice(&[0_u8; 4]); // st_nlink
    stat.extend_from_slice(&[0xA4, 0x81, 0x00, 0x00]); // st_uid
    stat.extend_from_slice(&[0_u8; 4]); // st_gid
    stat.extend_from_slice(&[0_u8; 8]); // st_rdev
    stat.extend_from_slice(&[0_u8; 8]); // __pad1
    stat.extend_from_slice(&size.to_le_bytes()); // st_size
    stat.extend_from_slice(&[0_u8; 4]); // st_blksize
    stat.extend_from_slice(&[0_u8; 4]); // __pad2
    stat.extend_from_slice(&[0_u8; 8]); // st_blocks
    stat.extend_from_slice(&[0_u8; 8]); // st_atime
    stat.extend_from_slice(&[0_u8; 8]); // st_atime_nsec
    stat.extend_from_slice(&[0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00]); // st_mtime
    stat.extend_from_slice(&[0_u8; 8]); // st_mtime_nsec
    stat.extend_from_slice(&[0_u8; 8]); // st_ctime
    stat.extend_from_slice(&[0_u8; 8]); // st_ctime_nsec
    stat.extend_from_slice(&[0_u8; 4]); // __unused4
    stat.extend_from_slice(&[0_u8; 4]); // __unused5

    stat
}

fn write_python_stat(
    uc: &mut Unicorn<'_, RuntimeState>,
    out_ptr: u64,
    mode: u32,
    size: u64,
    stat_blksize: u64,
    stat_blocks: u64,
) -> Result<(), VmError> {
    debug_print(format!("{size} {stat_blksize} {stat_blocks}"));

    let fake_blksize = 512_u64;
    let fake_blocks = size.div_ceil(512);
    debug_print(format!("{size} {fake_blksize} {fake_blocks}"));

    debug_print(format!("0x{mode:X} = {mode}"));
    let stat_bytes = build_python_stat_bytes(mode, size);
    debug_print(format!("{}", stat_bytes.len()));
    debug_print(format!("Write to ptr: 0x{out_ptr:X}"));
    uc.mem_write(out_ptr, &stat_bytes)?;
    debug_print("Stat struct written to guest memory");
    Ok(())
}

fn stat_path_into_guest(
    uc: &mut Unicorn<'_, RuntimeState>,
    path: &str,
    out_ptr: u64,
) -> Result<(), VmError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            debug_print(format!("Unable to stat '{path}'"));
            set_errno(uc, ENOENT)?;
            uc.reg_write(RegisterARM64::X0, u64::MAX)?;
            return Ok(());
        }
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        write_python_stat(
            uc,
            out_ptr,
            metadata.mode(),
            metadata.size(),
            metadata.blksize(),
            metadata.blocks(),
        )?;
    }

    #[cfg(not(unix))]
    {
        write_python_stat(uc, out_ptr, 0, metadata.len(), 0, 0)?;
    }

    uc.reg_write(RegisterARM64::X0, 0)?;
    Ok(())
}

fn stat_fd_into_guest(
    uc: &mut Unicorn<'_, RuntimeState>,
    fd: u64,
    out_ptr: u64,
) -> Result<(), VmError> {
    let fd_index = usize::try_from(fd).map_err(|_| VmError::InvalidFileDescriptor(fd))?;

    let metadata = {
        let state = uc.get_data_mut();
        let slot = state
            .file_handles
            .get_mut(fd_index)
            .ok_or(VmError::InvalidFileDescriptor(fd))?;
        let file = slot.as_mut().ok_or(VmError::InvalidFileDescriptor(fd))?;
        file.metadata()
    };

    let metadata = match metadata {
        Ok(metadata) => metadata,
        Err(_) => {
            debug_print(format!("Unable to stat '{fd}'"));
            set_errno(uc, ENOENT)?;
            uc.reg_write(RegisterARM64::X0, u64::MAX)?;
            return Ok(());
        }
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        write_python_stat(
            uc,
            out_ptr,
            metadata.mode(),
            metadata.size(),
            metadata.blksize(),
            metadata.blocks(),
        )?;
    }

    #[cfg(not(unix))]
    {
        write_python_stat(uc, out_ptr, 0, metadata.len(), 0, 0)?;
    }

    uc.reg_write(RegisterARM64::X0, 0)?;
    Ok(())
}
fn stub_lstat(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let path_ptr = uc.reg_read(RegisterARM64::X0)?;
    let out_ptr = uc.reg_read(RegisterARM64::X1)?;
    let path = read_c_string(uc, path_ptr, 0x1000)?;
    debug_trace(format!(
        "lstat(0x{path_ptr:X}:'{path}', [x1:0x{out_ptr:X}])"
    ));
    stat_path_into_guest(uc, &path, out_ptr)
}

fn stub_fstat(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let fd = uc.reg_read(RegisterARM64::X0)?;
    let out_ptr = uc.reg_read(RegisterARM64::X1)?;
    debug_trace(format!("fstat({fd}, [...])"));
    stat_fd_into_guest(uc, fd, out_ptr)
}

fn stub_open(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let path_ptr = uc.reg_read(RegisterARM64::X0)?;
    let flags = uc.reg_read(RegisterARM64::X1)?;
    let mode = uc.reg_read(RegisterARM64::X2)?;
    let path = read_c_string(uc, path_ptr, 0x1000)?;
    if path.is_empty() {
        return Err(VmError::EmptyPath);
    }

    debug_trace(format!("open('{path}', {flags:#o}, {mode:#o})"));
    // Only allow access to ./anisette/adi.pb (matches Python reference impl)
    if path != "./anisette/adi.pb" {
        debug_print(format!("open: rejecting invalid path '{path}'"));
        set_errno(uc, ENOENT)?;
        uc.reg_write(RegisterARM64::X0, u64::MAX)?;
        return Ok(());
    }

    if flags != O_NOFOLLOW && flags != (O_NOFOLLOW | O_CREAT | O_WRONLY) {
        debug_print(format!("open: rejecting unsupported flags {flags:#o}"));
        set_errno(uc, ENOENT)?;
        uc.reg_write(RegisterARM64::X0, u64::MAX)?;
        return Ok(());
    }

    let mut options = OpenOptions::new();
    let access_mode = flags & O_ACCMODE;
    let _write_only = access_mode == O_WRONLY;
    let create = (flags & O_CREAT) != 0;

    match access_mode {
        0 => {
            options.read(true);
        }
        O_WRONLY => {
            options.write(true).truncate(true);
        }
        O_RDWR => {
            options.read(true).write(true);
        }
        _ => {
            set_errno(uc, ENOENT)?;
            uc.reg_write(RegisterARM64::X0, u64::MAX)?;
            return Ok(());
        }
    }

    if create {
        options.create(true).read(true).write(true);
        if let Some(parent) = std::path::Path::new(&path).parent() {
            let _ = fs::create_dir_all(parent);
        }
    }

    if (flags & O_NOFOLLOW) == 0 {
        debug_trace("open without O_NOFOLLOW");
    }

    match options.open(&path) {
        Ok(file) => {
            let fd = {
                let state = uc.get_data_mut();
                state.file_handles.push(Some(file));
                (state.file_handles.len() - 1) as u64
            };

            uc.reg_write(RegisterARM64::X0, fd)?;
        }
        Err(_) => {
            set_errno(uc, ENOENT)?;
            uc.reg_write(RegisterARM64::X0, u64::MAX)?;
        }
    }

    Ok(())
}

fn stub_ftruncate(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let fd = uc.reg_read(RegisterARM64::X0)?;
    let length = uc.reg_read(RegisterARM64::X1)?;
    debug_trace(format!("ftruncate({fd}, {length})"));
    let fd_index = usize::try_from(fd).map_err(|_| VmError::InvalidFileDescriptor(fd))?;

    let result = {
        let state = uc.get_data_mut();
        let slot = state
            .file_handles
            .get_mut(fd_index)
            .ok_or(VmError::InvalidFileDescriptor(fd))?;
        let file = slot.as_mut().ok_or(VmError::InvalidFileDescriptor(fd))?;
        file.set_len(length)
    };

    match result {
        Ok(()) => uc.reg_write(RegisterARM64::X0, 0)?,
        Err(_) => {
            set_errno(uc, ENOENT)?;
            uc.reg_write(RegisterARM64::X0, u64::MAX)?;
        }
    }

    Ok(())
}

fn stub_read(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let fd = uc.reg_read(RegisterARM64::X0)?;
    let buf_ptr = uc.reg_read(RegisterARM64::X1)?;
    let count = uc.reg_read(RegisterARM64::X2)? as usize;

    let fd_index = usize::try_from(fd).map_err(|_| VmError::InvalidFileDescriptor(fd))?;

    let mut buffer = vec![0_u8; count];

    let read_size = {
        let state = uc.get_data_mut();
        let slot = state
            .file_handles
            .get_mut(fd_index)
            .ok_or(VmError::InvalidFileDescriptor(fd))?;
        let file = slot.as_mut().ok_or(VmError::InvalidFileDescriptor(fd))?;
        file.read(&mut buffer)
    };
    debug_trace(format!("read({fd}, 0x{buf_ptr:X}, {count})={read_size:?}"));
    match read_size {
        Ok(read_size) => {
            uc.mem_write(buf_ptr, &buffer[..read_size])?;
            uc.reg_write(RegisterARM64::X0, read_size as u64)?;
        }
        Err(_) => {
            set_errno(uc, ENOENT)?;
            uc.reg_write(RegisterARM64::X0, u64::MAX)?;
        }
    }

    Ok(())
}

fn stub_write(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let fd = uc.reg_read(RegisterARM64::X0)?;
    let buf_ptr = uc.reg_read(RegisterARM64::X1)?;
    let count = uc.reg_read(RegisterARM64::X2)? as usize;
    debug_trace(format!("write({fd}, 0x{buf_ptr:X}, {count})"));
    let fd_index = usize::try_from(fd).map_err(|_| VmError::InvalidFileDescriptor(fd))?;

    let bytes = uc.mem_read_as_vec(buf_ptr, count)?;

    let write_size = {
        let state = uc.get_data_mut();
        let slot = state
            .file_handles
            .get_mut(fd_index)
            .ok_or(VmError::InvalidFileDescriptor(fd))?;
        let file = slot.as_mut().ok_or(VmError::InvalidFileDescriptor(fd))?;
        file.write_all(&bytes)
    };

    match write_size {
        Ok(()) => uc.reg_write(RegisterARM64::X0, count as u64)?,
        Err(_) => {
            set_errno(uc, ENOENT)?;
            uc.reg_write(RegisterARM64::X0, u64::MAX)?;
        }
    }

    Ok(())
}

fn stub_close(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let fd = uc.reg_read(RegisterARM64::X0)?;
    let fd_index = usize::try_from(fd).map_err(|_| VmError::InvalidFileDescriptor(fd))?;

    let state = uc.get_data_mut();
    let slot = state
        .file_handles
        .get_mut(fd_index)
        .ok_or(VmError::InvalidFileDescriptor(fd))?;
    *slot = None;

    uc.reg_write(RegisterARM64::X0, 0)?;
    Ok(())
}

fn stub_dlopen(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let path_ptr = uc.reg_read(RegisterARM64::X0)?;
    let path = read_c_string(uc, path_ptr, 0x1000)?;

    let library_name = path.rsplit('/').next().ok_or(VmError::EmptyPath)?;
    debug_trace(format!("dlopen('{path}' ({library_name}))"));
    let library_index = load_library_by_name(uc, library_name)?;

    uc.reg_write(RegisterARM64::X0, (library_index + 1) as u64)?;
    Ok(())
}

fn stub_dlsym(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let handle = uc.reg_read(RegisterARM64::X0)?;
    if handle == 0 {
        return Err(VmError::InvalidDlopenHandle(handle));
    }

    let symbol_ptr = uc.reg_read(RegisterARM64::X1)?;
    let symbol_name = read_c_string(uc, symbol_ptr, 0x1000)?;
    let library_index = (handle - 1) as usize;

    {
        let state = uc.get_data();
        if let Some(library) = state.loaded_libraries.get(library_index) {
            debug_trace(format!(
                "dlsym({handle:X} ({}), '{}')",
                library.name, symbol_name
            ));
        }
    }

    let symbol_address =
        resolve_symbol_from_loaded_library_by_name(uc, library_index, &symbol_name)?;
    debug_print(format!("Found at 0x{symbol_address:X}"));
    uc.reg_write(RegisterARM64::X0, symbol_address)?;
    Ok(())
}

fn stub_dlclose(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    uc.reg_write(RegisterARM64::X0, 0)?;
    Ok(())
}

fn stub_gettimeofday(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let time_ptr = uc.reg_read(RegisterARM64::X0)?;
    let tz_ptr = uc.reg_read(RegisterARM64::X1)?;
    debug_trace(format!("gettimeofday(0x{time_ptr:X}, 0x{tz_ptr:X})"));
    if tz_ptr != 0 {
        return Err(VmError::UnhandledImport(format!(
            "gettimeofday tz pointer must be null, got 0x{tz_ptr:X}"
        )));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let sec = now.as_secs();
    let usec = now.subsec_micros() as i64;

    let mut timeval = [0_u8; 16];
    timeval[0..8].copy_from_slice(&sec.to_le_bytes());
    timeval[8..16].copy_from_slice(&usec.to_le_bytes());
    debug_print(format!(
        "{{'tv_sec': {sec}, 'tv_usec': {usec}}} {} {}",
        bytes_to_hex(&timeval),
        timeval.len()
    ));

    uc.mem_write(time_ptr, &timeval)?;
    uc.reg_write(RegisterARM64::X0, 0)?;

    Ok(())
}

fn stub_errno_location(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    if uc.get_data().errno_address.is_none() {
        debug_print("Checking errno before first error (!)");
    }
    let errno_address = ensure_errno_address(uc)?;
    uc.reg_write(RegisterARM64::X0, errno_address)?;
    Ok(())
}

fn stub_system_property_get(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    let name_ptr = uc.reg_read(RegisterARM64::X0)?;
    let name = read_c_string(uc, name_ptr, 0x1000)?;
    debug_trace(format!("__system_property_get({name}, [...])"));
    let value_ptr = uc.reg_read(RegisterARM64::X1)?;
    let value = b"no s/n number";
    uc.mem_write(value_ptr, value)?;
    uc.reg_write(RegisterARM64::X0, value.len() as u64)?;
    Ok(())
}

fn stub_arc4random(uc: &mut Unicorn<'_, RuntimeState>) -> Result<(), VmError> {
    uc.reg_write(RegisterARM64::X0, 0xDEAD_BEEF)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Allocator;

    #[test]
    fn allocator_aligns_to_pages() {
        let mut allocator = Allocator::new(0x1000_0000, 0x20_000);
        let a = allocator.alloc(1).expect("alloc 1");
        let b = allocator.alloc(0x1500).expect("alloc 2");

        assert_eq!(a, 0x1000_0000);
        assert_eq!(b, 0x1000_1000);
    }
}
