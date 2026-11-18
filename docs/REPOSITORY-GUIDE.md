# BitChord PC developer guide

The architecture and function inventory for **MawanRequiem/BitChord-PC-mwn**, branch **rust-2.0**, use commit [c7936d871a915567054ae05d39adc4c0b88e1358](https://github.com/MawanRequiem/BitChord-PC-mwn/tree/c7936d871a915567054ae05d39adc4c0b88e1358) as their baseline. The desktop Cargo package is **bitchord 2.0.1**. The checkout also includes a separate Jam relay, its smoke client, and a static website.

| Start here | What it covers |
| --- | --- |
| [Architecture and data flow](ARCHITECTURE.md) | Startup, thread ownership, playback, search, account state, lyrics, transfers, and shared listening |
| [API reference](APIS.md) | Desktop request/response types, public and internal Worker routes, both Jam protocols, and outbound integrations |
| [Data and storage](DATA.md) | Data models, IDs, local settings, credentials, caches, offline audio, and SQLite |
| [Modding guide](MODDING.md) | Build commands, useful entry points, regression checks, and fork-specific integration boundaries |
| [Code reference](code-reference/README.md) | Source-linked declarations, inputs and return types, and implementation notes for first-party functions |

## Repository map

| Path | Responsibility |
| --- | --- |
| [src/main.rs](../src/main.rs) and [src/ui/](../src/ui/) | Desktop startup, window and UI state, navigation, settings, queue actions, and event handling |
| [src/backend.rs](../src/backend.rs), [src/ytmapi.rs](../src/ytmapi.rs), and [src/ytm.rs](../src/ytm.rs) | Desktop background work, authenticated or anonymous InnerTube requests, and response translation |
| [src/audio.rs](../src/audio.rs), [src/stream.rs](../src/stream.rs), [src/opus.rs](../src/opus.rs), and [src/dash.rs](../src/dash.rs) | Output, media buffering, decoding, and special formats |
| [src/transfer/](../src/transfer/), [src/lyrics.rs](../src/lyrics.rs), and [src/jam.rs](../src/jam.rs) | Spotify import, lyrics providers, and the desktop shared-listening client |
| [server/](../server/) | Standalone Rust Cloudflare Worker and SQLite-backed Durable Objects, with a separate smoke client |
| [site/](../site/) | GitHub Pages marketing site, invitation landing, and Jam statistics dashboard |
| [assets/](../assets/) and [build.rs](../build.rs) | Fonts, artwork, theme presets, executable resources, and a generated embedded theme-cover lookup |
| [installer/](../installer/), [linux/](../linux/), [macos/](../macos/), and [.github/workflows/](../.github/workflows/) | Packaging, releases, CI tests, and Pages deployment |

## Important before modifying the fork

The desktop runs catalog and playback logic inside the app process. Each listener resolves and streams audio on their device. The Worker in `server/` relays one version of the Jam protocol and aggregates room statistics. The current desktop speaks a different Jam protocol to its configured service, whose implementation is absent from this checkout. Deploying `server/` is useful for the bundled smoke client and requires protocol work to support the current desktop. See the [Jam API comparison](APIS.md#jam-services).

The review is based on local source and configuration. It covers call paths and payload definitions as implemented at the pinned commit. Provider behavior, deployed service configuration, and end-to-end desktop playback require a live environment to confirm.

## Using the fork

- Open **Settings → Playback** for the **16-band equalizer** and independent left/right levels. Each slider explains what boosting and cutting its frequency range does.
- On a playlist, open **More → Save a local copy**. From the queue, use **Save queue locally**. Open either snapshot under **Library → Local playlists** while signed out. The app streams tracks that are absent from Downloads.
- To import playlists from your Google account, choose **YouTube and YouTube Music** in [Google Takeout](https://takeout.google.com/) and select **playlists** from the included data. Download a ZIP archive. In **Library → Local playlists → Import Google Takeout**, select the ZIP or the extracted playlist CSV files. Takeout sometimes exports video IDs alone. The app shows those IDs when titles are missing. [Import code](../src/local_playlists/takeout.rs)
- Google sign-in still imports session cookies. The fork leaves Windows device binding available, confines authenticated requests to approved HTTPS destinations, and keeps the session away from external playback helpers. Signing in can still require renewing the session.

### Takeout import flow

`App::choose_takeout_files` opens the file picker and parses the selection on a worker thread. `takeout::read` dispatches ZIP and CSV input, reads playlist CSV entries directly from ZIP, and preserves the order of valid video IDs. `App::poll_takeout_import` sends the parsed snapshots to `save_all`, which merges them into the existing local playlist file in one write. Re-importing identical playlist data updates the same snapshot. [Import parser](../src/local_playlists/takeout.rs) · [UI actions](../src/ui/actions.rs) · [Local storage](../src/local_playlists.rs)

The importer reads UTF-8 CSV with a `Video ID` or `ID Video` column. For a ZIP whose folder names differ from `YouTube and YouTube Music/playlists`, extract the playlist CSV files and select them directly. The import reads local files. Playback resolves playable video IDs through the app's normal streaming or download path.
