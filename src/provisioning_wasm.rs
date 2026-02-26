use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::io::Cursor;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::Utc;
use plist::Value;
use serde::Deserialize;
use serde_json::json;

use crate::Adi;
use crate::device::DeviceData;

#[derive(Debug, Deserialize)]
struct JsHttpResponse {
    status: u16,
    body: String,
    #[serde(default)]
    error: String,
}

pub struct ProvisioningSession<'a> {
    adi: &'a mut Adi,
    device: &'a DeviceData,
    url_bag: HashMap<String, String>,
}

impl<'a> ProvisioningSession<'a> {
    pub fn new(
        adi: &'a mut Adi,
        device: &'a DeviceData,
        _apple_root_pem: Option<PathBuf>,
    ) -> Result<Self> {
        Ok(Self {
            adi,
            device,
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
        let spim = STANDARD.decode(spim_b64.as_bytes())?;

        let start = self.adi.start_provisioning(dsid, &spim)?;
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
        let request = json!({
          "url": url,
          "headers": self.common_headers(None),
        });
        self.call_http("anisette_http_get", request)
    }

    fn post_with_time(&self, url: &str, body: &str) -> Result<Vec<u8>> {
        let client_time = current_client_time();
        let request = json!({
          "url": url,
          "headers": self.common_headers(Some(&client_time)),
          "body": body,
        });
        self.call_http("anisette_http_post", request)
    }

    fn common_headers(&self, client_time: Option<&str>) -> HashMap<&'static str, String> {
        let mut headers = HashMap::from([
            (
                "User-Agent",
                "akd/1.0 CFNetwork/1404.0.5 Darwin/22.3.0".to_string(),
            ),
            (
                "Content-Type",
                "application/x-www-form-urlencoded".to_string(),
            ),
            ("Connection", "keep-alive".to_string()),
            (
                "X-Mme-Device-Id",
                self.device.unique_device_identifier.clone(),
            ),
            (
                "X-MMe-Client-Info",
                self.device.server_friendly_description.clone(),
            ),
            ("X-Apple-I-MD-LU", self.device.local_user_uuid.clone()),
            ("X-Apple-Client-App-Name", "Setup".to_string()),
        ]);

        if let Some(time) = client_time {
            headers.insert("X-Apple-I-Client-Time", time.to_string());
        }

        headers
    }

    fn call_http(&self, name: &str, payload: serde_json::Value) -> Result<Vec<u8>> {
        // JS callback must return JSON: { status: number, body: base64, error?: string }.
        let payload_json = serde_json::to_string(&payload)?;
        let script = format!(
            "(function(){{var fn = (typeof {name} === 'function') ? {name} : (typeof Module !== 'undefined' ? Module.{name} : null); return fn ? fn({payload_json}) : '';}})();"
        );
        let response_json = run_script_string(&script)?;
        if response_json.trim().is_empty() {
            bail!("missing JS http callback {name}");
        }

        let response: JsHttpResponse = serde_json::from_str(&response_json)
            .with_context(|| format!("invalid JS http response for {name}"))?;
        if !response.error.trim().is_empty() {
            bail!("js http error: {}", response.error);
        }
        if response.status >= 400 {
            bail!("js http status {} for {}", response.status, name);
        }

        let bytes = STANDARD
            .decode(response.body.as_bytes())
            .map_err(|e| anyhow!("base64 decode failed: {e}"))?;
        Ok(bytes)
    }
}

#[cfg(target_os = "emscripten")]
unsafe extern "C" {
    fn emscripten_run_script_string(script: *const core::ffi::c_char) -> *mut core::ffi::c_char;
}

#[cfg(target_os = "emscripten")]
fn run_script_string(script: &str) -> Result<String> {
    let script = CString::new(script).map_err(|e| anyhow!("invalid JS script: {e}"))?;
    let ptr = unsafe { emscripten_run_script_string(script.as_ptr()) };
    if ptr.is_null() {
        return Err(anyhow!("emscripten_run_script_string returned null"));
    }
    let text = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    Ok(text)
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
    Utc::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}
