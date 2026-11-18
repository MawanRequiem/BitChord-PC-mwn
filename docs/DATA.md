# Data and storage

[Documentation home](REPOSITORY-GUIDE.md) · [Architecture](ARCHITECTURE.md) · [API reference](APIS.md)

## Models and identifiers

| Model | Fields and meaning |
| --- | --- |
| `Track` | Video ID, title, artist links, optional album, duration in seconds, thumbnails, optional track number and plays, and the music-video flag. The display model can be serialized into snapshots. |
| `Link`, `Thumb` | Optional browse ID with display name, or an image URL and its pixel dimensions. The thumbnail picker chooses an appropriate size and preserves relevant crop hints. |
| `Target`, `Card`, `Shelf`, `Chip` | Typed destinations for track, album, playlist and artist, plus presentation metadata, content sections and filter parameters. |
| `Collection`, `ArtistPage`, `Library` | Parsed collection/artist data and the signed-in library view. Album save ID, playlist ownership and per-entry IDs carry edit semantics. |
| `LyricsSet`, `Lyrics`, `translate::Extras` | Available provider entries, timing and credit metadata, and optional romanized or translated text. |
| `stream::Shared`, `stream::Stats` | Runtime buffer/read state and the decoder's format/quality reporting. The buffer holds bytes and tracks completion, cancellation and failure. |
| `jam::Item`, `jam::Party` | Wire track model and a local mirror of a party. Playback position and duration use milliseconds. Party identity, track identity, queue position and queue sequence serve different purposes. |

Sources [model.rs](../src/model.rs), [backend.rs](../src/backend.rs), [lyrics.rs](../src/lyrics.rs), [translate.rs](../src/translate.rs), [stream.rs](../src/stream.rs), [jam.rs](../src/jam.rs).

When changing playlists, carry the YouTube video ID and `setVideoId` together. Several rows can point at one video ID, and removal needs the specific playlist entry. Keep album browse IDs separate from the playlist IDs used to save albums. Keep list identity separate from the song selected to play for a music video; this affects the queue, history, download aliases and Jam synchronization. [ytm.rs](../src/ytm.rs) · [backend.rs](../src/backend.rs) · [downloads.rs](../src/downloads.rs)

Use explicit units at API changes. A track's catalog duration is seconds, audio stream/status and party positions use milliseconds or Rust `Duration`, lyric timings are processed in milliseconds, and SponsorBlock segment bounds are seconds. [model.rs](../src/model.rs) · [audio.rs](../src/audio.rs) · [lyrics.rs](../src/lyrics.rs) · [sponsorblock.rs](../src/sponsorblock.rs)

## Desktop data directory

The app chooses `YTM_DATA_DIR` first, then a `data` directory beside a portable executable or bundle, then the operating-system application data location from `ProjectDirs` using the name `ytm-rs`. Portable mode depends on a marker next to the executable or in the app bundle. [main.rs](../src/main.rs) · [update.rs](../src/update.rs)

| Location under the data directory | Contents |
| --- | --- |
| `app.ron` | eframe storage for window geometry and app settings. String values include source settings, theme/preferences, audio behavior, recent searches, sidebar caches, Jam IDs, and serialized queue/playback session state under `session`. |
| `local-playlists.json` | Local playlist snapshots with a title and ordered track metadata. Updates use a process lock and replace the JSON file after writing the new content. Snapshots can come from Google Takeout ZIP or CSV files. Downloaded audio stays in the downloads folder. |
| `renderer`, `proxy` | Bootstrap text settings read before portions of the UI start. The proxy value can contain credential-bearing configuration. |
| `ytmapi.json` | Cached YouTube Music web client/bootstrap data. |
| `webview/`, `webview-spotify/`, `webview-lyrics/` | Browser profiles for Google, Spotify and lyric verification. They can contain provider session or browser state. |
| `lyrics_jwt`, `translations/` | Lyrics verification cache and reusable translation/romanization results. The lyrics provider also keeps a recent in-memory result cache. |
| `thumbs/`, `motion/`, `media-cover/` | Downloaded or processed presentation assets for thumbnails, motion artwork and system media integration. |
| `downloads.json` | Map of video IDs to asset paths, durations, loudness and optional alias and track metadata. Download files live in the chosen folder, ordinarily the user's Music directory under `BitChord`. |
| `transfers/` and `spotify_queries.json` | Transfer checkpoints and Spotify query metadata cache. Checkpoints can store source tracks, candidate results, the destination and how far the job got. |
| `lastfm_queue.json` | Scrobbles awaiting delivery. Retryable failures can leave entries here for a later run. |
| `ejs/`, `updates/` | Playback solver cache and update staging. |
| `ytm-rs.log` and `ytm-rs.prev.log` | Release log and the preceding launch's log. Debug builds ordinarily log to stderr, and `YTM_LOG` can set a separate destination. |

Sources [main.rs](../src/main.rs), [ui/mod.rs](../src/ui/mod.rs), [ytmapi.rs](../src/ytmapi.rs), [lyrics.rs](../src/lyrics.rs), [translate.rs](../src/translate.rs), [downloads.rs](../src/downloads.rs), [transfer/mod.rs](../src/transfer/mod.rs), [transfer/spotify.rs](../src/transfer/spotify.rs), [lastfm.rs](../src/lastfm.rs), [renderer.rs](../src/renderer.rs), [proxy.rs](../src/proxy.rs).

