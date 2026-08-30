# Whisper

A small, fast REST client — like Postman or Insomnia, but self-contained and dependency-free. Build requests, organize them into collections, use environments with `{{variables}}`, import cURL commands, and generate code snippets. Everything is stored locally; nothing leaves your machine except the requests you send.

Whisper ships as two pieces:

| File | What it is |
|---|---|
| `rest-client.html` | The entire UI — one HTML file, vanilla JS, no build step. Works opened directly in any browser. |
| `whisper-server.ts` | The **native engine** — a tiny Deno server that embeds the UI, serves it at `http://127.0.0.1:7788`, and sends requests natively. |

Opened directly in a browser, the page runs in *browser mode* and some requests/headers are restricted by browser security. Run through the compiled companion app, requests are made natively: any host, any header, every response header visible (including `Set-Cookie`).

## Features

- All HTTP methods, multi-tab workspace, two-way URL ↔ query-param sync
- Body types: JSON (validation + beautify), text, XML, form-urlencoded, multipart with files
- Auth helpers: Basic, Bearer, API key (header or query)
- Environments with `{{variable}}` substitution across URL, params, headers, body, and auth
- Collections and request history, persisted in localStorage, with JSON export/import
- cURL import (bash quoting, ANSI-C `$'…'`, Windows cmd caret style, `--data-urlencode`)
- Code generation: cURL, JavaScript `fetch`, Python `requests`
- Response viewer: pretty JSON with highlighting, raw, HTML preview, image preview, full header list, copy/download, cancel and timeout
- Dark/light theme, keyboard shortcuts (`Ctrl+Enter` send, `Ctrl+S` save)

## Run it

Grab a compiled binary (see Releases or build below) and double-click — your browser opens with the app. Or run from source:

```bash
deno run --allow-net --allow-read --allow-run whisper-server.ts
```

Or just open `rest-client.html` in a browser (browser mode).

## Build the binaries

Requires [Deno](https://deno.com) 2.x. From the repo root:

```bash
deno compile --allow-net --allow-read --allow-run --include rest-client.html --target x86_64-pc-windows-msvc --output dist/Whisper-Windows.exe whisper-server.ts
deno compile --allow-net --allow-read --allow-run --include rest-client.html --target aarch64-apple-darwin --output dist/Whisper-macOS-AppleSilicon whisper-server.ts
deno compile --allow-net --allow-read --allow-run --include rest-client.html --target x86_64-apple-darwin --output dist/Whisper-macOS-Intel whisper-server.ts
```

All three targets cross-compile from any OS. The HTML is embedded, so each binary is fully self-contained. `dist/README.txt` is the end-user guide to include alongside the binaries (covers the unsigned-app prompts on Windows SmartScreen and macOS Gatekeeper).

## Security model

The companion listens on `127.0.0.1` only. Its request engine requires a per-session random token that only the served page knows (cross-origin pages can't read it — no CORS headers are ever emitted), and a Host-header allowlist blocks DNS-rebinding. Other devices and websites cannot use the engine.
