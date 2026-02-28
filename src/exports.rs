use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::fs;
use std::path::Path;

use crate::{Adi, AdiInit, sync_idbfs};

#[derive(Default)]
struct ExportState {
    adi: Option<Adi>,
    last_error: String,
    cpim: Vec<u8>,
    session: u32,
    otp: Vec<u8>,
    mid: Vec<u8>,
    read_buf: Vec<u8>,
}

thread_local! {
  static STATE: RefCell<ExportState> = RefCell::new(ExportState::default());
}

fn set_last_error(message: impl Into<String>) {
    STATE.with(|state| {
        state.borrow_mut().last_error = message.into();
    });
}

fn clear_last_error() {
    STATE.with(|state| {
        state.borrow_mut().last_error.clear();
    });
}

unsafe fn c_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null C string pointer".to_string());
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map(|s| s.to_string())
        .map_err(|e| format!("invalid utf-8 string: {e}"))
}

unsafe fn optional_c_string(ptr: *const c_char) -> Result<Option<String>, String> {
    if ptr.is_null() {
        return Ok(None);
    }
    unsafe { c_string(ptr).map(Some) }
}

unsafe fn input_bytes(ptr: *const u8, len: usize) -> Result<Vec<u8>, String> {
    if len == 0 {
        return Ok(Vec::new());
    }
    if ptr.is_null() {
        return Err("null bytes pointer with non-zero length".to_string());
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec())
}

fn with_adi_mut<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&mut Adi) -> Result<T, String>,
{
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let adi = state
            .adi
            .as_mut()
            .ok_or_else(|| "ADI is not initialized".to_string())?;
        f(adi)
    })
}

fn install_adi(adi: Adi) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.adi = Some(adi);
        state.cpim.clear();
        state.otp.clear();
        state.mid.clear();
        state.session = 0;
    });
}

