# Lokii

A calm desktop app for watching anime on macOS and Windows. Lokii finds a show on AniList, picks a good Release, and streams it through TorBox or a local torrent. All your data stays on your device.

> Status: early development (milestone M6: Ship). Episodes play through TorBox or a local torrent, with Skip Segments from AniSkip, Next Episode and keyboard shortcuts. Watch Progress, Continue watching and a Watchlist stay on the device. Releases install from GitHub and update themselves.

## Install

Download the file for your computer from [Releases](https://github.com/mhensberg2003/lokii/releases/latest):

| Computer | File |
|---|---|
| Mac with Apple silicon (M1 or later) | `Lokii_<version>_aarch64.dmg` |
| Mac with Intel | `Lokii_<version>_x64.dmg` |
| Windows 10 or 11 (64-bit) | `Lokii_<version>_x64-setup.exe` |

Lokii is not signed with an Apple or Microsoft certificate, so the system shows a warning the first time:

- **macOS:** open the DMG and drag Lokii to Applications. Open Lokii. When macOS says it cannot check the app, open System Settings → Privacy & Security, and click **Open Anyway** next to Lokii. If macOS says the app is damaged, run `xattr -dr com.apple.quarantine /Applications/Lokii.app` in Terminal.
- **Windows:** when SmartScreen says "Windows protected your PC", click **More info**, then **Run anyway**.

After that, Lokii finds new versions itself: an **Update** button shows in the sidebar.

## Features (v1 plan)

- Browse and search anime, with data from AniList, and Episode titles and images from ani.zip
- One Pace: search "one pace" to watch every Arc, with Episode titles from the One Pace Episode Guide
- Stream through TorBox, or through a local torrent when TorBox is not connected
- Built-in mpv player: plays MKV, HEVC, 10-bit video and styled ASS subtitles
- Continue watching, watchlist, and skip intro/outro
- No account and no cloud: watch progress and settings stay on your device

## Docs

- [Plan](docs/PLAN.md): decisions, architecture, milestones
- [Design](docs/DESIGN.md): visual references for each screen
- [Glossary](CONTEXT.md): the words we use in code and issues
- [Decisions](docs/adr/): architecture decision records

## Development

Requirements: macOS or Windows, Rust (stable), Node 22 and pnpm. On Windows, run the commands in Git Bash.

```sh
scripts/fetch-libmpv.sh   # the prebuilt libmpv that Lokii links and bundles
pnpm install
pnpm tauri dev
```

Without `scripts/fetch-libmpv.sh`, macOS dev builds use Homebrew's libmpv (`brew install mpv`).

To stream through TorBox, paste your API key in Settings → Sources. Without a key, Lokii uses Local Torrent.

Check the local stream server against real peers (downloads a few MB of Big Buck Bunny):

```sh
cd src-tauri && cargo test live_local_stream -- --ignored --nocapture
```

### Browser preview

The UI can run in a normal browser with saved AniList data, without Tauri:

```sh
pnpm preview:ui   # http://localhost:1430
```

Refresh the saved data with `cd src-tauri && cargo test dump_preview_fixtures -- --ignored`
(catalog) and `cargo test live_episode_releases -- --ignored --nocapture` (Releases).

### ID mapping

The app ships an AniList → AniDB/MAL ID table and refreshes it weekly at runtime.
Rebuild the shipped copy with `node scripts/update-anime-ids.mjs`.

### Releases

Push a tag such as `v0.2.0` (the same version as in `src-tauri/tauri.conf.json`). The Release workflow builds macOS (Apple silicon and Intel) and Windows, and puts the files in a draft GitHub Release. Publish the draft; installed apps then offer the update.

The updater checks each download with the key in the `TAURI_SIGNING_PRIVATE_KEY` repository secret. Keep a copy of that key: without it, installed apps cannot update.

### Tests

```sh
pnpm test && pnpm typecheck
cd src-tauri && cargo test && cargo clippy --all-targets
```

## License

[GPL-3.0-or-later](LICENSE). Lokii bundles mpv and FFmpeg, which are GPL-licensed. The bundled builds come from [media-kit/libmpv-darwin-build](https://github.com/media-kit/libmpv-darwin-build) (macOS) and [shinchiro/mpv-winbuild-cmake](https://github.com/shinchiro/mpv-winbuild-cmake) (Windows); the [libmpv-v1 release](https://github.com/mhensberg2003/lokii/releases/tag/libmpv-v1) links to their sources.
