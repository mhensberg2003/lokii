# Lokii plan

Desktop anime streaming app for macOS and Windows. All user state stays on the device. Terms follow [CONTEXT.md](../CONTEXT.md). Visual references are in [DESIGN.md](./DESIGN.md).

## Decisions

| Area | Decision |
|---|---|
| Look | Calm dark, sidebar shell ([DESIGN.md](./DESIGN.md)) |
| Stack | Tauri 2 + Rust core, React + TypeScript UI ([ADR 0001](./adr/0001-tauri-and-rust-core.md)) |
| Player | Embedded libmpv with our own controls ([ADR 0002](./adr/0002-embedded-libmpv-player.md)) |
| Catalog | AniList GraphQL, no login |
| Index | AnimeTosho, with SeaDex for the Best Release |
| One Pace | Arcs as Shows in one Franchise, built from the One Pace Episode Guide and Nyaa; bundled snapshot, rebuilt weekly |
| Source | TorBox when connected, else Local Torrent |
| After Watched | Local Torrent: delete data, stop seeding. TorBox: remove the torrent (a Batch stays until its last Episode is Watched) |
| Unfinished Streams | Local data is deleted when the app quits |
| Seasons | Franchise shown as a season strip on the Show page |
| Chosen Release | Best Release, else 1080p with most seeders; user can override |
| v1 extras | Continue watching, Watchlist, Skip Segments |
| License | Open source, GPL-3.0-or-later (compatible with GPL builds of mpv and FFmpeg) |
| Distribution | GitHub Releases. Unsigned for now: macOS builds are ad-hoc signed, not notarized |

## Architecture

```
React UI (webview)                         Rust core (src-tauri)
─────────────────                          ─────────────────────
Home · Browse · Show · Search    ◄─IPC─►   catalog   AniList client, 30 req/min throttle, SQLite cache
Player overlay controls                    ids       AniList ↔ AniDB ↔ MAL mapping (Fribb anime-lists)
Downloads · Settings                       index     AnimeTosho search, SeaDex lookup, Chosen Release
                                           sources   TorBox client · Local Torrent (librqbit)
                                           stream    Stream lifecycle → one playable HTTP URL
                                           player    libmpv control: load URL, tracks, seek, events
                                           library   Watch Progress, Watchlist, Up Next (SQLite)
                                           onepace   One Pace Arcs and their Nyaa Releases (weekly rebuild)
                                           skip      AniSkip client
```

Every Stream ends as one HTTP URL that mpv plays:

- **TorBox**: `checkcached` → `createtorrent` if needed → poll `mylist` until ready → `requestdl` gives a direct link (valid 3 hours).
- **Local Torrent**: librqbit downloads with piece priority for the selected file and serves it on `127.0.0.1` with HTTP Range support.

The player never knows which Source it plays from.

### ID spaces

Three ID systems meet in Lokii:

| Service | Key |
|---|---|
| AniList, SeaDex | AniList ID |
| AnimeTosho | AniDB anime ID and episode ID |
| AniSkip | MAL ID (from AniList `idMal`) |

The `ids` module ships the Fribb anime-lists mapping in the app, refreshes it weekly, and stores it in SQLite.

### Local state

- SQLite database in the OS app-data folder: catalog cache, ID mapping, Watch Progress, Watchlist, Chosen Release per Show, settings.
- TorBox API key in the OS keychain (macOS Keychain, Windows Credential Manager).
- Local Torrent data in a temporary folder that the app empties on quit.

## External limits

| Service | Limit | Effect on design |
|---|---|---|
| AniList | 30 req/min in practice (docs say 90) | Cache every response. Batch queries. Home loads from cache first. |
| TorBox | 300 req/min; 60 uncached adds per hour | Poll `mylist` every 2–3 s only while a Stream prepares. |
| AniSkip | Needs exact episode length | Query after mpv reports the duration. |
| AnimeTosho | JSON base is `feed.animetosho.xyz/json` | Keep the base URL in one config value. |

## Milestones

Estimates are for one developer working with Claude, full days.

| # | Milestone | Done when | Estimate |
|---|---|---|---|
| M0 | **Spike: player + stream** | mpv plays an MKV with ASS subtitles inside a Tauri window on macOS and Windows, with a React button on top that pauses it. The same file plays from a librqbit local URL. | 3–5 days |
| M1 | Shell + catalog | Sidebar shell, Cmd/Ctrl+K search, Home (hero + 4 rows), Browse (genre tabs with rows), Show page with Franchise strip and episode grid. | 5–7 days |
| M2 | Finding Releases | Show page lists Releases per Episode, marks the Best Release, and auto-picks the Chosen Release. | 3–4 days |
| M3 | Sources + Streams | Play works through TorBox and Local Torrent. Activity panel and Downloads page. Delete-after-Watched rules. Settings page for TorBox. | 7–9 days |
| M4 | Player | Overlay controls, subtitle + audio panel, Skip Segments, next-Episode button, keyboard shortcuts. | 5–7 days |
| M5 | Library | Watch Progress saved every 5 s, Continue watching row, Up Next, Watchlist, Library page. | 3–4 days |
| M6 | Ship | libmpv bundled per OS and CPU (arm64 + x64 macOS, x64 Windows), ad-hoc signed macOS build, Tauri updater with its own signing key, install notes for the Gatekeeper and SmartScreen warnings. | 3–5 days |

Total: about 6–8 weeks.

## Risks

1. **libmpv inside a Tauri window.** macOS: solved in M0. mpv ignores `--wid` on macOS, so mpv renders through the render API into a `CAOpenGLLayer` below the transparent webview. HEVC 10-bit and styled ASS subtitles play with React controls on top. Windows: not tested yet; the plan is `--wid` on a child window below WebView2.
2. **Unsigned builds.** Without an Apple Developer account, macOS blocks the first launch ("Apple could not verify…"). Users open it via System Settings → Privacy & Security → Open Anyway. Windows SmartScreen shows a similar warning. The Tauri updater still works, because it uses its own signing key. Notarization can be added later without code changes.
3. **AniList rate limit.** 30 req/min is low for Home + Browse. Cache aggressively and prefetch in the background.
4. **Release matching.** Some Releases on AnimeTosho have no AniDB episode ID. Fallback: parse file names with an anime-aware parser (e.g. an `anitomy` port).

## Open questions

None at the moment.
