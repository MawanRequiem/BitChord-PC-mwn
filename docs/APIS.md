# API reference

[Documentation home](REPOSITORY-GUIDE.md) · [Architecture](ARCHITECTURE.md) · [Code reference](code-reference/README.md)

## Desktop request and response API

The desktop calls `Backend::request(Req)` inside the process. `Resp` messages go to the UI through a Crossbeam channel. The following table lists every request variant at the recorded revision. Responses may arrive out of order. The backend and response handlers use account epochs, collection load IDs, query rounds and playback generations where applicable. A failed foreground request generally emits `Error`. Quiet requests can log a failure and allow the current UI state to stand. [backend.rs](../src/backend.rs) · [ui/responses.rs](../src/ui/responses.rs)

| Requests | Main results and effects |
| --- | --- |
| `Home(params)`, `HomeMore(token)`, `MoreQuickPicks(seed)`, `Explore(ids)` | `Home`, `HomeMore`, `QuickPicksMore`, and `Explore` deliver browse shelves, continuations, related tracks, or chart and new-release cards. |
| `Search(query)`, `Suggest{query,round}`, `ForgetSearch(token)` | `Search` returns a query plus typed tracks, albums, artists and playlists. `Suggest` echoes query and round. `SearchForgotten` acknowledges a history removal. |
| `Album{id,load}`, `Artist(id)`, `Playlist{id,load,stream}` | `Collection` or `Artist` returns display data. A streamed playlist emits `CollectionMore` with load and ID, page tracks, entry IDs and the remaining-pages flag. |
| `Library`, `History`, `Uploads`, `Account`, `Channels` | Returns the corresponding account view. `Library` has independent failure flags for saved playlists and albums. `Channels` includes the epoch that makes a later selection valid. |
| `SwitchChannel{page_id,epoch}`, `SetCookie(cookie)`, `Logout` | `SwitchedChannel` conveys the active channel, new epoch or failure. `LoggedIn` reflects sign-in state. |
| `ServerQueue`, `Radio{seed,song_of}`, `PlayCollection{target}` | Returns a cross-device queue when present, seeded radio entries, or `PlayQueue` for the requested collection. |
| `Upload(paths)`, `Download{track,for_video}` | `Uploaded` carries success count and file failures. `Downloaded` carries title and local path. The optional video ID can alias a downloaded song version. |
| `Lyrics(track)`, `Segments{video_id}`, `LyricsExtras{video_id,source,lines,lang,target}` | `Lyrics`, `Segments`, or `LyricsExtras` returns per-track presentation data. Provider or clip identity travels with results so the UI can apply them in the correct context. |
| `Art{key,url}`, `AppleArtist{id,name}`, `AppleCover{key,album,artist,title}` | Returns `Art`, `AppleArtist`, or `AppleCover` with the corresponding lookup key or artist ID. |
| `ArtistSignature{key,endpoint,name}`, `CheckHiResSource{key,link}` | `ArtistSignature` returns an optional prepared asset. `HiResSourceChecked` returns a displayable result per key. |
| `Play{video_id,generation,at,report,song_of,song,track}`, `Prefetch{video_id,song_of,track}` | Playback dispatches `AudioCmd::Start` or `PlayError` and may emit `PlayingAs`, `StreamStats`, `AudioInfo`, and `UpgradingQuality`. Prefetch resolves ahead of time and retains a reusable stream. |
| `Like{video_id,like}`, `LikeMany{video_ids,like}`, `Dislike(video_id)` | A single like or unlike returns `Liked`. Bulk operations can emit `LikeProgress` and end with `LikedMany`, including failures. Dislike has a remote write side effect. |
| `CreatePlaylist{title,video_ids}`, `EditPlaylist{id,title,description,privacy}`, `AddToPlaylist{playlist_id,video_ids}`, `RemoveFromPlaylist{playlist_id,items}`, `DeletePlaylist(id)` | `PlaylistCreated` returns ID and song count. Other playlist edits report `PlaylistChanged` as applicable. Each removal item is a video ID and its entry ID. |
| `Collaboration(id)`, `SetCollaboration{info,change}`, `SaveCollection{id,save}` | Collaboration reads or refreshes the collaboration settings and invite link. Save or unsave updates the account library. |

