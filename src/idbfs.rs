#[cfg(target_os = "emscripten")]
use std::ffi::CString;

fn normalize_mount_path(path: &str) -> String {
    let trimmed = path.trim();
    let no_slash = trimmed.trim_end_matches('/');
    let no_dot = no_slash.strip_prefix("./").unwrap_or(no_slash);

    if no_dot.is_empty() {
        "/".to_string()
    } else if no_dot.starts_with('/') {
        no_dot.to_string()
    } else {
        format!("/{no_dot}")
    }
}

#[cfg(target_os = "emscripten")]
unsafe extern "C" {
    fn emscripten_run_script(script: *const core::ffi::c_char);
}

#[cfg(target_os = "emscripten")]
fn run_script(script: &str) -> Result<(), String> {
    let script = CString::new(script).map_err(|e| format!("invalid JS script: {e}"))?;
    unsafe {
        emscripten_run_script(script.as_ptr());
    }
    Ok(())
}

#[cfg(not(target_os = "emscripten"))]
fn run_script(_script: &str) -> Result<(), String> {
    Ok(())
}

pub fn init_idbfs_for_path(path: &str) -> Result<String, String> {
    let mount_path = normalize_mount_path(path);
    let script = format!(
        r#"(function() {{
  if (typeof FS === 'undefined' || typeof IDBFS === 'undefined') {{
    console.warn('[anisette-rs] FS/IDBFS unavailable');
    return;
  }}
  var mp = "{mount_path}";
  try {{ FS.mkdirTree(mp); }} catch (_e) {{}}
  try {{ FS.mount(IDBFS, {{}}, mp); }} catch (_e) {{}}
  FS.syncfs(true, function(err) {{
    if (err) {{
      console.error('[anisette-rs] IDBFS initial sync failed', err);
    }} else {{
      console.log('[anisette-rs] IDBFS ready at ' + mp);
    }}
  }});
}})();"#,
    );
    run_script(&script)?;
    Ok(mount_path)
}

pub fn sync_idbfs(populate_from_storage: bool) -> Result<(), String> {
    let populate = if populate_from_storage {
        "true"
    } else {
        "false"
    };
    let script = format!(
        r#"(function() {{
  if (typeof FS === 'undefined') {{
    return;
  }}
  FS.syncfs({populate}, function(err) {{
    if (err) {{
      console.error('[anisette-rs] IDBFS sync failed', err);
    }} else {{
      console.log('[anisette-rs] IDBFS sync done');
    }}
  }});
}})();"#,
    );
    run_script(&script)
}
