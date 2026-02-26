use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const DEFAULT_CLIENT_INFO: &str =
    "<MacBookPro13,2> <macOS;13.1;22C65> <com.apple.AuthKit/1 (com.apple.dt.Xcode/3594.4.19)>";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceData {
    #[serde(rename = "UUID")]
    pub unique_device_identifier: String,
    #[serde(rename = "clientInfo")]
    pub server_friendly_description: String,
    #[serde(rename = "identifier")]
    pub adi_identifier: String,
    #[serde(rename = "localUUID")]
    pub local_user_uuid: String,
}

#[derive(Debug, Clone)]
pub struct Device {
    path: PathBuf,
    pub data: DeviceData,
    pub initialized: bool,
}

impl Device {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        if !path.exists() {
            return Ok(Self {
                path,
                data: DeviceData::default(),
                initialized: false,
            });
        }

        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read device file {}", path.display()))?;
        let data: DeviceData = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse device file {}", path.display()))?;

        Ok(Self {
            path,
            data,
            initialized: true,
        })
    }

    pub fn initialize_defaults(&mut self) {
        self.data.server_friendly_description = DEFAULT_CLIENT_INFO.to_string();
        self.data.unique_device_identifier = Uuid::new_v4().to_string().to_uppercase();
        self.data.adi_identifier = random_hex(8, false);
        self.data.local_user_uuid = random_hex(32, true);
        self.initialized = true;
    }

    pub fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create parent dir {}", parent.display()))?;
        }

        let bytes = serde_json::to_vec_pretty(&self.data)?;
        fs::write(&self.path, bytes)
            .with_context(|| format!("failed to write device file {}", self.path.display()))?;

        Ok(())
    }
}

fn random_hex(byte_len: usize, uppercase: bool) -> String {
    let mut bytes = vec![0_u8; byte_len];
    rand::thread_rng().fill_bytes(&mut bytes);

    let mut output = String::with_capacity(byte_len * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }

    if uppercase {
        output.make_ascii_uppercase();
    }

    output
}