The synchronous audio command channel takes `Start`, `Replace`, `Upgrade`, `Pause`, `Resume`, `Stop`, `Handoff`, `Seek`, `SeekLive`, `Volume`, `DeviceLost` and `SetDevice`. `Start`, `Replace` and `Upgrade` carry prepared decoders. `Replace` also carries a seek number. `AudioStatus` supplies shared position, duration, device/transition-related state and generation. `Finished`, `Ending`, `Interrupted` and `Error` carry event notifications. [audio.rs](../src/audio.rs)

## YouTube Music and YouTube

The base for the catalog transport is `https://music.youtube.com/youtubei/v1/`. Requests send JSON with the web client context. The account-supplied variants add the cookie and signed authorization, plus the active account and channel as required. Signed requests validate the HTTPS destination, and the authenticated client rejects redirects. A request can fail due to HTTP status or an error in the response body. The wrappers in `ytm.rs` translate renderer JSON into the desktop's models. [ytmapi.rs](../src/ytmapi.rs) · [ytm.rs](../src/ytm.rs) · [webmusic.rs](../src/webmusic.rs)

| Interface | Inputs | Output or effect |
| --- | --- | --- |
| `POST browse` | `browseId`, optional `params`, or a `continuation` | Home/chips, album, artist, playlist, charts, new releases, saved items, history, and uploads. Some feeds and continuations require account context. |
| `POST search` | `query`, filter `params` | Song/video and collection/artist renderer rows. Search result families are parsed according to filter. |
| `POST music/get_search_suggestions` | `input` | Suggested queries and items, with feedback tokens where provided. |
| `POST feedback` | `feedbackTokens` | Removes an applicable search-history item. |
| `POST next` | Seed or playback context, playlist ID and/or continuation depending on call | Radio entries, paired song information and signed-in cross-device queue. |
| `POST account/account_menu`, `POST account/accounts_list` | Client context, account credentials | Account name/photo or channel options. Channel discovery can use the Google account context independently of the selected channel. |
| `POST like/like`, `POST like/removelike`, `POST like/dislike` | `target.videoId`; likes also support `target.playlistId` | Updates track ratings or saved collection state. |
| `POST playlist/create` | Title, description, privacy and optional `videoIds` | Creates a private playlist in exposed app flows and returns its ID. |
| `POST browse/edit_playlist` | Playlist ID and action list | Adds or removes entries, updates name/description/privacy, or changes collaboration. Duplicate additions request skip behavior. |
| `POST playlist/delete` | `playlistId` | Deletes a remote playlist. |
| `POST get_panel` | Collaboration panel ID and page-provided parameters | Returns controls and invite metadata used for collaboration edits. |
| `POST player` | `videoId` and client-specific playback context | Stream descriptors or playback tracking URLs, depending on client and use. Code uses signed-in Music web and anonymous YouTube player contexts in separate modules. |
| Web bootstrap and account maintenance | `GET music.youtube.com/`, `GET www.youtube.com/results?search_query=`, and `POST accounts.youtube.com/RotateCookies` | Reads web client and region context, inspects the imported account session, and maintains sign-in cookies. |
| Direct music upload | `POST upload.youtube.com/upload/usermusic/http` with the account index and file metadata, then `POST` bytes to the returned upload URL | Resumable protocol start followed by upload and finalize. The app checks a 300 MiB maximum after reading the selected file. |
| Playback history and watch time | Player-provided URLs, playback nonce and progress ranges via signed `GET` | Reports signed-in activity to YouTube when the session and playback state allow it. |
| Media CDN and helper paths | Resolved URL, user agent, HTTP range or query parameters, plus renewed URL when applicable | Delivers compressed bytes. `innertube.rs` also calls the direct Google-hosted `next` API. `webmusic.rs` checks entitlement with signed YouTube web browse. |

