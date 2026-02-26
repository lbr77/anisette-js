use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::Local;
use plist::Value;
use reqwest::Certificate;
use reqwest::blocking::{Client, RequestBuilder};

use crate::Adi;
use crate::device::DeviceData;

pub struct ProvisioningSession<'a> {
    adi: &'a mut Adi,
    device: &'a DeviceData,
    client: Client,
    url_bag: HashMap<String, String>,
}

impl<'a> ProvisioningSession<'a> {
    pub fn new(
        adi: &'a mut Adi,
        device: &'a DeviceData,
        apple_root_pem: Option<PathBuf>,
    ) -> Result<Self> {
        let client = build_http_client(apple_root_pem.as_deref())?;

        Ok(Self {
            adi,
            device,
            client,
            url_bag: HashMap::new(),
        })
    }

    pub fn provision(&mut self, dsid: u64) -> Result<()> {
        println!("ProvisioningSession.provision");
        if self.url_bag.is_empty() {
            self.load_url_bag()?;
        }

        let start_url = self
            .url_bag
            .get("midStartProvisioning")
            .cloned()
            .ok_or_else(|| anyhow!("url bag missing midStartProvisioning"))?;

        let finish_url = self
            .url_bag
            .get("midFinishProvisioning")
            .cloned()
            .ok_or_else(|| anyhow!("url bag missing midFinishProvisioning"))?;

        let start_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Header</key>
  <dict/>
  <key>Request</key>
  <dict/>
</dict>
</plist>"#;

        let start_bytes = self.post_with_time(&start_url, start_body)?;
        let start_plist = parse_plist(&start_bytes)?;

        let spim_b64 = plist_get_string_in_response(&start_plist, "spim")?;
        println!("{spim_b64}");
        let spim = STANDARD.decode(spim_b64.as_bytes())?;

        let start = self.adi.start_provisioning(dsid, &spim)?;
        println!("{}", bytes_to_hex(&start.cpim));
        let cpim_b64 = STANDARD.encode(&start.cpim);

        let finish_body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Header</key>\n  <dict/>\n  <key>Request</key>\n  <dict>\n    <key>cpim</key>\n    <string>{}</string>\n  </dict>\n</dict>\n</plist>",
            cpim_b64
        );

        let finish_bytes = self.post_with_time(&finish_url, &finish_body)?;
        let finish_plist = parse_plist(&finish_bytes)?;

        let ptm_b64 = plist_get_string_in_response(&finish_plist, "ptm")?;
        let tk_b64 = plist_get_string_in_response(&finish_plist, "tk")?;

        let ptm = STANDARD.decode(ptm_b64.as_bytes())?;
        let tk = STANDARD.decode(tk_b64.as_bytes())?;

        self.adi.end_provisioning(start.session, &ptm, &tk)?;
        Ok(())
    }

    fn load_url_bag(&mut self) -> Result<()> {
        let bytes = self.get("https://gsa.apple.com/grandslam/GsService2/lookup")?;
        let plist = parse_plist(&bytes)?;

        let root = plist
            .as_dictionary()
            .ok_or_else(|| anyhow!("lookup plist root is not a dictionary"))?;
        let urls = root
            .get("urls")
            .and_then(Value::as_dictionary)
            .ok_or_else(|| anyhow!("lookup plist missing urls dictionary"))?;

        self.url_bag.clear();
        for (name, value) in urls {
            if let Some(url) = value.as_string() {
                self.url_bag.insert(name.to_string(), url.to_string());
            }
        }

        Ok(())
    }

    fn get(&self, url: &str) -> Result<Vec<u8>> {
        let request = self.with_common_headers(self.client.get(url), None);
        let response = request.send()?.error_for_status()?;
        Ok(response.bytes()?.to_vec())
    }

    fn post_with_time(&self, url: &str, body: &str) -> Result<Vec<u8>> {
        let client_time = current_client_time();
        let request = self.with_common_headers(
            self.client.post(url).body(body.to_string()),
            Some(&client_time),
        );
        let response = request.send()?.error_for_status()?;
        Ok(response.bytes()?.to_vec())
    }

    fn with_common_headers(
        &self,
        request: RequestBuilder,
        client_time: Option<&str>,
    ) -> RequestBuilder {
        let mut request = request
            .header("User-Agent", "akd/1.0 CFNetwork/1404.0.5 Darwin/22.3.0")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Connection", "keep-alive")
            .header("X-Mme-Device-Id", &self.device.unique_device_identifier)
            .header(
                "X-MMe-Client-Info",
                &self.device.server_friendly_description,
            )
            .header("X-Apple-I-MD-LU", &self.device.local_user_uuid)
            .header("X-Apple-Client-App-Name", "Setup");

        if let Some(time) = client_time {
            request = request.header("X-Apple-I-Client-Time", time);
        }

        request
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn build_http_client(apple_root_pem: Option<&Path>) -> Result<Client> {
    let mut builder = Client::builder().timeout(Duration::from_secs(5));

    if let Some(cert) = load_apple_root_cert(apple_root_pem)? {
        builder = builder.add_root_certificate(cert);
    } else {
        eprintln!("warning: apple-root.pem not found, falling back to insecure TLS mode");
        builder = builder.danger_accept_invalid_certs(true);
    }

    Ok(builder.build()?)
}

fn load_apple_root_cert(explicit_path: Option<&Path>) -> Result<Option<Certificate>> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(path) = explicit_path {
        candidates.push(path.to_path_buf());
    }

    candidates.push(PathBuf::from("apple-root.pem"));
    candidates.push(PathBuf::from(
        "/Users/libr/Desktop/Life/Anisette.py/src/anisette/apple-root.pem",
    ));

    for candidate in candidates {
        if candidate.exists() {
            let pem = fs::read(&candidate)
                .with_context(|| format!("failed to read certificate {}", candidate.display()))?;
            let cert = Certificate::from_pem(&pem)
                .with_context(|| format!("invalid certificate pem {}", candidate.display()))?;
            return Ok(Some(cert));
        }
    }

    Ok(None)
}

fn parse_plist(bytes: &[u8]) -> Result<Value> {
    Ok(Value::from_reader_xml(Cursor::new(bytes))?)
}

fn plist_get_string_in_response<'a>(plist: &'a Value, key: &str) -> Result<&'a str> {
    let root = plist
        .as_dictionary()
        .ok_or_else(|| anyhow!("plist root is not a dictionary"))?;

    let response = root
        .get("Response")
        .and_then(Value::as_dictionary)
        .ok_or_else(|| anyhow!("plist missing Response dictionary"))?;

    let value = response
        .get(key)
        .ok_or_else(|| anyhow!("plist Response missing {key}"))?;

    if let Some(text) = value.as_string() {
        return Ok(text);
    }

    bail!("plist Response field {key} is not a string")
}

fn current_client_time() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}
