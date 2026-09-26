mod catalog;
mod ids;
mod index;
mod player;
mod sources;
mod store;
mod stream;

use tauri::RunEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(player::Player::default())
        .manage(store::StoreState::default())
        .manage(catalog::CatalogState::default())
        .manage(index::IndexState::default())
        .manage(sources::torbox::TorBoxState::default())
        .manage(sources::local::LocalTorrentState::default())
        .manage(stream::StreamState::default())
        .setup(|app| {
            player::setup(app)?;
            ids::spawn_refresh(app.handle());
            sources::local::spawn_startup_cleanup(app.handle());
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
            catalog::catalog_home,
            catalog::catalog_browse,
            catalog::catalog_show,
            catalog::catalog_search,
            index::index_releases,
            index::index_pick,
            sources::torbox::torbox_status,
            sources::torbox::torbox_connect,
            sources::torbox::torbox_disconnect,
            sources::local::settings::local_settings,
            sources::local::settings::local_set_settings,
            stream::stream_start,
            stream::stream_list,
            stream::stream_watched,
            stream::stream_close,
            stream::stream_remove,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            tauri::async_runtime::block_on(stream::shutdown(handle));
        }
    });
}