Sources [ytm.rs](../src/ytm.rs), [ytmapi.rs](../src/ytmapi.rs), [webmusic.rs](../src/webmusic.rs), [innertube.rs](../src/innertube.rs), [stream.rs](../src/stream.rs).

## Jam services

The current desktop defaults to `https://api.bitchord.kushagrasingh.in`. Its Settings and `YTM_JAM_SERVER` can select another base. The bundled Worker exposes `/jam/{CODE}` with a different message envelope. The smoke client targets that Worker contract. The dashboard has its own hard-coded Worker stats URL. A custom server must implement the interface required by each consumer. [jam.rs](../src/jam.rs) · [Worker](../server/src/lib.rs) · [smoke client](../server/smoke/src/main.rs) · [dashboard](../site/jam/stats/index.html)

### Current desktop party contract

| Transport | Desktop sends | Desktop consumes |
| --- | --- | --- |
| `POST /api/parties` | JSON `userId`, `deviceId`, `displayName`, optional `avatarUrl`, and `maxMembers` | Party code, access token and party snapshot. Default requested size is five. |
| `POST /api/parties/{code}/join` | Same member identity fields | Party code, access token and snapshot. HTTP failure responses can contain `error` and `message`. |
| `WS /ws/parties/{code}` | Authorization bearer header and JSON frames keyed by `type` | `welcome`, `state`, `queue`, `members`, `pong`, `activity`, `error`, and `bye`. |
| `POST /api/parties/{code}/leave` | Party bearer token | Best-effort departure. |

The client emits `type=control` with an `action` for `play`, `pause`, `seek`, `setTrack`, `setQueue` and `setHostOnlyControl`. It also sends `syncQueue`, `report` with `positionMs` and `isPlaying`, and `ping` with `clientMs`. The party mirror uses `playback`, `queue`, members, host controls, timestamps, and a queue sequence. Track items may include `videoId`, `title`, `artist`, `thumbnailUrl`, `durationMs` and `fromAutoplay`. The desktop code accepts six normalized alphanumeric characters from its configured alphabet, including 0 and 1. [jam.rs](../src/jam.rs)

### Bundled Worker HTTP and WebSocket API

| Route | Inputs | Response |
| --- | --- | --- |
| `GET /` | None | Plain-text health/identity response. |
| `GET /stats` | None | JSON aggregate described below. Sets `Cache-Control: no-store` and `Access-Control-Allow-Origin: *`. |
| `GET /jam/{CODE}?create=0&name=...&v=1` | WebSocket upgrade and six-character code. Use `create=0` to join or `create=1` to create. Name defaults to Guest and is cleaned to 32 characters. An omitted `v` is accepted by this revision. | WebSocket upgrade or HTTP 400 for a malformed code, 426 for missing upgrade. Refused membership sends a frame and then closes. |
| Internal `POST /report` on `Stats` | `{code,ev,members}` submitted by a room object | Updates metrics. The public Worker routes only `/stats` to the Stats object. |

The Worker normalizes codes to uppercase using `ABCDEFGHJKMNPQRSTUVWXYZ23456789` and holds up to 16 simultaneous members per room. The following frame names use its `t` field. [Worker implementation](../server/src/lib.rs)

