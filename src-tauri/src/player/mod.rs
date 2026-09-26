//! Embedded mpv player. mpv renders into a native view behind the transparent webview.

#[cfg(target_os = "macos")]
mod macos;

use std::sync::{Arc, OnceLock};

use libmpv2::events::{Event, PropertyData};
use libmpv2::{Format, Mpv};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

const OBSERVED: &[(&str, Format)] = &[
    ("pause", Format::Flag),
    ("time-pos", Format::Double),
    ("duration", Format::Double),
    ("paused-for-cache", Format::Flag),
    ("sid", Format::String),
    ("aid", Format::String),
    ("media-title", Format::String),
    ("volume", Format::Double),
    ("mute", Format::Flag),
];

/// Highest volume the UI can set, in percent.
const MAX_VOLUME: f64 = 100.0;

#[derive(Default)]
pub struct Player {
    mpv: OnceLock<Arc<Mpv>>,
}

impl Player {
    fn mpv(&self) -> Result<&Arc<Mpv>, String> {
        self.mpv.get().ok_or_else(|| "player is not ready".to_string())
    }
}

/// One audio or subtitle track of the loaded file.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    /// "audio" or "sub".
    pub kind: String,
    pub title: Option<String>,
    /// Language code as the file gives it, for example "jpn" or "en".
    pub lang: Option<String>,
    pub codec: Option<String>,
    pub selected: bool,
}

#[derive(Clone, Serialize)]
struct PropertyEvent {
    name: String,
    value: Value,
}

/// Creates the video surface on the main thread, then starts mpv on its own thread.
pub fn setup(app: &tauri::App) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("main window is missing")?;
    video_surface(&window)?;
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        if let Err(err) = run(handle.clone()) {
            let _ = handle.emit("player://error", err);
        }
    });
    Ok(())
}

#[cfg(target_os = "macos")]
fn video_surface(window: &tauri::WebviewWindow) -> Result<(), String> {
    macos::attach_video_view(window)
}

#[cfg(target_os = "macos")]
fn attach_renderer(mpv: &Arc<Mpv>) -> Result<(), String> {
    macos::attach_mpv(mpv.clone(), std::time::Duration::from_secs(10))
}

#[cfg(not(target_os = "macos"))]
fn video_surface(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Err("video surface is not implemented on this platform yet".into())
}

#[cfg(not(target_os = "macos"))]
fn attach_renderer(_mpv: &Arc<Mpv>) -> Result<(), String> {
    Ok(())
}

fn run(app: AppHandle) -> Result<(), String> {
    let mpv = Mpv::with_initializer(|init| {
        init.set_option("vo", "libmpv")?;
        init.set_option("hwdec", "auto-safe")?;
        init.set_option("keep-open", "yes")?;
        // keep-open pauses at the end of a file; the next file must still start playing.
        init.set_option("reset-on-next-file", "pause")?;
        init.set_option("osc", "no")?;
        init.set_option("osd-level", 0i64)?;
        init.set_option("input-default-bindings", "no")?;
        init.set_option("slang", "eng,en")?;
        init.set_option("alang", "jpn,ja")?;
        Ok(())
    })
    .map_err(|e| format!("mpv failed to start: {e}"))?;

    for (id, (name, format)) in OBSERVED.iter().enumerate() {
        mpv.observe_property(name, *format, id as u64).map_err(|e| format!("cannot observe {name}: {e}"))?;
    }

    let mpv = Arc::new(mpv);
    attach_renderer(&mpv)?;
    let player = app.state::<Player>();
    player.mpv.set(mpv.clone()).map_err(|_| "player started twice")?;
    let _ = app.emit("player://ready", ());

    loop {
        match mpv.wait_event(60.0) {
            Some(Ok(Event::PropertyChange { name, change, .. })) => {
                let value = match change {
                    PropertyData::Flag(v) => json!(v),
                    PropertyData::Double(v) => json!(v),
                    PropertyData::Int64(v) => json!(v),
                    PropertyData::Str(v) | PropertyData::OsdStr(v) => json!(v),
                };
                let _ = app.emit("player://property", PropertyEvent { name: name.to_string(), value });
            }
            Some(Ok(Event::FileLoaded)) => {
                let _ = app.emit("player://file-loaded", ());
            }
            Some(Ok(Event::EndFile(reason))) => {
                // libmpv sends no change when these become unavailable, so clear them here.
                for name in ["time-pos", "duration"] {
                    let _ = app.emit("player://property", PropertyEvent { name: name.into(), value: Value::Null });
                }
                let _ = app.emit("player://end-file", reason as i64);
            }
            Some(Ok(Event::Shutdown)) => return Ok(()),
            Some(Err(err)) => {
                let _ = app.emit("player://error", err.to_string());
            }
            _ => {}
        }
    }
}

