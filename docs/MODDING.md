# Modding guide

[Documentation home](REPOSITORY-GUIDE.md) · [Architecture](ARCHITECTURE.md) · [API reference](APIS.md)

## Build the desktop on Windows

Use Rust 1.95 or newer with the MSVC toolchain and the Visual Studio C++ build tools/Windows SDK. The Windows sign-in flows use WebView2. The release workflow builds Windows x64 and ARM64 and packages with Inno Setup. It also defines the signing, Linux and macOS build environments. [Cargo.toml](../Cargo.toml) · [release workflow](../.github/workflows/release.yml) · [signing notes](../CODE_SIGNING.md)

Run these commands from the checkout root.

```powershell
cargo build --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings

$env:YTM_DATA_DIR = Join-Path $PWD 'target\dev-profile'
cargo run --locked
```

Run `cargo build --release --locked` to build the Windows desktop executable at `target/release/bitchord.exe`. A separate profile starts with fresh app settings and its own credential scope. The same incoming deep-link and single-instance behavior applies on Windows. Check `src/deeplink.rs` when launching two copies, and close the running copy before testing startup if it intercepts the launch. [main.rs](../src/main.rs) · [deeplink.rs](../src/deeplink.rs) · [vault.rs](../src/vault.rs)

Useful local variables include `YTM_LOG` for a separate log file, `YTM_BACKGROUND` for an off-screen test window, `YTM_MUTE` to start silent, `YTM_NO_SHADER` for the fallback background, and `YTM_JAM_SERVER` to select a party service. Some ignored tests explicitly use a live account or change library data. Inspect the tests and use an isolated test account when those cases are needed. [main.rs](../src/main.rs) · [jam.rs](../src/jam.rs) · [transfer/mod.rs](../src/transfer/mod.rs)

## Run the bundled Jam Worker

From the checkout root, run the Worker and smoke client in separate terminals. The command installs Wrangler on demand through `npx` if it is unavailable locally. Rust needs the WebAssembly target. [server README](../server/README.md) · [Worker config](../server/wrangler.jsonc)

```powershell
rustup target add wasm32-unknown-unknown
Set-Location server
npx wrangler dev
```

```powershell
cargo run --locked --manifest-path server/smoke/Cargo.toml
```

The smoke client defaults to `ws://127.0.0.1:8787`. Set `JAM_URL` to use a different address. It exercises the bundled Worker's create/join and relay behavior. Desktop Jam testing requires a service that implements the desktop party contract or a coordinated protocol change. [Smoke client](../server/smoke/src/main.rs) · [Jam API comparison](APIS.md#jam-services)

## Where to change features

| Change | Start with | Follow through |
| --- | --- | --- |
| Search behavior and suggestion UX | [ui/pages.rs](../src/ui/pages.rs), [ui/responses.rs](../src/ui/responses.rs), [backend.rs](../src/backend.rs) | Inspect the query/round guards, filter routing, renderer parser fixtures and merge order in [ytm.rs](../src/ytm.rs). |
| Album, artist, playlist or library display | [ui/collections.rs](../src/ui/collections.rs), [ytm.rs](../src/ytm.rs) | Follow continuation, collection load IDs, edit permissions, playlist entry identity and failure states. |
| Queue actions, keyboard shortcuts and play/resume | [ui/actions.rs](../src/ui/actions.rs), [ui/queue.rs](../src/ui/queue.rs), [ui/keys.rs](../src/ui/keys.rs), [ui/playback.rs](../src/ui/playback.rs) | Check the play and seek generations, offline and streaming paths, handoff, shuffle, repeat, autoplay and Jam follower rules. |
| Local playlists and offline queue snapshots | [local_playlists.rs](../src/local_playlists.rs), [ui/pages.rs](../src/ui/pages.rs), [ui/actions.rs](../src/ui/actions.rs) | Check signed-out library access, snapshot ordering and repeated IDs, and playback from a saved row. |
| New media format or playback quality | [backend.rs](../src/backend.rs), [hires.rs](../src/hires.rs), [stream.rs](../src/stream.rs), [audio.rs](../src/audio.rs) | Check range/whole fetch, decoder/seek behavior, stats badges, prefetch reuse and same-generation upgrades. |
| EQ and channel levels | [audio/effects.rs](../src/audio/effects.rs), [audio.rs](../src/audio.rs), [ui/pages.rs](../src/ui/pages.rs) | Check flat pass-through, separate filter histories, headroom with overlapping boosts, and stereo output after mixing. |
| Lyrics source, translation or rendering | [lyrics.rs](../src/lyrics.rs), [translate.rs](../src/translate.rs), [ui/lyric_prefs.rs](../src/ui/lyric_prefs.rs), [ui/karaoke.rs](../src/ui/karaoke.rs) | Preserve provider keys and time units, empty/partial results, per-song offsets and independent translation alignment. |
| Sign-in or channel switching | [login.rs](../src/login.rs), [ytmapi.rs](../src/ytmapi.rs), [vault.rs](../src/vault.rs), [ui/account.rs](../src/ui/account.rs) | Exercise first sign-in, refresh, channel changes, sign-out, old profile migration and stale account-scoped responses. |
| Spotify transfer or bulk library edits | [transfer/mod.rs](../src/transfer/mod.rs), [transfer/spotify.rs](../src/transfer/spotify.rs), [ui/transfer.rs](../src/ui/transfer.rs), [backend.rs](../src/backend.rs) | Check pagination, duplicate destinations, review/resume paths and partial write handling. |
| Shared listening | [jam.rs](../src/jam.rs), [ui/jam.rs](../src/ui/jam.rs), [deeplink.rs](../src/deeplink.rs) | Check payload shape, clock offset, out-of-order queue/state frames, reconnect, role changes and links. Worker-only changes also need [Worker smoke scenarios](../server/smoke/src/main.rs). |
| Theme, graphics or window chrome | [theme.rs](../src/theme.rs), [preset.rs](../src/preset.rs), [ui/shell.rs](../src/ui/shell.rs), [ui/widgets.rs](../src/ui/widgets.rs), [renderer.rs](../src/renderer.rs) | Check Windows native titlebar, alternate renderers, mini mode, persisted preferences and responsive layout. |
| Packaging, update URLs and downloads page | [update.rs](../src/update.rs), [installer/bitchord.iss](../installer/bitchord.iss), [release workflow](../.github/workflows/release.yml), [site/main.js](../site/main.js) | Keep package naming, architecture detection, install paths and the release feed in agreement. |

## Fork-specific settings to verify before publishing

- The updater reads releases from the fork, so an upstream package cannot replace a build through the built-in updater. Crate repository metadata, site release links, and the installer support links retain their upstream values. [Cargo.toml](../Cargo.toml) · [update.rs](../src/update.rs) · [site/main.js](../site/main.js) · [installer script](../installer/bitchord.iss)
- Test and website workflows have push filters for `main`, while this fork's checked-out default branch is `rust-2.0`. Tests also have a pull-request and manual trigger. Release jobs trigger on version tags or manual dispatch. Review those filters if this fork should build and deploy from its default branch. [test workflow](../.github/workflows/tests.yml) · [Pages workflow](../.github/workflows/pages.yml) · [release workflow](../.github/workflows/release.yml)
- The desktop party service, its invitation host, the bundled Worker, and the dashboard URL are separate integration values. Confirm the desired contract before changing one. [jam.rs](../src/jam.rs) · [Worker config](../server/wrangler.jsonc) · [dashboard](../site/jam/stats/index.html)