| Direction | Frame | Fields and behavior |
| --- | --- | --- |
| Client → server | Literal text `ping` | Runtime auto-responds with literal `pong`. |
| Client → server | `ping` | JSON `c`; server replies `{t:"pong",c,s}` with its clock. |
| Client → server | `state` | Host sends `vid`, numeric `pos`, boolean `playing`, object-array `queue`, numeric `index`, and object `by`. Stored with a server timestamp and replaces the stored tick projection. |
| Client → server | `tick` | Host sends `vid`, `pos` and `playing`. Stored with server `ts`. |
| Client → server | `control` | Host sends boolean `on` for guest playback controls and optional boolean `queue` for guest queue edits. |
| Client → server | `req` | `cmd.k` is `add` or `next` with object `track`, `seek` with numeric `pos`, `jump` with numeric `index`, `toggle`, `skip` or `prev`. Guests need the relevant permission. |
| Client → server | `end`, `bye` | `end` is host-only and clears the room. `bye` departs and closes that member's socket. |
| Server → client | `welcome` | `you`, `host`, `control`, `queue`, `members`, server `now`, stored `state` and `tick` where present. |
| Server → client | `state`, `tick` | Server adds `ts` and broadcasts to the other sockets. |
| Server → client | `members`, `control` | Updates host/member list or current guest control and queue permissions. |
| Server → client | `req` | Forwarded to the host with `from`, `name` and `cmd`. |
| Server → client | `ended`, `error` | A terminated room sends `ended`. An error carries `code` and `msg`. |

Membership close codes are `4004` for `not_found`, `4009` for `taken`, `4029` for `full`, and `4000` for a bad handshake parameter. Per-frame `bad`, `rate` and `denied` errors leave the socket open. Frame size is capped at 256 KiB, and the token bucket starts with 20 messages and replenishes five per second. The earliest remaining member becomes host on host departure. Empty rooms get five minutes for a return before storage is cleared. [Worker implementation](../server/src/lib.rs) · [smoke scenarios](../server/smoke/src/main.rs)

The stats response contains `live{rooms,listeners,largest_room}`, `totals{rooms_created,joins,guest_joins,rooms_ended,sessions,avg_session_min}`, `peaks{rooms,listeners}`, `daily[{day,rooms,joins,peak_listeners}]`, `since`, and `now`. Times are milliseconds since Unix epoch, with `since` nullable. The response returns up to 30 recorded UTC days, newest first. [Worker implementation](../server/src/lib.rs)

## Other outbound interfaces

The following are source-observed clients and data exchanges. Credentials and provider configuration live with the local profile. [Data and storage](DATA.md)

