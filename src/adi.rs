use crate::debug::debug_print;
use crate::emu::{EmuCore, alloc_c_string, ensure_zero_return};
use crate::errors::VmError;
use crate::util::bytes_to_hex;

pub struct AdiInit {
    pub storeservicescore: Vec<u8>,
    pub coreadi: Vec<u8>,
    pub library_path: String,
    pub provisioning_path: Option<String>,
    pub identifier: Option<String>,
}

pub struct ProvisioningStartResult {
    pub cpim: Vec<u8>,
    pub session: u32,
}

pub struct OtpResult {
    pub otp: Vec<u8>,
    pub machine_id: Vec<u8>,
}

pub struct Adi {
    core: EmuCore,
    p_load_library_with_path: u64,
    p_set_android_id: u64,
    p_set_provisioning_path: u64,
    p_get_login_code: u64,
    p_provisioning_start: u64,
    p_provisioning_end: u64,
    p_otp_request: u64,
}

impl Adi {
    pub fn new(init: AdiInit) -> Result<Self, VmError> {
        debug_print(format!("Constructing ADI for '{}'", init.library_path));
        let mut core = EmuCore::new_arm64()?;
        core.set_library_root(&init.library_path);
        core.register_library_blob("libstoreservicescore.so", init.storeservicescore);
        core.register_library_blob("libCoreADI.so", init.coreadi);

        let storeservices_idx = core.load_library("libstoreservicescore.so")?;

        debug_print("Loading Android-specific symbols...");
        let p_load_library_with_path =
            core.resolve_symbol_by_name(storeservices_idx, "kq56gsgHG6")?;
        let p_set_android_id = core.resolve_symbol_by_name(storeservices_idx, "Sph98paBcz")?;
        let p_set_provisioning_path =
            core.resolve_symbol_by_name(storeservices_idx, "nf92ngaK92")?;

        debug_print("Loading ADI symbols...");
        let p_get_login_code = core.resolve_symbol_by_name(storeservices_idx, "aslgmuibau")?;
        let p_provisioning_start = core.resolve_symbol_by_name(storeservices_idx, "rsegvyrt87")?;
        let p_provisioning_end = core.resolve_symbol_by_name(storeservices_idx, "uv5t6nhkui")?;
        let p_otp_request = core.resolve_symbol_by_name(storeservices_idx, "qi864985u0")?;

        let mut adi = Self {
            core,
            p_load_library_with_path,
            p_set_android_id,
            p_set_provisioning_path,
            p_get_login_code,
            p_provisioning_start,
            p_provisioning_end,
            p_otp_request,
        };

        adi.load_library_with_path(&init.library_path)?;

        if let Some(provisioning_path) = init.provisioning_path.as_deref() {
            adi.set_provisioning_path(provisioning_path)?;
        }

        if let Some(identifier) = init.identifier.as_deref() {
            adi.set_identifier(identifier)?;
        }

        Ok(adi)
    }

    pub fn set_identifier(&mut self, identifier: &str) -> Result<(), VmError> {
        if identifier.is_empty() {
            debug_print("Skipping empty identifier");
            return Ok(());
        }
        debug_print(format!("Setting identifier {identifier}"));
        let bytes = identifier.as_bytes();
        let p_identifier = self.core.alloc_data(bytes)?;
        let ret = self
            .core
            .invoke_cdecl(self.p_set_android_id, &[p_identifier, bytes.len() as u64])?;
        debug_print(format!(
            "{}: {:X}={}",
            "pADISetAndroidID", ret, ret as u32 as i32
        ));
        ensure_zero_return("ADISetAndroidID", ret)
    }

    pub fn set_provisioning_path(&mut self, path: &str) -> Result<(), VmError> {
        let p_path = alloc_c_string(&mut self.core, path)?;
        let ret = self
            .core
            .invoke_cdecl(self.p_set_provisioning_path, &[p_path])?;
        ensure_zero_return("ADISetProvisioningPath", ret)
    }

