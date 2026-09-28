# Embedded libmpv instead of a web video player

Anime Releases are mostly MKV files with HEVC or 10-bit H.264 video and styled ASS subtitles. The system webviews cannot decode many of these, and ASS subtitles need a WASM renderer. We embed libmpv in the app window and draw our own controls on top, so every Release plays correctly and the player still looks like Lokii.

## Considered Options

- **Web player (HTML video)**: fastest to build, but some files do not play at all.
- **External mpv or IINA**: simplest, but the user must install it and the playback UI is not ours.

## Consequences

- On macOS, mpv ignores `--wid` and always opens its own window (checked in mpv 0.41 source). Lokii therefore uses the libmpv render API on macOS: mpv draws into a `CAOpenGLLayer` (OpenGL 3.2 core) in a view below the transparent webview, as IINA does. On Windows, mpv gets the Lokii window as `--wid` and makes its own child window below WebView2; `force-window` keeps it black while nothing plays.
- libmpv must ship with the app for each OS and CPU, and its license (GPL or LGPL build) must match Lokii's license.