fn init_adi_from_parts(
    storeservicescore: Vec<u8>,
    coreadi: Vec<u8>,
    library_path: String,
    provisioning_path: Option<String>,
    identifier: Option<String>,
) -> Result<(), String> {
    let adi = Adi::new(AdiInit {
        storeservicescore,
        coreadi,
        library_path,
        provisioning_path,
        identifier,
    })
    .map_err(|e| format!("ADI init failed: {e}"))?;

    install_adi(adi);
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_init_from_files(
    storeservices_path: *const c_char,
    coreadi_path: *const c_char,
    library_path: *const c_char,
    provisioning_path: *const c_char,
    identifier: *const c_char,
) -> i32 {
    let result = (|| -> Result<(), String> {
        let storeservices_path = unsafe { c_string(storeservices_path)? };
        let coreadi_path = unsafe { c_string(coreadi_path)? };
        let library_path = unsafe { c_string(library_path)? };
        let provisioning_path = unsafe { optional_c_string(provisioning_path)? };
        let identifier = unsafe { optional_c_string(identifier)? };

        let storeservicescore = fs::read(&storeservices_path).map_err(|e| {
            format!(
                "failed to read storeservices core '{}': {e}",
                storeservices_path
            )
        })?;
        let coreadi = fs::read(&coreadi_path)
            .map_err(|e| format!("failed to read coreadi '{}': {e}", coreadi_path))?;

        init_adi_from_parts(
            storeservicescore,
            coreadi,
            library_path,
            provisioning_path,
            identifier,
        )
    })();

    match result {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_init_from_blobs(
    storeservices_ptr: *const u8,
    storeservices_len: usize,
    coreadi_ptr: *const u8,
    coreadi_len: usize,
    library_path: *const c_char,
    provisioning_path: *const c_char,
    identifier: *const c_char,
) -> i32 {
    let result = (|| -> Result<(), String> {
        let storeservicescore = unsafe { input_bytes(storeservices_ptr, storeservices_len)? };
        let coreadi = unsafe { input_bytes(coreadi_ptr, coreadi_len)? };
        let library_path = unsafe { c_string(library_path)? };
        let provisioning_path = unsafe { optional_c_string(provisioning_path)? };
        let identifier = unsafe { optional_c_string(identifier)? };

        init_adi_from_parts(
            storeservicescore,
            coreadi,
            library_path,
            provisioning_path,
            identifier,
        )
    })();

    match result {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_set_identifier(identifier: *const c_char) -> i32 {
    let result = (|| -> Result<(), String> {
        let identifier = unsafe { c_string(identifier)? };
        with_adi_mut(|adi| adi.set_identifier(&identifier).map_err(|e| e.to_string()))
    })();

    match result {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_set_provisioning_path(path: *const c_char) -> i32 {
    let result = (|| -> Result<(), String> {
        let path = unsafe { c_string(path)? };
        with_adi_mut(|adi| adi.set_provisioning_path(&path).map_err(|e| e.to_string()))
    })();

    match result {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}



#[unsafe(no_mangle)]
pub extern "C" fn anisette_is_machine_provisioned(dsid: u64) -> i32 {
    let result = (|| -> Result<i32, String> {
        let mut out = -1;
        with_adi_mut(|adi| {
            let provisioned = adi
                .is_machine_provisioned(dsid)
                .map_err(|e| e.to_string())?;
            out = if provisioned { 1 } else { 0 };
            Ok(())
        })?;
        Ok(out)
    })();

    match result {
        Ok(value) => {
            clear_last_error();
            value
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_start_provisioning(
    dsid: u64,
    spim_ptr: *const u8,
    spim_len: usize,
) -> i32 {
    let result = (|| -> Result<(), String> {
        let spim = unsafe { input_bytes(spim_ptr, spim_len)? };
        let out = with_adi_mut(|adi| {
            adi.start_provisioning(dsid, &spim)
                .map_err(|e| format!("start_provisioning failed: {e}"))
        })?;
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.cpim = out.cpim;
            state.session = out.session;
        });
        Ok(())
    })();

    match result {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_get_cpim_ptr() -> *const u8 {
    STATE.with(|state| state.borrow().cpim.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_get_cpim_len() -> usize {
    STATE.with(|state| state.borrow().cpim.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_get_session() -> u32 {
    STATE.with(|state| state.borrow().session)
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_end_provisioning(
    session: u32,
    ptm_ptr: *const u8,
    ptm_len: usize,
    tk_ptr: *const u8,
    tk_len: usize,
) -> i32 {
    let result = (|| -> Result<(), String> {
        let ptm = unsafe { input_bytes(ptm_ptr, ptm_len)? };
        let tk = unsafe { input_bytes(tk_ptr, tk_len)? };
        with_adi_mut(|adi| {
            adi.end_provisioning(session, &ptm, &tk)
                .map_err(|e| format!("end_provisioning failed: {e}"))
        })
    })();

    match result {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_request_otp(dsid: u64) -> i32 {
    let result = (|| -> Result<(), String> {
        let out = with_adi_mut(|adi| {
            adi.request_otp(dsid)
                .map_err(|e| format!("request_otp failed: {e:#}"))
        })?;
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.otp = out.otp;
            state.mid = out.machine_id;
        });
        Ok(())
    })();

    match result {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_get_otp_ptr() -> *const u8 {
    STATE.with(|state| state.borrow().otp.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_get_otp_len() -> usize {
    STATE.with(|state| state.borrow().otp.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_get_mid_ptr() -> *const u8 {
    STATE.with(|state| state.borrow().mid.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_get_mid_len() -> usize {
    STATE.with(|state| state.borrow().mid.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_fs_write_file(
    path: *const c_char,
    data_ptr: *const u8,
    data_len: usize,
) -> i32 {
    let result = (|| -> Result<(), String> {
        let path = unsafe { c_string(path)? };
        let data = unsafe { input_bytes(data_ptr, data_len)? };
        let path_ref = Path::new(&path);
        if let Some(parent) = path_ref.parent()
            && !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("failed to create dir '{}': {e}", parent.display()))?;
            }
        fs::write(&path, data).map_err(|e| format!("failed to write '{path}': {e}"))?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_fs_read_file(path: *const c_char) -> i32 {
    let result = (|| -> Result<Vec<u8>, String> {
        let path = unsafe { c_string(path)? };
        fs::read(&path).map_err(|e| format!("failed to read '{path}': {e}"))
    })();

    match result {
        Ok(data) => {
            STATE.with(|state| state.borrow_mut().read_buf = data);
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_fs_read_ptr() -> *const u8 {
    STATE.with(|state| state.borrow().read_buf.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_fs_read_len() -> usize {
    STATE.with(|state| state.borrow().read_buf.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_idbfs_sync(populate_from_storage: i32) -> i32 {
    let result = sync_idbfs(populate_from_storage != 0);
    match result {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(err) => {
            set_last_error(err);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_last_error_ptr() -> *const u8 {
    STATE.with(|state| state.borrow().last_error.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn anisette_last_error_len() -> usize {
    STATE.with(|state| state.borrow().last_error.len())
}