    pub fn load_library_with_path(&mut self, path: &str) -> Result<(), VmError> {
        let p_path = alloc_c_string(&mut self.core, path)?;
        let ret = self
            .core
            .invoke_cdecl(self.p_load_library_with_path, &[p_path])?;
        ensure_zero_return("ADILoadLibraryWithPath", ret)
    }
    pub fn start_provisioning(
        &mut self,
        dsid: u64,
        server_provisioning_intermediate_metadata: &[u8],
    ) -> Result<ProvisioningStartResult, VmError> {
        debug_print("ADI.start_provisioning");
        let p_cpim = self.core.alloc_temporary(8)?;
        let p_cpim_len = self.core.alloc_temporary(4)?;
        let p_session = self.core.alloc_temporary(4)?;
        let p_spim = self
            .core
            .alloc_data(server_provisioning_intermediate_metadata)?;

        debug_print(format!("0x{dsid:X}"));
        debug_print(bytes_to_hex(server_provisioning_intermediate_metadata));

        let ret = self.core.invoke_cdecl(
            self.p_provisioning_start,
            &[
                dsid,
                p_spim,
                server_provisioning_intermediate_metadata.len() as u64,
                p_cpim,
                p_cpim_len,
                p_session,
            ],
        )?;
        debug_print(format!(
            "{}: {:X}={}",
            "pADIProvisioningStart", ret, ret as u32 as i32
        ));
        ensure_zero_return("ADIProvisioningStart", ret)?;

        let cpim_ptr = self.core.read_u64(p_cpim)?;
        let cpim_len = self.core.read_u32(p_cpim_len)? as usize;
        let cpim = self.core.read_data(cpim_ptr, cpim_len)?;
        let session = self.core.read_u32(p_session)?;

        debug_print(format!("Wrote data to 0x{cpim_ptr:X}"));
        debug_print(format!("{} {} {}", cpim_len, bytes_to_hex(&cpim), session));

        Ok(ProvisioningStartResult { cpim, session })
    }

    pub fn is_machine_provisioned(&mut self, dsid: u64) -> Result<bool, VmError> {
        debug_print("ADI.is_machine_provisioned");
        let ret = self.core.invoke_cdecl(self.p_get_login_code, &[dsid])?;
        let code = ret as u32 as i32;

        if code == 0 {
            return Ok(true);
        }
        if code == -45061 {
            return Ok(false);
        }

        debug_print(format!(
            "Unknown errorCode in is_machine_provisioned: {code}=0x{code:X}"
        ));

        Err(VmError::AdiCallFailed {
            name: "ADIGetLoginCode",
            code,
        })
    }

    pub fn end_provisioning(
        &mut self,
        session: u32,
        persistent_token_metadata: &[u8],
        trust_key: &[u8],
    ) -> Result<(), VmError> {
        let p_ptm = self.core.alloc_data(persistent_token_metadata)?;
        let p_tk = self.core.alloc_data(trust_key)?;

        let ret = self.core.invoke_cdecl(
            self.p_provisioning_end,
            &[
                session as u64,
                p_ptm,
                persistent_token_metadata.len() as u64,
                p_tk,
                trust_key.len() as u64,
            ],
        )?;

        debug_print(format!("0x{session:X}"));
        debug_print(format!(
            "{} {}",
            bytes_to_hex(persistent_token_metadata),
            persistent_token_metadata.len()
        ));
        debug_print(format!("{} {}", bytes_to_hex(trust_key), trust_key.len()));
        debug_print(format!(
            "{}: {:X}={}",
            "pADIProvisioningEnd", ret, ret as u32 as i32
        ));

        ensure_zero_return("ADIProvisioningEnd", ret)
    }

    pub fn request_otp(&mut self, dsid: u64) -> Result<OtpResult, VmError> {
        debug_print("ADI.request_otp");
        let p_otp = self.core.alloc_temporary(8)?;
        let p_otp_len = self.core.alloc_temporary(4)?;
        let p_mid = self.core.alloc_temporary(8)?;
        let p_mid_len = self.core.alloc_temporary(4)?;

        let ret = self.core.invoke_cdecl(
            self.p_otp_request,
            &[dsid, p_mid, p_mid_len, p_otp, p_otp_len],
        )?;
        debug_print(format!(
            "{}: {:X}={}",
            "pADIOTPRequest", ret, ret as u32 as i32
        ));
        ensure_zero_return("ADIOTPRequest", ret)?;

        let otp_ptr = self.core.read_u64(p_otp)?;
        let otp_len = self.core.read_u32(p_otp_len)? as usize;
        let otp = self.core.read_data(otp_ptr, otp_len)?;

        let mid_ptr = self.core.read_u64(p_mid)?;
        let mid_len = self.core.read_u32(p_mid_len)? as usize;
        let machine_id = self.core.read_data(mid_ptr, mid_len)?;

        Ok(OtpResult { otp, machine_id })
    }
}