The main desktop profile uses files and OS credential services. Catalog, account and playlist records are read and written through provider APIs. The running app keeps additional data in memory, including pages, pending requests, the playback buffer and prefetch, and temporarily cached provider results. [backend.rs](../src/backend.rs) · [ui/mod.rs](../src/ui/mod.rs)

## Account material and offline data

- `vault` stores Google and Spotify sign-in material, the Last.fm session, and hi-res source URLs under the service name `ytm-rs`. Windows uses Credential Manager. macOS and Linux try Keychain or Secret Service and can fall back to per-profile secret files. Credentials for a non-default data directory use a profile-scoped key. The system keyring path splits large secrets into numbered parts. [vault.rs](../src/vault.rs) · [webmusic.rs](../src/webmusic.rs) · [transfer/mod.rs](../src/transfer/mod.rs) · [lastfm.rs](../src/lastfm.rs)
- Existing hi-res source URLs migrate from `app.ron` when settings are saved. Palv renewal writes a replacement URL to the credential vault as soon as it arrives. The `auto_renew_palv` preference stays in `app.ron`. [ui/mod.rs](../src/ui/mod.rs) · [ui/palv.rs](../src/ui/palv.rs)
- Google session loading includes migration paths for old `account.json` and `rustypipe_cache.json` formats. The backup and recovery details are in `Session::load` and `Session::save`. Saved webview data and plain settings can also contain session-related information or tokens. [webmusic.rs](../src/webmusic.rs) · [ui/mod.rs](../src/ui/mod.rs)
- A download is playable when its indexed file can be opened. The saved audio is tagged with available metadata and artwork, and the index can map a music video to its alternate song file. Moving an audio file can invalidate its index path. [downloads.rs](../src/downloads.rs) · [backend.rs](../src/backend.rs)
- The `output_effects` preference in `app.ron` stores 16 band gains and independent channel levels. Defaults are flat EQ and 100% per channel. The settings affect playback output only. [audio/effects.rs](../src/audio/effects.rs) · [ui/mod.rs](../src/ui/mod.rs)
- Upload sends the selected file's contents to the Google account's music library. Transfer reads source-library metadata from Spotify and writes to YouTube Music. Lyrics, art matching and scrobbling share the operation-specific track metadata with the relevant configured service. The current Jam client's join payload can include a display name, stable user/device IDs and an avatar URL. [webmusic.rs](../src/webmusic.rs) · [transfer/mod.rs](../src/transfer/mod.rs) · [APIs](APIS.md) · [jam.rs](../src/jam.rs)

## Jam Durable Objects

The bundled Worker declares two SQLite-backed Durable Object classes. `ROOMS` maps normalized room codes to instances of `Room`. `STATS` uses a singleton named `global`. These structures describe the bundled Worker's data. The desktop's configured party service has a separate implementation and storage contract. [wrangler.jsonc](../server/wrangler.jsonc) · [Worker source](../server/src/lib.rs)

| Data | Storage shape | Lifecycle |
| --- | --- | --- |
| `Room` key `meta` | Host ID, guest playback flag, queue restriction, join sequence and optional empty timestamp | Created or reset on room creation. Updated on join, departure and control changes. |
| `Room` keys `state`, `tick` | Latest full playback/queue state and latest playhead tick, serialized as JSON strings | Host messages replace snapshots. A full state updates the tick projection. A join reads both snapshots. |
| WebSocket attachment | Member ID and display name, join order, rate-limit tokens/time and departure flag | Survives Durable Object hibernation. Departure is handled once. |
| `Stats.live` | `code TEXT PRIMARY KEY, members INTEGER, started INTEGER, seen INTEGER` | Reports update current counts and last-seen time. Zero members ends the current session. Stale rows older than 25 minutes are pruned when Stats receives a request. |
| `Stats.counters` | `k TEXT PRIMARY KEY, v INTEGER` | Stores cumulative counters, sums, first observation and all-time peaks. |
| `Stats.daily` | `day TEXT PRIMARY KEY, rooms INTEGER, joins INTEGER, peak_listeners INTEGER` | A row accumulates per UTC day. The public snapshot selects the latest 30 recorded dates. |

Rooms report creation, joins, departures, explicit ending and a heartbeat while occupied. An emptied room can admit members during a five-minute grace period. The room alarm later deletes its storage if it stayed empty. The public metrics response exposes aggregate values. Internal reports, `Stats.live` and structured Worker logs include the room code. [Worker source](../server/src/lib.rs)

## Website state

The site consists of static files and browser-side JavaScript. The homepage fetches GitHub repository and release metadata, with a session-storage cache for release data. The stats page fetches and renders public Worker aggregates. Invitation pages hand the room code to an app deep link or present the code to the user. [site/main.js](../site/main.js) · [site/404.html](../site/404.html) · [site/jam/index.html](../site/jam/index.html) · [site/jam/stats/index.html](../site/jam/stats/index.html)
