# Whisper

[![Build & Release](https://github.com/ptweezy/whisper/actions/workflows/build.yml/badge.svg)](https://github.com/ptweezy/whisper/actions/workflows/build.yml)

A small, fast REST client — like Postman or Insomnia, but a lightweight native desktop app with no account, no cloud, and no telemetry. Build requests, organize them into collections, use environments with `{{variables}}`, import cURL commands, and generate code snippets. Everything is stored locally; nothing leaves your machine except the requests you send.

## Install

Download the installer for your platform from [Releases](https://github.com/ptweezy/whisper/releases/latest):

| Platform | File |
|---|---|
| Windows 10/11 | `Whisper_x.y.z_x64-setup.exe` (or the `.msi`) |
| macOS (Apple Silicon and Intel) | `Whisper_x.y.z_universal.dmg` |

The builds are not code-signed, so the OS will warn on first launch:

- **Windows:** SmartScreen shows "Windows protected your PC" → click *More info* → *Run anyway*.
- **macOS:** Drag Whisper to Applications. If macOS says the app "cannot be opened" or "is damaged", open Terminal and run `xattr -cr /Applications/Whisper.app`, then open it again (or right-click → *Open* → *Open*).

## Features

- All HTTP methods, multi-tab workspace, two-way URL ↔ query-param sync
- Body types: JSON (validation + beautify), text, XML, form-urlencoded, multipart with files
- Auth helpers: Basic, Bearer, API key (header or query)
- Environments with `{{variable}}` substitution across URL, params, headers, body, and auth
- Collections and request history, persisted locally, with JSON export/import
- cURL import (bash quoting, ANSI-C `$'…'`, Windows cmd caret style, `--data-urlencode`)
- Code generation: cURL, JavaScript `fetch`, Python `requests`
- Response viewer: pretty JSON with highlighting, raw, HTML preview, image preview, every response header (including `Set-Cookie`), copy/save, cancel and timeout
- Dark/light theme, keyboard shortcuts (`Ctrl+Enter` send, `Ctrl+S` save)

## How it's built

| Path | What it is |
|---|---|
| `ui/index.html` | The entire UI — one HTML file, vanilla JS, no build step or dependencies. |
| `src-tauri/` | A [Tauri 2](https://tauri.app) shell. `src/lib.rs` is the native HTTP engine (`reqwest`) the UI calls over IPC, plus clipboard and save-file commands. |
| `app-icon.png` | Source icon; `tauri icon` generates the platform formats from it. |

Requests are made natively by the Rust core, so there are no browser restrictions on hosts or headers. `ui/index.html` also works opened directly in a browser ("browser mode"), where ordinary web-page limits apply.

## Develop

Requires [Rust](https://rustup.rs) and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS, plus the Tauri CLI (`cargo install tauri-cli --version "^2"` or `npm i -g @tauri-apps/cli`).

```bash
tauri icon app-icon.png   # once, generates src-tauri/icons
tauri dev                 # run with hot reload of ui/
tauri build               # produce installers in src-tauri/target/release/bundle
```

## Release

CI builds Windows (`windows-latest`) and a universal macOS binary (`macos-latest`) on every push, uploading the installers as workflow artifacts. Pushing a `v*` tag publishes them as a GitHub Release:

```bash
git tag v1.1.0 && git push origin v1.1.0
```

Bump `version` in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` to match.
