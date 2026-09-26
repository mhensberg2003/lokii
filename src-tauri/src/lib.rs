mod catalog;
mod ids;
mod index;
mod player;
mod store;
mod torrent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(player::Player::default())
        .manage(torrent::TorrentEngine::default())
        .manage(store::StoreState::default())
        .manage(catalog::CatalogState::default())
        .manage(index::IndexState::default())
        .setup(|app| {
            player::setup(app)?;
            torrent::spike_probe(app.handle());
            ids::spawn_refresh(app.handle());
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
            catalog::catalog_home,
            catalog::catalog_browse,
            catalog::catalog_show,
            catalog::catalog_search,
            index::index_releases,
            index::index_pick,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