/// Current values of all observed properties. The UI calls this on mount, because mpv
/// sends the first values before the webview's listeners exist.
#[tauri::command]
pub fn player_snapshot(player: State<Player>) -> Result<serde_json::Map<String, Value>, String> {
    let mpv = player.mpv()?;
    let mut snapshot = serde_json::Map::new();
    for (name, format) in OBSERVED {
        let value = match format {
            Format::Flag => mpv.get_property::<bool>(name).map(|v| json!(v)),
            Format::Double => mpv.get_property::<f64>(name).map(|v| json!(v)),
            _ => mpv.get_property::<String>(name).map(|v| json!(v)),
        };
        snapshot.insert(name.to_string(), value.unwrap_or(Value::Null));
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn player_ready(player: State<Player>) -> bool {
    player.mpv.get().is_some()
}

#[tauri::command]
pub fn player_load(player: State<Player>, url: String) -> Result<(), String> {
    player.mpv()?.command("loadfile", &[&url, "replace"]).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn player_toggle_pause(player: State<Player>) -> Result<(), String> {
    player.mpv()?.command("cycle", &["pause"]).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn player_seek(player: State<Player>, seconds: f64) -> Result<(), String> {
    player.mpv()?.command("seek", &[&seconds.to_string(), "absolute"]).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn player_cycle(player: State<Player>, property: String) -> Result<(), String> {
    if !matches!(property.as_str(), "sid" | "aid") {
        return Err(format!("cannot cycle {property}"));
    }
    player.mpv()?.command("cycle", &[&property]).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn player_stop(player: State<Player>) -> Result<(), String> {
    player.mpv()?.command("stop", &[]).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn player_seek_by(player: State<Player>, seconds: f64) -> Result<(), String> {
    player.mpv()?.command("seek", &[&seconds.to_string(), "relative"]).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn player_set_volume(player: State<Player>, volume: f64) -> Result<(), String> {
    let mpv = player.mpv()?;
    mpv.set_property("volume", volume.clamp(0.0, MAX_VOLUME)).map_err(|e| e.to_string())?;
    mpv.set_property("mute", false).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn player_toggle_mute(player: State<Player>) -> Result<(), String> {
    player.mpv()?.command("cycle", &["mute"]).map_err(|e| e.to_string())
}

/// The audio and subtitle tracks of the loaded file.
#[tauri::command]
pub fn player_tracks(player: State<Player>) -> Result<Vec<Track>, String> {
    let mpv = player.mpv()?;
    let count = mpv.get_property::<i64>("track-list/count").unwrap_or(0);
    let text = |i: i64, field: &str| mpv.get_property::<String>(&format!("track-list/{i}/{field}")).ok();
    let tracks = (0..count)
        .filter_map(|i| {
            let kind = text(i, "type").filter(|kind| kind == "audio" || kind == "sub")?;
            Some(Track {
                id: mpv.get_property::<i64>(&format!("track-list/{i}/id")).ok()?,
                kind,
                title: text(i, "title"),
                lang: text(i, "lang"),
                codec: text(i, "codec"),
                selected: mpv.get_property::<bool>(&format!("track-list/{i}/selected")).unwrap_or(false),
            })
        })
        .collect();
    Ok(tracks)
}

/// Selects a track by ID, or turns the kind off with `None`.
#[tauri::command]
pub fn player_set_track(player: State<Player>, kind: String, id: Option<i64>) -> Result<(), String> {
    let property = match kind.as_str() {
        "audio" => "aid",
        "sub" => "sid",
        _ => return Err(format!("unknown track kind {kind}")),
    };
    let value = id.map_or_else(|| "no".to_string(), |id| id.to_string());
    player.mpv()?.set_property(property, value).map_err(|e| e.to_string())
}
