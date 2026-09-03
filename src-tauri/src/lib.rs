//! Whisper's native core: the HTTP engine the UI talks to over Tauri IPC,
//! plus clipboard and save-file helpers that webviews can't do reliably.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use tauri::ipc::Response;
use tauri::State;
use tokio::sync::oneshot;

const MAX_RESPONSE_BYTES: usize = 256 * 1024 * 1024;
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const MSG_TIMEOUT: &str = "Request timed out.";
const MSG_CANCELLED: &str = "Request cancelled.";
/// The UI's own timer owns the deadline and classifies the outcome; the
/// reqwest timeout is only a backstop, so give it a margin to fire second.
const TIMEOUT_MARGIN_MS: u64 = 2000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReqMeta {
    id: String,
    url: String,
    method: String,
    #[serde(default)]
    headers: Vec<(String, String)>,
    #[serde(default)]
    timeout_ms: u64,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct RespMeta {
    status: u16,
    status_text: String,
    url: String,
    redirected: bool,
    headers: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// "timeout" | "cancelled" | "network" — lets the UI classify without parsing text
    #[serde(skip_serializing_if = "Option::is_none")]
    error_kind: Option<String>,
}

/// In-flight requests by id, so the UI's Cancel button can abort them.
#[derive(Default)]
struct Inflight {
    active: Mutex<HashMap<String, oneshot::Sender<()>>>,
    /// Cancels that raced ahead of their request's registration.
    cancelled_early: Mutex<HashSet<String>>,
}

/// Wire format handed back to the page:
/// `[u32 big-endian meta length][meta JSON][raw body bytes]`
fn pack(meta: &RespMeta, body: &[u8]) -> Vec<u8> {
    let m = serde_json::to_vec(meta).expect("meta serializes");
    let mut out = Vec::with_capacity(4 + m.len() + body.len());
    out.extend_from_slice(&(m.len() as u32).to_be_bytes());
    out.extend_from_slice(&m);
    out.extend_from_slice(body);
    out
}

fn error_meta(message: String) -> RespMeta {
    let kind = if message == MSG_TIMEOUT {
        "timeout"
    } else if message == MSG_CANCELLED {
        "cancelled"
    } else {
        "network"
    };
    RespMeta { error: Some(message), error_kind: Some(kind.to_string()), ..Default::default() }
}

fn fail(message: impl Into<String>) -> Response {
    Response::new(pack(&error_meta(message.into()), &[]))
}

/// reqwest's top-level messages are vague ("error sending request") — walk the
/// source chain so the user sees the real cause (DNS, TLS, connection refused…).
fn describe(e: reqwest::Error) -> String {
    if e.is_timeout() {
        return MSG_TIMEOUT.to_string();
    }
    let mut msg = e.to_string();
    let mut src = std::error::Error::source(&e);
    while let Some(s) = src {
        let sm = s.to_string();
        if !sm.is_empty() && !msg.contains(&sm) {
            msg.push_str(": ");
            msg.push_str(&sm);
        }
        src = s.source();
    }
    msg
}

fn build_headers(list: &[(String, String)]) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    for (k, v) in list {
        // reqwest derives Content-Length from the body; a stale user value would corrupt the request
        if k.eq_ignore_ascii_case("content-length") {
            continue;
        }
        let name = HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| format!("\"{k}\" is not a valid header name."))?;
        let value = HeaderValue::from_str(v).map_err(|_| {
            format!("The value of header \"{k}\" is invalid — header values must be plain text with no line breaks.")
        })?;
        headers.append(name, value);
    }
    Ok(headers)
}

