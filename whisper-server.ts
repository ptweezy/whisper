/**
 * Whisper engine — tiny local companion for rest-client.html.
 *
 * Serves the UI at http://127.0.0.1:<port> and proxies its HTTP requests
 * natively, so they are not subject to browser CORS rules.
 *
 * Compile (from the folder containing rest-client.html):
 *   deno compile --allow-net --allow-read --allow-run --include rest-client.html \
 *     --target x86_64-pc-windows-msvc --output dist/Whisper-Windows whisper-server.ts
 *   (targets: x86_64-pc-windows-msvc, aarch64-apple-darwin, x86_64-apple-darwin)
 */

const VERSION = "1.0.0";
const MAX_RESPONSE_BYTES = 256 * 1024 * 1024;

const token = crypto.randomUUID();
const html = await Deno.readTextFile(new URL("./rest-client.html", import.meta.url));

function b64EncodeUtf8(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(bin);
}
function b64DecodeUtf8(s: string): string {
  const bin = atob(s);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new TextDecoder().decode(bytes);
}

let port = 7788;

// Reject requests whose Host header is not our own loopback address. A remote
// website cannot read our responses (we never send CORS headers), and this
// additionally blocks DNS-rebinding tricks that would make an attacker's
// domain resolve to 127.0.0.1 and become "same-origin" with this server.
function hostOk(req: Request): boolean {
  const host = (req.headers.get("host") || "").toLowerCase();
  return host === `127.0.0.1:${port}` || host === `localhost:${port}`;
}

function metaHeader(obj: unknown): string {
  return b64EncodeUtf8(JSON.stringify(obj));
}
function fail(message: string): Response {
  return new Response(null, {
    status: 200,
    headers: { "x-whisper-meta": metaHeader({ error: message }), "cache-control": "no-store" },
  });
}
// "fetch failed" alone is useless — surface the underlying cause chain
function describeError(e: unknown): string {
  let msg = e instanceof Error ? e.message : String(e);
  let cause = e instanceof Error ? e.cause : undefined;
  while (cause) {
    const cm = cause instanceof Error ? cause.message : String(cause);
    if (cm && !msg.includes(cm)) msg += ": " + cm;
    cause = cause instanceof Error ? cause.cause : undefined;
  }
  return msg;
}

async function proxy(req: Request): Promise<Response> {
  let target = "";
  try { target = decodeURIComponent(req.headers.get("x-whisper-url") || ""); }
  catch { return fail("Invalid target URL encoding."); }
  if (!/^https?:\/\//i.test(target)) return fail("Only http:// and https:// URLs are supported.");

  const method = (req.headers.get("x-whisper-method") || "GET").toUpperCase();
  let headerMap: Record<string, string> = {};
  try {
    const raw = req.headers.get("x-whisper-headers");
    if (raw) headerMap = JSON.parse(b64DecodeUtf8(raw));
  } catch { return fail("Invalid header payload."); }

  const upstreamHeaders = new Headers();
  for (const [k, v] of Object.entries(headerMap)) {
    try { upstreamHeaders.set(k, String(v)); }
    catch {
      // Headers.set throws for bad names AND bad values — tell them apart
      let nameOk = true;
      try { new Headers({ [k]: "x" }); } catch { nameOk = false; }
      return fail(nameOk
        ? `The value of header "${k}" is invalid — header values must be plain ISO-8859-1 text with no line breaks (check for smart quotes or non-breaking spaces).`
        : `"${k}" is not a valid header name.`);
    }
  }

  let body: ArrayBuffer | undefined;
  if (!["GET", "HEAD"].includes(method)) {
    try {
      const buf = await req.arrayBuffer();
      if (buf.byteLength > 0) body = buf;
    } catch {
      return fail("Request cancelled.");
    }
  }
  // multipart bodies: the page leaves Content-Type unset so the browser adds
  // the boundary — carry that transport Content-Type through to the target
  const transportCt = req.headers.get("content-type");
  if (body && !upstreamHeaders.has("content-type") && transportCt) {
    upstreamHeaders.set("content-type", transportCt);
  }

  let res: Response;
  try {
    res = await fetch(target, { method, headers: upstreamHeaders, body, redirect: "follow", signal: req.signal });
  } catch (e) {
    if (req.signal.aborted) return fail("Request cancelled.");
    return fail(describeError(e));
  }

  const headerList: [string, string][] = [];
  const wasCompressed = res.headers.has("content-encoding");
  for (const [k, v] of res.headers.entries()) {
    const lk = k.toLowerCase();
    if (lk === "set-cookie") continue;
    // fetch already decompressed the body — these wire headers would contradict it
    if (wasCompressed && (lk === "content-encoding" || lk === "content-length")) continue;
    headerList.push([k, v]);
  }
  for (const sc of res.headers.getSetCookie()) headerList.push(["set-cookie", sc]);

  const chunks: Uint8Array[] = [];
  let received = 0;
  try {
    const reader = res.body?.getReader();
    if (reader) {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        received += value.byteLength;
        if (received > MAX_RESPONSE_BYTES) {
          try { await reader.cancel(); } catch { /* already closed */ }
          return fail("Response exceeded 256 MB — too large for the viewer.");
        }
        chunks.push(value);
      }
    }
  } catch (e) {
    // mid-body failure: connection reset, corrupt encoding, or a client abort
    // that errors the upstream stream after fetch() already resolved
    if (req.signal.aborted) return fail("Request cancelled.");
    return fail(describeError(e));
  }
  const bodyBytes = new Uint8Array(received);
  let off = 0;
  for (const c of chunks) { bodyBytes.set(c, off); off += c.byteLength; }

  const respHeaders: Record<string, string> = {
    "x-whisper-meta": metaHeader({
      status: res.status,
      statusText: res.statusText,
      url: res.url,
      redirected: res.redirected,
      headers: headerList,
    }),
    "cache-control": "no-store",
  };
  const ct = res.headers.get("content-type");
  if (ct) respHeaders["content-type"] = ct;
  return new Response(bodyBytes, { status: 200, headers: respHeaders });
}

