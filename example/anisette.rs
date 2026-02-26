use std::fs;
use std::path::{Path, PathBuf};

use anisette_rs::{Adi, AdiInit, Device, ProvisioningSession, init_idbfs_for_path, sync_idbfs};
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;

fn main() -> Result<()> {
    // Usage:
    // cargo run --example anisette -- <libstoreservicescore.so> <libCoreADI.so> <library_path> [dsid] [apple_root_pem]
    let storeservices_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "libstoreservicescore.so".to_string());
    let coreadi_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "libCoreADI.so".to_string());
    let library_path = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "./anisette/".to_string());
    let dsid_raw = std::env::args().nth(4).unwrap_or_else(|| "-2".to_string());
    let apple_root_pem = std::env::args().nth(5).map(PathBuf::from);

    let _mount_path = init_idbfs_for_path(&library_path)
        .map_err(|e| anyhow::anyhow!("failed to initialize IDBFS: {e}"))?;

    fs::create_dir_all(&library_path)
        .with_context(|| format!("failed to create library path: {library_path}"))?;

    let device_path = Path::new(&library_path).join("device.json");
    let mut device = Device::load(&device_path)?;

    let storeservicescore = fs::read(&storeservices_path)
        .with_context(|| format!("failed to read {storeservices_path}"))?;
    let coreadi =
        fs::read(&coreadi_path).with_context(|| format!("failed to read {coreadi_path}"))?;

    let mut adi = Adi::new(AdiInit {
        storeservicescore,
        coreadi,
        library_path: library_path.clone(),
        provisioning_path: Some(library_path.clone()),
        identifier: None,
    })?;

    if !device.initialized {
        println!("Initializing device");
        device.initialize_defaults();
        device.persist()?;
    } else {
        println!(
            "(Device initialized: server-description='{}' device-uid='{}' adi='{}' user-uid='{}')",
            device.data.server_friendly_description,
            device.data.unique_device_identifier,
            device.data.adi_identifier,
            device.data.local_user_uuid
        );
    }

    adi.set_identifier(&device.data.adi_identifier)?;

    let dsid = if let Some(hex) = dsid_raw.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)?
    } else {
        let signed: i64 = dsid_raw.parse()?;
        signed as u64
    };

    let is_provisioned = adi.is_machine_provisioned(dsid)?;

    if !is_provisioned {
        println!("Provisioning...");
        let mut provisioning_session =
            ProvisioningSession::new(&mut adi, &device.data, apple_root_pem)?;
        provisioning_session.provision(dsid)?;
    } else {
        println!("(Already provisioned)");
    }

    let otp = adi.request_otp(dsid)?;
    let headers = json!({
      "X-Apple-I-MD": STANDARD.encode(otp.otp),
      "X-Apple-I-MD-M": STANDARD.encode(otp.machine_id),
    });

    let _ = sync_idbfs(false);
    println!("{}", serde_json::to_string_pretty(&headers)?);
    Ok(())
}
