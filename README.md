# Lokii

A calm desktop app for watching anime on macOS and Windows. Lokii finds a show on AniList, picks a good Release, and streams it through TorBox or a local torrent. All your data stays on your device.

> Status: early development (milestone M0, player spike). Not usable yet.

## Features (v1 plan)

- Browse and search anime, with data from AniList
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

Requirements: Rust (stable), Node 22, pnpm, and mpv.

```sh
brew install mpv   # macOS: provides libmpv
pnpm install
pnpm tauri dev
```

To test playback without the UI, set `LOKII_SPIKE_FILE` to a local video file:

```sh
LOKII_SPIKE_FILE=/path/to/video.mkv pnpm tauri dev
```

## License

[GPL-3.0-or-later](LICENSE). Lokii bundles mpv and FFmpeg, which are GPL-licensed.
