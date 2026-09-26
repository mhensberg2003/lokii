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
];

#[derive(Default)]
pub struct Player {
    mpv: OnceLock<Arc<Mpv>>,
}

impl Player {
    fn mpv(&self) -> Result<&Arc<Mpv>, String> {
        self.mpv.get().ok_or_else(|| "player is not ready".to_string())
    }
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

    // Spike helper: autoplay a file so playback can be checked without the UI.
    if let Ok(path) = std::env::var("LOKII_SPIKE_FILE") {
        mpv.command("loadfile", &[&path, "replace"]).map_err(|e| format!("cannot load {path}: {e}"))?;
    }

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
            Some(Ok(Event::EndFile(reason))) => {
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
