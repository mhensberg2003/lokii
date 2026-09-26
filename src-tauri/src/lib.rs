mod player;
mod torrent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(player::Player::default())
        .manage(torrent::TorrentEngine::default())
        .setup(|app| {
            player::setup(app)?;
            torrent::spike_probe(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            player::player_ready,
            player::player_snapshot,
            player::player_load,
            player::player_toggle_pause,
            player::player_seek,
            player::player_cycle,
            player::player_stop,
            torrent::torrent_stream,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
