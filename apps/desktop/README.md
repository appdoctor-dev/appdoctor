# AppDoctor desktop

The native AppDoctor app for macOS and Windows: Rust, [GPUI](https://www.gpui.rs/)
and [gpui-kit](https://gpui-kit.com) (gpui-component). No webview. Keyword data,
rank history and credentials never leave the machine.

![Keywords for Things 3 after a rank check](docs/keywords.png)

## Running it

```sh
cargo run -p appdoctor-desktop --features no-license-check
```

`rust-toolchain.toml` pins Rust 1.99.1 (gpui-kit 0.8.1's gpui snapshot needs
std APIs newer than 1.96), and rustup installs it on first use. The first build
compiles gpui's whole tree; after that, an edit rebuilds only the app crate, which
builds at `opt-level = 0` and incrementally while dependencies build at
`opt-level = 2`, as in Cap's desktop-gpui.

- `no-license-check`: for contributors. Every feature works without an
  AppDoctor account. Official installers are built without it.
- `cargo test`: unit tests for App Store parsing, throttling, scoring, the
  store, both Apple JWTs and entitlement verification. The UI has no tests yet.
- `apps/desktop/scripts/bundle-dev.sh --features no-license-check` (macOS)
  wraps the debug binary in `target/debug/AppDoctor.app`. Use it when the app
  needs a bundle identity: accessibility and automation, keychain prompts that
  say "AppDoctor", or the Dock.

Environment variables:

| Variable | Effect |
|---|---|
| `APPDOCTOR_DATA_DIR` | Data folder. Default: `~/Library/Application Support/AppDoctor` on macOS, `%APPDATA%\AppDoctor` on Windows |
| `APPDOCTOR_EPHEMERAL_SECRETS=1` | Keep secrets in memory, not the keychain. An unsigned rebuild changes the binary's identity, so macOS asks again for keychain access every time |
| `APPDOCTOR_ENTITLEMENT_PUBLIC_KEY` | Debug builds only: verify licences against this key, e.g. a local API's dev key pair |
| `RUST_LOG` | e.g. `appdoctor=debug` |

## Architecture

```text
Cargo.toml              one workspace; the npm/turbo side of the repo never touches it
apps/desktop            the `appdoctor` binary: GPUI window, views, app state
crates/appstore         iTunes Lookup/Search, app URL parsing, storefronts, adaptive throttle
crates/store            SQLite (WAL) + migrations: apps, keywords, rank snapshots, popularity, competitors, settings
crates/scoring          keyword difficulty from the top 10 results (opt-level 3 even in dev)
crates/jwt              ES256 signing and P-256 key handling for both Apple APIs
crates/asc              App Store Connect: apps, app-info and version localizations; never submits for review
crates/apple-ads        Apple Ads Platform API 1.0: client secret, token, ACLs, popularity, suggestions
crates/entitlement      device-flow sign-in, offline Ed25519 licence check, update check
crates/secrets          typed OS keychain items (keyring 4: macOS Keychain, Windows Credential Manager)
```

Unlike Cap, the library crates share a single workspace with the GPUI app. Cap
keeps gpui in a workspace of its own because its root workspace carries
`[patch]` entries that gpui's tree doesn't want; there is no such conflict here,
so a single `Cargo.lock` builds shared deps (tokio, rustls/ring, hyper, serde) once.
`reqwest` stays on 0.12 so it reuses the rustls+ring stack that gpui's own HTTP
fork already builds, which also avoids aws-lc-rs, the default TLS provider in
0.13, which needs CMake and NASM on Windows.

**Threads.** GPUI owns the main thread and its executors. All network work runs
on a small tokio runtime (`runtime.rs`), and views await it from GPUI tasks.
`AppState` (`state.rs`) is the single model that views observe. Its operations
return `Task`s, and the calling view reports any error in its own window.

**Rank check** (`rank_check.rs`). Every keyword is searched in its storefront
(top 200). The job records the app's rank, a difficulty score from the top 10,
and those 10 apps (id, name, icon), then refreshes Apple Ads popularity if
Apple Ads is connected. Up to 8 searches are in flight at once, and the iTunes
client's throttle sets the real pace: it starts quick and halves when Apple
answers 403/429/503. Searches go through the `SearchSource` trait, so a source
that mirrors the App Store's real result order can replace the iTunes Search
API without touching anything else.

**Network endpoints.** The app talks only to Apple (`itunes.apple.com`, the
`mzstatic.com` icon CDN, `api.appstoreconnect.apple.com`,
`appleid.apple.com`, `api.ads.apple.com`) and to the AppDoctor API (sign-in,
licence, update check). There is no telemetry. The device ID is a random UUID
stored in the database.

**Entitlement.** Every licence decision goes through `src/entitlement.rs`, and
the rest of the app asks only `Account::is_entitled()`. Sign-in uses Better
Auth's device flow. The API's token is verified offline against
`appdoctor_entitlement::PUBLIC_KEY` and refreshed about once a day. Today only
*Check ranks* requires an entitlement.

## What works and what's stubbed

Working:
- Adding an app by App Store link or ID.
- Keywords per storefront, with a searchable country picker.
- A virtualized keyword table: popularity, difficulty, position, change since
  the last check, the apps in the top results, last update, sorting, and a
  right-click Delete.
- Check ranks, with live progress.
- Settings:
  - App Store Connect key: Issuer ID, Key ID and a `.p8` file picker. It is
    saved to the keychain and can be tested by listing apps.
  - Apple Ads: generate a key pair, copy the public PEM, enter the IDs, then
    test. The test resolves the ad account from `/v1/acls`.
  - Account sign-in, licence status, the API server address, the update check
    and the data folder.

Stubbed (a sidebar entry with an explanation):
- **Rankings**: history charts. The snapshots are already recorded.
- **Competitors**.
- **Localization**: the `asc` client is done; the editor isn't.
- **Screenshots**.

Planned, all as sections of this same window (one app, not separate tools):
- **Screenshot designer** (device frames, captions, every export size): a
  `crates/screenshots` with its pixel work in an opt-level-3 kernels crate,
  like Cap's.
- **App preview video designer.**
- **Competitor tracking and review analysis.** Each rank snapshot already
  stores the top 10 apps per keyword, which is the data for "who outranks me".
- **Local MCP server** (`crates/mcp`, localhost only), so the user's own agent
  can drive AppDoctor. For example, "localize my app into these languages"
  writes through `crates/asc`, which can't submit for review.
- **Bring-your-own AI provider or Ollama.**

## Windows

Platform code is `cfg`-gated: the computer name in `entitlement.rs`,
keychain backends chosen by keyring's target dependencies, data directories via
`dirs`, and the quit key (Cmd-Q / Ctrl-Q). Fonts come from the theme rather
than macOS font names. gpui-kit's `TitleBar` draws the window controls on
Windows. **Nothing has been built or run on Windows yet.** Cross-checking from
macOS needs the MSVC target plus a C toolchain for SQLite and ring. Still
unverified: the build itself, Credential Manager's 2.5 KB item limit against
our JSON items (about 400 bytes each), and how the title bar looks.

## Open decisions

1. **What the subscription gates.** Today only Check ranks; everything else works
   unlicensed. Gate more, add a trial, or gate nothing and sell only
   installers and updates?
2. **The licence public key.** `PUBLIC_KEY` is a placeholder until
   `npm run keys:generate` in `apps/api` produces the production key.
3. **The rank source.** The iTunes Search API's order is close to the real
   App Store's but not identical. A better source (the App Store web
   catalog's search) can sit behind `SearchSource`; whether to use it is a
   terms and robustness call.
4. **Apple Ads popularity by country.** The phrase `SEARCH` route documents no
   country filter, so for now the same score is filed under every storefront
   (TODO in `rank_check.rs`). The country-scoped
   `insights/apps/search-term-popularity` endpoint may be the right source.
   Verify both with a real account.
5. **Request pacing.** The defaults are 4 concurrent requests with a 300 ms
   starting gap, backing off on 403/429. Apple documents about 20 calls per
   minute for iTunes Search.
6. **Signing.** macOS keychain prompts and auto-update both want a Developer
   ID-signed, notarized bundle. Packaging, as in Cap's `bundle-gpui`, isn't
   done yet.
