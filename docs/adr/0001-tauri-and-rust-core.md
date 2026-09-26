# Tauri 2 with a Rust core

Lokii runs on Tauri 2 with all logic (catalog, torrent, TorBox, player control, storage) in Rust and a React UI in the system webview. We chose it over Electron + WebTorrent because the app stays near 15 MB instead of ~150 MB, uses less RAM during long playback, and librqbit gives us a mature Rust torrent engine that streams while it downloads.

## Consequences

- The UI renders in WebKit on macOS and WebView2 on Windows, so CSS must be tested on both.
- The player cannot live in the webview (see [ADR 0002](./0002-embedded-libmpv-player.md)).
