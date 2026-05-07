use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use base64::Engine;

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZulipMessage {
    pub id: u64,
    pub sender: String,
    pub content: String,
    pub time: String,
    pub stream: String,
    pub topic: String,
    pub image_urls: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ZulipChannel {
    pub stream_id: u64,
    pub name: String,
    pub topics: Vec<String>,
    pub topics_loaded: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ZulipConfig {
    pub enabled: bool,
    pub url: String,
    pub email: String,
    pub api_key: String,
    pub stream: String,
    pub topic: String,
    pub poll_rate: f64,
}

#[derive(Clone, PartialEq)]
pub enum CachedImage {
    Loading,
    Ready,
    Failed,
}

/// RGBA bytes decoded and ready for GPU upload by the render thread.
pub struct DecodedImage {
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

static DECODED_IMAGES: OnceLock<Mutex<HashMap<String, DecodedImage>>> = OnceLock::new();

/// Drain all fully-decoded images so the render thread can upload them.
pub fn take_decoded_images() -> Vec<(String, DecodedImage)> {
    let map = DECODED_IMAGES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut lock = map.lock().unwrap();
    lock.drain().collect()
}

// ── State ──────────────────────────────────────────────────────────────────────

struct ZulipState {
    config: ZulipConfig,

    live_messages: Vec<ZulipMessage>,
    queue_id: Option<String>,
    last_event_id: i64,

    channels: Vec<ZulipChannel>,
    channels_loading: bool,
    channels_loaded: bool,

    // keyed by channel_key(stream, topic)
    channel_history: HashMap<String, Vec<ZulipMessage>>,
    channel_history_loading: HashSet<String>,

    image_cache: HashMap<String, CachedImage>,
}

static STATE: OnceLock<Arc<Mutex<ZulipState>>> = OnceLock::new();

// ── Init ───────────────────────────────────────────────────────────────────────

pub fn init() {
    STATE.get_or_init(|| {
        let state = Arc::new(Mutex::new(ZulipState {
            config: ZulipConfig::default(),
            live_messages: Vec::new(),
            queue_id: None,
            last_event_id: -1,
            channels: Vec::new(),
            channels_loading: false,
            channels_loaded: false,
            channel_history: HashMap::new(),
            channel_history_loading: HashSet::new(),
            image_cache: HashMap::new(),
        }));
        let clone = Arc::clone(&state);
        thread::spawn(move || worker_loop(clone));
        state
    });
}

// ── Config ─────────────────────────────────────────────────────────────────────

pub fn set_config(cfg: ZulipConfig) {
    crate::config::modify(|c| c.zulip = cfg.clone());
    if let Some(s) = STATE.get() {
        let mut lock = s.lock().unwrap();
        let old = lock.config.clone();
        let creds_changed = cfg.url != old.url || cfg.email != old.email || cfg.api_key != old.api_key;
        let just_enabled = cfg.enabled && !old.enabled;
        lock.config = cfg;
        if creds_changed || just_enabled {
            lock.queue_id = None;
            lock.last_event_id = -1;
            lock.channels_loaded = false;
            lock.channels_loading = false;
        }
    }
}

pub fn get_config() -> ZulipConfig {
    STATE.get().map(|s| s.lock().unwrap().config.clone()).unwrap_or_default()
}

// ── Live messages (for MC chat bridge) ────────────────────────────────────────

pub fn get_messages() -> Vec<ZulipMessage> {
    if let Some(s) = STATE.get() {
        s.try_lock().map(|l| l.live_messages.clone()).unwrap_or_default()
    } else {
        vec![]
    }
}

pub fn clear_messages() {
    if let Some(s) = STATE.get() {
        if let Ok(mut l) = s.lock() {
            l.live_messages.clear();
        }
    }
}

// ── Send ───────────────────────────────────────────────────────────────────────

pub fn send_message(content: String) {
    let cfg = get_config();
    send_to(cfg.stream, cfg.topic, content);
}

pub fn send_to(stream: String, topic: String, content: String) {
    let s = match STATE.get() {
        Some(s) => s,
        None => return,
    };
    let (url, email, api_key) = {
        let lock = s.lock().unwrap();
        if !lock.config.enabled {
            return;
        }
        (lock.config.url.clone(), lock.config.email.clone(), lock.config.api_key.clone())
    };
    thread::spawn(move || {
        let auth = make_auth(&email, &api_key);
        let body = format!(
            "type=stream&to={}&topic={}&content={}",
            urlencoding::encode(&stream),
            urlencoding::encode(&topic),
            urlencoding::encode(&content),
        );
        if let Err(e) = ureq::post(&format!("{}/api/v1/messages", url))
            .set("Authorization", &format!("Basic {}", auth))
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_string(&body)
        {
            error!("Zulip send error: {}", e);
        }
    });
}

// ── Channel browser ────────────────────────────────────────────────────────────

pub fn fetch_channels() {
    let s = match STATE.get() {
        Some(s) => s,
        None => return,
    };
    let (url, email, api_key, go) = {
        let lock = s.lock().unwrap();
        let go = lock.config.enabled
            && !lock.config.url.is_empty()
            && !lock.channels_loading
            && !lock.channels_loaded;
        (lock.config.url.clone(), lock.config.email.clone(), lock.config.api_key.clone(), go)
    };
    if !go {
        return;
    }
    s.lock().unwrap().channels_loading = true;
    let sc = Arc::clone(s);
    thread::spawn(move || {
        let auth = make_auth(&email, &api_key);
        match fetch_channels_impl(&url, &auth) {
            Ok(channels) => {
                let mut lock = sc.lock().unwrap();
                lock.channels = channels;
                lock.channels_loaded = true;
                lock.channels_loading = false;
            }
            Err(e) => {
                warn!("fetch_channels: {}", e);
                sc.lock().unwrap().channels_loading = false;
            }
        }
    });
}

pub fn get_channels() -> Vec<ZulipChannel> {
    STATE.get().map(|s| s.lock().unwrap().channels.clone()).unwrap_or_default()
}

pub fn channels_loaded() -> bool {
    STATE.get().map(|s| s.lock().unwrap().channels_loaded).unwrap_or(false)
}

pub fn channels_loading() -> bool {
    STATE.get().map(|s| s.lock().unwrap().channels_loading).unwrap_or(false)
}

pub fn reset_channels() {
    if let Some(s) = STATE.get() {
        if let Ok(mut lock) = s.lock() {
            lock.channels.clear();
            lock.channels_loaded = false;
            lock.channels_loading = false;
        }
    }
}

fn fetch_channels_impl(url: &str, auth: &str) -> anyhow::Result<Vec<ZulipChannel>> {
    let res = ureq::get(&format!("{}/api/v1/users/me/subscriptions", url))
        .set("Authorization", &format!("Basic {}", auth))
        .call()?;
    let json: serde_json::Value = res.into_json()?;
    let mut channels = Vec::new();
    if let Some(subs) = json["subscriptions"].as_array() {
        for sub in subs {
            channels.push(ZulipChannel {
                stream_id: sub["stream_id"].as_u64().unwrap_or(0),
                name: sub["name"].as_str().unwrap_or_default().to_string(),
                topics: Vec::new(),
                topics_loaded: false,
            });
        }
        channels.sort_by(|a, b| a.name.cmp(&b.name));
    }
    Ok(channels)
}

// ── Topics ─────────────────────────────────────────────────────────────────────

pub fn fetch_topics(stream_id: u64) {
    let s = match STATE.get() {
        Some(s) => s,
        None => return,
    };
    let (url, email, api_key, go) = {
        let lock = s.lock().unwrap();
        let already = lock.channels.iter().any(|c| c.stream_id == stream_id && c.topics_loaded);
        let go = !already && lock.config.enabled && !lock.config.url.is_empty();
        (lock.config.url.clone(), lock.config.email.clone(), lock.config.api_key.clone(), go)
    };
    if !go {
        return;
    }
    let sc = Arc::clone(s);
    thread::spawn(move || {
        let auth = make_auth(&email, &api_key);
        match fetch_topics_impl(&url, &auth, stream_id) {
            Ok(topics) => {
                let mut lock = sc.lock().unwrap();
                if let Some(ch) = lock.channels.iter_mut().find(|c| c.stream_id == stream_id) {
                    ch.topics = topics;
                    ch.topics_loaded = true;
                }
            }
            Err(e) => warn!("fetch_topics {}: {}", stream_id, e),
        }
    });
}

fn fetch_topics_impl(url: &str, auth: &str, stream_id: u64) -> anyhow::Result<Vec<String>> {
    let res = ureq::get(&format!(
        "{}/api/v1/users/me/{}/topics",
        url, stream_id
    ))
    .set("Authorization", &format!("Basic {}", auth))
    .call()?;
    let json: serde_json::Value = res.into_json()?;
    let mut topics = Vec::new();
    if let Some(arr) = json["topics"].as_array() {
        for t in arr {
            if let Some(name) = t["name"].as_str() {
                topics.push(name.to_string());
            }
        }
    }
    Ok(topics)
}

// ── Message history ────────────────────────────────────────────────────────────

pub fn channel_key(stream: &str, topic: &str) -> String {
    format!("{}\x00{}", stream, topic)
}

pub fn fetch_channel_messages(stream: String, topic: String) {
    let s = match STATE.get() {
        Some(s) => s,
        None => return,
    };
    let key = channel_key(&stream, &topic);
    let (url, email, api_key, go) = {
        let lock = s.lock().unwrap();
        let loaded = lock.channel_history.contains_key(&key);
        let loading = lock.channel_history_loading.contains(&key);
        let go = !loaded && !loading && lock.config.enabled && !lock.config.url.is_empty();
        (lock.config.url.clone(), lock.config.email.clone(), lock.config.api_key.clone(), go)
    };
    if !go {
        return;
    }
    s.lock().unwrap().channel_history_loading.insert(key.clone());
    let sc = Arc::clone(s);
    thread::spawn(move || {
        let auth = make_auth(&email, &api_key);
        match fetch_messages_impl(&url, &auth, &stream, &topic) {
            Ok(msgs) => {
                let mut lock = sc.lock().unwrap();
                lock.channel_history_loading.remove(&key);
                lock.channel_history.insert(key, msgs);
            }
            Err(e) => {
                warn!("fetch_messages {}/{}: {}", stream, topic, e);
                let mut lock = sc.lock().unwrap();
                lock.channel_history_loading.remove(&key);
                lock.channel_history.insert(key, Vec::new());
            }
        }
    });
}

fn fetch_messages_impl(
    url: &str,
    auth: &str,
    stream: &str,
    topic: &str,
) -> anyhow::Result<Vec<ZulipMessage>> {
    let narrow = serde_json::json!([
        {"operator": "stream", "operand": stream},
        {"operator": "topic",  "operand": topic}
    ])
    .to_string();
    let req_url = format!(
        "{}/api/v1/messages?anchor=newest&num_before=50&num_after=0&narrow={}",
        url,
        urlencoding::encode(&narrow)
    );
    let res = ureq::get(&req_url)
        .set("Authorization", &format!("Basic {}", auth))
        .call()?;
    let json: serde_json::Value = res.into_json()?;
    let mut msgs = Vec::new();
    if let Some(arr) = json["messages"].as_array() {
        for m in arr {
            let content = m["content"].as_str().unwrap_or_default().to_string();
            let image_urls = extract_image_urls(&content, url);
            msgs.push(ZulipMessage {
                id: m["id"].as_u64().unwrap_or(0),
                sender: m["sender_full_name"].as_str().unwrap_or("?").to_string(),
                content,
                time: fmt_ts(m["timestamp"].as_i64().unwrap_or(0)),
                stream: stream.to_string(),
                topic: topic.to_string(),
                image_urls,
            });
        }
    }
    Ok(msgs)
}

pub fn get_channel_messages(stream: &str, topic: &str) -> Option<Vec<ZulipMessage>> {
    let key = channel_key(stream, topic);
    let s = STATE.get()?;
    let lock = s.lock().unwrap();
    let mut msgs = lock.channel_history.get(&key)?.clone();
    // Append live messages not already in history
    for msg in &lock.live_messages {
        if msg.stream == stream && msg.topic == topic && !msgs.iter().any(|m| m.id == msg.id) {
            msgs.push(msg.clone());
        }
    }
    Some(msgs)
}

pub fn get_stream_messages(stream: &str) -> Vec<ZulipMessage> {
    STATE.get()
        .and_then(|s| s.lock().ok())
        .map(|l| l.live_messages.iter().filter(|m| m.stream == stream).cloned().collect())
        .unwrap_or_default()
}

pub fn is_channel_loading(stream: &str, topic: &str) -> bool {
    let key = channel_key(stream, topic);
    STATE.get()
        .and_then(|s| s.lock().ok())
        .map(|l| l.channel_history_loading.contains(&key))
        .unwrap_or(false)
}

// ── Image cache ─────────────────────────────────────────────────────────────────

pub fn fetch_image(url: String) {
    let s = match STATE.get() {
        Some(s) => s,
        None => return,
    };
    let (auth, already) = {
        let lock = s.lock().unwrap();
        let already = lock.image_cache.contains_key(&url);
        let auth = make_auth(&lock.config.email, &lock.config.api_key);
        (auth, already)
    };
    if already {
        return;
    }
    s.lock().unwrap().image_cache.insert(url.clone(), CachedImage::Loading);
    let sc = Arc::clone(s);
    thread::spawn(move || {
        match ureq::get(&url)
            .set("Authorization", &format!("Basic {}", auth))
            .call()
        {
            Ok(res) => {
                let mut bytes = Vec::new();
                if res.into_reader().read_to_end(&mut bytes).is_err() {
                    sc.lock().unwrap().image_cache.insert(url, CachedImage::Failed);
                    return;
                }
                // Decode to RGBA in this background thread so render thread doesn't stall.
                match image::load_from_memory(&bytes) {
                    Ok(img) => {
                        let img = img.resize(256, 256, image::imageops::FilterType::Triangle);
                        let rgba = img.to_rgba8();
                        let w = rgba.width() as usize;
                        let h = rgba.height() as usize;
                        let pixels = rgba.into_raw();
                        let store = DECODED_IMAGES.get_or_init(|| Mutex::new(HashMap::new()));
                        store.lock().unwrap().insert(url.clone(), DecodedImage { pixels, width: w, height: h });
                        sc.lock().unwrap().image_cache.insert(url, CachedImage::Ready);
                    }
                    Err(e) => {
                        warn!("image decode {}: {}", url, e);
                        sc.lock().unwrap().image_cache.insert(url, CachedImage::Failed);
                    }
                }
            }
            Err(e) => {
                warn!("image fetch {}: {}", url, e);
                sc.lock().unwrap().image_cache.insert(url, CachedImage::Failed);
            }
        }
    });
}

pub fn get_image(url: &str) -> Option<CachedImage> {
    STATE.get()?.lock().ok()?.image_cache.get(url).cloned()
}

// ── Worker loop ────────────────────────────────────────────────────────────────

fn worker_loop(state: Arc<Mutex<ZulipState>>) {
    loop {
        let (enabled, poll_rate) = {
            let lock = state.lock().unwrap();
            (lock.config.enabled, lock.config.poll_rate)
        };
        if !enabled {
            thread::sleep(Duration::from_secs(1));
            continue;
        }
        if let Err(e) = poll_step(&state) {
            warn!("Zulip poll: {}", e);
            thread::sleep(Duration::from_secs(5));
        }
        let sleep_ms = ((poll_rate * 1000.0) as u64).max(500);
        thread::sleep(Duration::from_millis(sleep_ms));
    }
}

fn poll_step(state: &Arc<Mutex<ZulipState>>) -> anyhow::Result<()> {
    let (url, auth, queue_id, last_event_id) = {
        let lock = state.lock().unwrap();
        let auth = make_auth(&lock.config.email, &lock.config.api_key);
        (lock.config.url.clone(), auth, lock.queue_id.clone(), lock.last_event_id)
    };
    if url.is_empty() {
        return Ok(());
    }

    if queue_id.is_none() {
        let res = ureq::post(&format!("{}/api/v1/register", url))
            .set("Authorization", &format!("Basic {}", auth))
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_string("event_types=[\"message\"]")?;
        let json: serde_json::Value = res.into_json()?;
        if json["result"] == "success" {
            let mut lock = state.lock().unwrap();
            lock.queue_id = Some(json["queue_id"].as_str().unwrap_or_default().to_string());
            lock.last_event_id = json["last_event_id"].as_i64().unwrap_or(-1);
            info!("Zulip queue registered: {:?}", lock.queue_id);
        }
        return Ok(());
    }

    let poll_url = format!(
        "{}/api/v1/events?queue_id={}&last_event_id={}&dont_block=true",
        url,
        queue_id.unwrap(),
        last_event_id
    );
    match ureq::get(&poll_url)
        .set("Authorization", &format!("Basic {}", auth))
        .call()
    {
        Ok(r) => {
            let json: serde_json::Value = r.into_json()?;
            if json["result"] == "success" {
                let mut lock = state.lock().unwrap();
                let base_url = lock.config.url.clone();
                if let Some(events) = json["events"].as_array() {
                    for event in events {
                        if event["type"] == "message" {
                            let msg = &event["message"];
                            let content = msg["content"].as_str().unwrap_or_default().to_string();
                            let image_urls = extract_image_urls(&content, &base_url);
                            let stream = if msg["type"] == "stream" {
                                msg["display_recipient"].as_str().unwrap_or_default().to_string()
                            } else {
                                String::new()
                            };
                            let topic = msg["subject"].as_str().unwrap_or_default().to_string();
                            let zulip_msg = ZulipMessage {
                                id: msg["id"].as_u64().unwrap_or(0),
                                sender: msg["sender_full_name"].as_str().unwrap_or("?").to_string(),
                                content,
                                time: fmt_ts(msg["timestamp"].as_i64().unwrap_or(0)),
                                stream: stream.clone(),
                                topic: topic.clone(),
                                image_urls,
                            };
                            // Append to loaded channel history if present
                            let key = channel_key(&stream, &topic);
                            if let Some(hist) = lock.channel_history.get_mut(&key) {
                                if !hist.iter().any(|m| m.id == zulip_msg.id) {
                                    hist.push(zulip_msg.clone());
                                }
                            }
                            lock.live_messages.push(zulip_msg);
                            if lock.live_messages.len() > 200 {
                                lock.live_messages.remove(0);
                            }
                        }
                        if let Some(id) = event["id"].as_i64() {
                            lock.last_event_id = lock.last_event_id.max(id);
                        }
                    }
                }
            }
        }
        Err(ureq::Error::Status(400, _)) => {
            // Queue expired, re-register next tick
            state.lock().unwrap().queue_id = None;
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn make_auth(email: &str, api_key: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", email, api_key))
}

fn fmt_ts(ts: i64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_else(|| "??:??".to_string())
}

fn extract_image_urls(content: &str, base_url: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let base = base_url.trim_end_matches('/');
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Markdown image: ![alt](url)
        if i + 1 < len && bytes[i] == b'!' && bytes[i + 1] == b'[' {
            if let Some(rel) = content[i..].find("](") {
                let url_start = i + rel + 2;
                let after = &content[url_start..];
                // Find closing paren, respecting one level of nesting
                let mut depth = 0usize;
                let mut end = None;
                for (j, c) in after.char_indices() {
                    match c {
                        '(' => depth += 1,
                        ')' => {
                            if depth == 0 {
                                end = Some(j);
                                break;
                            }
                            depth -= 1;
                        }
                        _ => {}
                    }
                }
                if let Some(e) = end {
                    let raw = after[..e].split_whitespace().next().unwrap_or("");
                    if raw.starts_with("http://") || raw.starts_with("https://") {
                        if is_image_url(raw) {
                            urls.push(raw.to_string());
                        }
                    } else if raw.starts_with("/user_uploads/") {
                        urls.push(format!("{}{}", base, raw));
                    }
                    i = url_start + e + 1;
                    continue;
                }
            }
        }

        // Relative /user_uploads/ path appearing outside image syntax
        if content[i..].starts_with("/user_uploads/") {
            let rest = &content[i..];
            let end = rest
                .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\'' | '>'))
                .unwrap_or(rest.len());
            let full = format!("{}{}", base, &rest[..end]);
            if !urls.contains(&full) {
                urls.push(full);
            }
            i += end.max(1);
            continue;
        }

        i += 1;
    }

    urls
}

fn is_image_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    let path = lower.split('?').next().unwrap_or(&lower);
    path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".gif")
        || path.ends_with(".webp")
}
