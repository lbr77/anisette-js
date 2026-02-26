pub mod device;
mod exports;
pub mod idbfs;
#[cfg(not(target_arch = "wasm32"))]
pub mod provisioning;
#[cfg(target_arch = "wasm32")]
mod provisioning_wasm;

mod adi;
mod allocator;
mod constants;
mod debug;
mod emu;
mod errors;
mod runtime;
mod stub;
mod util;

pub use adi::{Adi, AdiInit, OtpResult, ProvisioningStartResult};
pub use allocator::Allocator;
pub use device::{Device, DeviceData};
pub use emu::EmuCore;
pub use errors::VmError;
pub use idbfs::{init_idbfs_for_path, sync_idbfs};
#[cfg(not(target_arch = "wasm32"))]
pub use provisioning::ProvisioningSession;
#[cfg(target_arch = "wasm32")]
pub use provisioning_wasm::ProvisioningSession;