| Provider or boundary | HTTP or IPC contract | Inputs and use |
| --- | --- | --- |
| Google and Spotify login | Browser navigation to Google or Spotify sign-in, then provider target site | Account session is brought back from the login webview. [login.rs](../src/login.rs) |
| Better Lyrics streaming gateway | `POST lyrics.api.dacubeking.com/v2/lyrics`; challenge page and `POST /verify-turnstile` | Video ID, optional song/artist/album/duration and token. Parses streaming responses, caches a gateway JWT and can renew it after a forbidden response. [lyrics.rs](../src/lyrics.rs) · [turnstile.rs](../src/turnstile.rs) |
| Unison | `GET unison.betterlyrics.org/lyrics`; `POST /translate` | Lyric query uses video ID, metadata and duration. Translation uses `lines`, target `to` and optional source `from`, returning per-line translation and romanization. [lyrics.rs](../src/lyrics.rs) · [translate.rs](../src/translate.rs) |
| lrc.red | `GET /search.json?q=...`; `GET /s/{isrc}.ttml` | Searches by song and artist, scores results, and parses TTML for timing and metadata. [lyrics.rs](../src/lyrics.rs) |
| Google translation | `POST translate.googleapis.com/translate_a/single` or `clients5.google.com/translate_a/single` | Query specifies client, languages and result types. Form supplies lines as text for fallback translation or romanization. [translate.rs](../src/translate.rs) |
| SponsorBlock | `GET sponsor.ajay.app/api/skipSegments/{hashPrefix}` | Sends a four-hex-character video ID hash prefix and the selected categories. Uses the matching returned video ID and checks segment duration before use. [sponsorblock.rs](../src/sponsorblock.rs) |
| Hi-res addons | `GET {base}/manifest.json`, `GET {base}/search?q=...&quality=...`, `GET {base}/stream/{trackId}?quality=...` | Reads addon capabilities, searches by track metadata, then resolves and validates stream choices. Audio retrieval may use an HTTP byte range probe. Endpoint settings can include tokens. [hires.rs](../src/hires.rs) |
| Palv session renewal | Palv dashboard's HTTPS token renewal action | Uses an add-on token already issued by Palv. On success the client replaces the saved source URL with the later-expiring token. A request for browser action pauses automatic attempts. [palv.rs](../src/palv.rs) · [Renewal UI](../src/ui/palv.rs) |
| Apple music catalog and media | `GET music.apple.com/us/browse` and web assets; `GET amp-api.music.apple.com/v1/catalog/us/search`, `/albums/{id}`, artist pages, posters and motion media | Obtains web catalog context, matches artist/album/song metadata, and extracts imagery or playable motion artwork. [apple.rs](../src/apple.rs) · [motion.rs](../src/motion.rs) |
| Artist signature source | `GET` configured URL, using an artist placeholder or artist query, followed by an asset `GET` | Selects a supported, suitable raster asset from service JSON and prepares it for the header. [artist_signature.rs](../src/artist_signature.rs) |
| Spotify import | Spotify web page/CDN; `GET open.spotify.com/api/token`; `POST clienttoken.spotify.com/v1/clienttoken`; `POST api-partner.spotify.com/pathfinder/v1/query`; `GET api.spotify.com/v1/me/playlists` and its pagination URLs | Loads account or playlist data. Pathfinder operations include `profileAttributes`, `libraryV3`, `fetchPlaylist`, and `fetchLibraryTracks`. The client also has a remote bootstrap data dependency for the web token flow. [transfer/spotify.rs](../src/transfer/spotify.rs) |
| Last.fm | `GET ws.audioscrobbler.com/2.0/` using `auth.getToken` and `auth.getSession`; POST form to the same endpoint for `track.updateNowPlaying` and `track.scrobble` | Desktop auth confirmation, now-playing track metadata and batched history. Failed scrobbles can remain on disk for retry. [lastfm.rs](../src/lastfm.rs) |
| Discord and OS controls | Discord Rich Presence IPC, native media controls and tray events | Shares player metadata with the local Discord client when enabled and receives local playback actions. [discord.rs](../src/discord.rs) · [media.rs](../src/media.rs) · [tray.rs](../src/tray.rs) |
| Proxy and availability checks | `GET music.youtube.com/generate_204`, YouTube Music homepage, and proxy handshakes for relevant transports | Checks whether the configured route works and how YouTube Music responds from that route. [proxy.rs](../src/proxy.rs) · [ytmapi.rs](../src/ytmapi.rs) |
| Artwork and updater | `GET` remote thumbnails and media artwork; GitHub Releases latest/list/tag requests and asset downloads | Size-aware images feed the UI. The updater selects a package for the current platform. The site separately queries GitHub repository and latest-release data. [images.rs](../src/images.rs) · [update.rs](../src/update.rs) · [site/main.js](../site/main.js) |
| Playback tooling | `GET www.youtube.com/iframe_api` and player JavaScript requests | Supports native playback URL resolution with a local solver cache. [jsc.rs](../src/jsc.rs) |

## Desktop launch interfaces

The executable accepts invitation URLs as arguments and forwards them to a running instance when supported. It parses `bitchord://party/CODE`, `bitchord://party?code=CODE`, and HTTP(S) invitation links ending in `/invite/CODE`. The deep link can carry a custom service address. Desktop child-process modes use `--login`, `--login-spotify`, or `--turnstile` followed by a webview profile path. [main.rs](../src/main.rs) · [deeplink.rs](../src/deeplink.rs) · [login.rs](../src/login.rs) · [turnstile.rs](../src/turnstile.rs)