function handler(req: Request): Response | Promise<Response> {
  if (!hostOk(req)) return new Response("Forbidden", { status: 403 });
  const url = new URL(req.url);
  if (req.method === "GET" && (url.pathname === "/" || url.pathname === "/index.html")) {
    return new Response(html, {
      headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" },
    });
  }
  if (req.method === "GET" && url.pathname === "/whisper-proxy/info") {
    return new Response(JSON.stringify({ whisper: true, version: VERSION, token }), {
      headers: { "content-type": "application/json", "cache-control": "no-store" },
    });
  }
  if (req.method === "POST" && url.pathname === "/whisper-proxy") {
    if (req.headers.get("x-whisper-token") !== token) return new Response("Forbidden", { status: 403 });
    return proxy(req);
  }
  return new Response("Not found", { status: 404 });
}

let server: Deno.HttpServer | null = null;
for (let attempt = 0; attempt < 25; attempt++) {
  try {
    server = Deno.serve({
      hostname: "127.0.0.1",
      port,
      onListen: () => {},
      // last-resort net: an unexpected handler crash answers in the meta
      // protocol instead of printing a stack trace and a bare 500
      onError: (e) => new Response(null, {
        status: 200,
        headers: { "x-whisper-meta": metaHeader({ error: "Whisper engine error: " + describeError(e) }), "cache-control": "no-store" },
      }),
    }, handler);
    break;
  } catch (e) {
    if (e instanceof Deno.errors.AddrInUse) { port++; continue; }
    throw e;
  }
}
if (!server) {
  console.error("Could not find a free port near 7788 — is Whisper already running?");
  Deno.exit(1);
}

const appUrl = `http://127.0.0.1:${port}/`;
console.log("");
console.log("  ┌──────────────────────────────────────────────┐");
console.log("  │  Whisper — REST client                         │");
console.log(`  │  Running at ${appUrl.padEnd(33)}│`);
console.log("  │                                              │");
console.log("  │  Keep this window open while you use it.     │");
console.log("  │  Close it (or press Ctrl+C) to quit.         │");
console.log("  └──────────────────────────────────────────────┘");
console.log("");

try {
  const os = Deno.build.os;
  const cmd = os === "windows"
    ? new Deno.Command("cmd", { args: ["/c", "start", "", appUrl], stdout: "null", stderr: "null" })
    : os === "darwin"
      ? new Deno.Command("open", { args: [appUrl], stdout: "null", stderr: "null" })
      : new Deno.Command("xdg-open", { args: [appUrl], stdout: "null", stderr: "null" });
  cmd.spawn();
} catch {
  console.log("  Open the address above in your browser to start.");
}

await server.finished;