async fn perform(
    client: &reqwest::Client,
    meta: &ReqMeta,
    body: Option<Vec<u8>>,
) -> Result<(RespMeta, Vec<u8>), String> {
    let method = reqwest::Method::from_bytes(meta.method.as_bytes())
        .map_err(|_| format!("Unsupported method \"{}\".", meta.method))?;
    let url = reqwest::Url::parse(&meta.url).map_err(|e| format!("Invalid URL: {e}."))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only http:// and https:// URLs are supported.".to_string());
    }
    let headers = build_headers(&meta.headers)?;

    let mut req = client.request(method, url).headers(headers);
    if let Some(b) = body {
        req = req.body(b);
    }
    if meta.timeout_ms > 0 {
        req = req.timeout(Duration::from_millis(meta.timeout_ms + TIMEOUT_MARGIN_MS));
    }

    // compare against the URL reqwest actually sends (it strips user:pass@ into
    // an Authorization header), otherwise every such request looks redirected
    let req = req.build().map_err(describe)?;
    let sent = req.url().clone();
    let mut resp = client.execute(req).await.map_err(describe)?;
    let status = resp.status();
    let final_url = resp.url().to_string();
    let redirected = resp.url() != &sent;
    let headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
        .collect();

    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(describe)? {
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err("Response exceeded 256 MB — too large for the viewer.".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok((
        RespMeta {
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or("").to_string(),
            url: final_url,
            redirected,
            headers,
            error: None,
        },
        bytes,
    ))
}

#[tauri::command]
async fn send_request(
    meta: ReqMeta,
    body_b64: Option<String>,
    client: State<'_, reqwest::Client>,
    inflight: State<'_, Inflight>,
) -> Result<Response, String> {
    // register before any real work (base64 decode of a large body can take a
    // while) so a Cancel that arrives immediately is not lost
    let (tx, rx) = oneshot::channel::<()>();
    if inflight.cancelled_early.lock().unwrap().remove(&meta.id) {
        return Ok(fail(MSG_CANCELLED));
    }
    inflight.active.lock().unwrap().insert(meta.id.clone(), tx);

    let result = tokio::select! {
        r = async {
            let body = match body_b64 {
                Some(s) => Some(B64.decode(s).map_err(|_| "Invalid request body encoding.".to_string())?),
                None => None,
            };
            perform(client.inner(), &meta, body).await
        } => r,
        _ = rx => Err(MSG_CANCELLED.to_string()),
    };
    inflight.active.lock().unwrap().remove(&meta.id);
    Ok(match result {
        Ok((m, b)) => Response::new(pack(&m, &b)),
        Err(e) => fail(e),
    })
}

#[tauri::command]
fn cancel_request(id: String, inflight: State<'_, Inflight>) {
    if let Some(tx) = inflight.active.lock().unwrap().remove(&id) {
        let _ = tx.send(());
    } else {
        // the cancel raced ahead of registration — remember it so the request
        // aborts on arrival (bounded: stray ids from already-finished requests)
        let mut early = inflight.cancelled_early.lock().unwrap();
        if early.len() > 256 {
            early.clear();
        }
        early.insert(id);
    }
}

#[tauri::command]
fn copy_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

/// Native "Save as…" dialog + write. Returns the chosen path, or None if cancelled.
#[tauri::command]
async fn save_file(
    app: tauri::AppHandle,
    suggested_name: String,
    data_b64: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let data = B64.decode(data_b64).map_err(|_| "Invalid file data encoding.".to_string())?;
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(suggested_name)
        .save_file(move |picked| {
            let _ = tx.send(picked);
        });
    let Some(picked) = rx.await.map_err(|_| "Save dialog was closed.".to_string())? else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| format!("Could not write file: {e}"))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = reqwest::Client::builder()
        .user_agent(concat!("Whisper/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .expect("failed to build HTTP client");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(client)
        .manage(Inflight::default())
        .invoke_handler(tauri::generate_handler![send_request, cancel_request, copy_text, save_file])
        .run(tauri::generate_context!())
        .expect("error while running Whisper");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_roundtrip() {
        let meta = RespMeta {
            status: 200,
            status_text: "OK".into(),
            url: "https://example.test/".into(),
            redirected: false,
            headers: vec![("content-type".into(), "text/plain".into())],
            error: None,
            error_kind: None,
        };
        let out = pack(&meta, b"hello");
        let len = u32::from_be_bytes([out[0], out[1], out[2], out[3]]) as usize;
        let m: serde_json::Value = serde_json::from_slice(&out[4..4 + len]).unwrap();
        assert_eq!(m["status"], 200);
        assert_eq!(m["statusText"], "OK");
        assert!(m.get("error").is_none());
        assert_eq!(&out[4 + len..], b"hello");
    }

    #[test]
    fn header_validation() {
        let ok = build_headers(&[("Accept".into(), "a".into()), ("accept".into(), "b".into())]).unwrap();
        assert_eq!(ok.get_all("accept").iter().count(), 2, "duplicates are preserved");
        assert!(build_headers(&[("bad name".into(), "x".into())]).is_err());
        assert!(build_headers(&[("X-Test".into(), "line\nbreak".into())]).is_err());
        assert!(build_headers(&[("Content-Length".into(), "5".into())]).unwrap().is_empty());
    }

    #[test]
    fn fail_packs_error_meta_with_kind() {
        for (msg, kind) in [(MSG_TIMEOUT, "timeout"), (MSG_CANCELLED, "cancelled"), ("dns error: no record", "network")] {
            let out = pack(&error_meta(msg.to_string()), &[]);
            let len = u32::from_be_bytes([out[0], out[1], out[2], out[3]]) as usize;
            let m: serde_json::Value = serde_json::from_slice(&out[4..4 + len]).unwrap();
            assert_eq!(m["error"], msg);
            assert_eq!(m["errorKind"], kind);
            assert_eq!(out.len(), 4 + len, "error packets carry no body");
        }
    }
}
