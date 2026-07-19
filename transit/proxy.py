#!/usr/bin/env python3
"""transit: transparent reverse proxy that reports LLM token usage.

Requests are forwarded verbatim (method, path, headers, body) -- the proxy
never touches auth. It only *reads* the response, extracts the `usage`
object (OpenAI chat / OpenAI Responses / Anthropic messages, streaming and
non-streaming), and logs one JSON line per request. If USAGE_WEBHOOK_URL is
set, each record is also POSTed there.

Run:
    UPSTREAM_URL=https://api.kimi.com/coding/v1 \
    PORT=8787 \
    USAGE_WEBHOOK_URL=http://127.0.0.1:9000/usage \
    python3 proxy.py

Then point the CLI at the proxy root, e.g.:
    KIMI_BASE_URL=http://127.0.0.1:8787

(UPSTREAM_URL may carry a path prefix; it is prepended to every request
path, so /chat/completions -> /coding/v1/chat/completions.)

Only Python stdlib is required.
"""

import http.client
import json
import os
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

UPSTREAM = urlsplit(os.environ.get("UPSTREAM_URL", "https://api.kimi.com/coding/v1"))
LISTEN_HOST = os.environ.get("LISTEN_HOST", "127.0.0.1")
LISTEN_PORT = int(os.environ.get("PORT", "8787"))
WEBHOOK_URL = os.environ.get("USAGE_WEBHOOK_URL")

HOP_BY_HOP = frozenset(
    {
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailers",
        "transfer-encoding",
        "upgrade",
        # stripped on purpose:
        "host",  # rewritten for the upstream
        "content-length",  # recomputed by http.client
        "accept-encoding",  # force identity so the body stays parseable
    }
)

# Response headers to strip. Content-Length must survive: the body is relayed
# byte-identical, and the client needs it for framing. Server/Date are dropped
# because send_response() already adds its own.
RESPONSE_SKIP = (HOP_BY_HOP - {"content-length"}) | {"server", "date"}


def extract_usage(body, content_type):
    """Pull the token-usage object out of a response body. Returns dict or None."""
    text = body.decode("utf-8", "replace")

    def from_obj(obj):
        if not isinstance(obj, dict):
            return None
        if isinstance(obj.get("usage"), dict):
            return obj["usage"]  # OpenAI chat / Anthropic non-stream
        resp = obj.get("response")
        if isinstance(resp, dict) and isinstance(resp.get("usage"), dict):
            return resp["usage"]  # OpenAI Responses API events
        if obj.get("type") == "message_start":  # Anthropic stream: input tokens
            msg = obj.get("message")
            if isinstance(msg, dict) and isinstance(msg.get("usage"), dict):
                return msg["usage"]
        return None

    if "text/event-stream" in content_type:
        usage = None
        for line in text.splitlines():
            if not line.startswith("data:"):
                continue
            payload = line[5:].strip()
            if not payload or payload == "[DONE]":
                continue
            try:
                obj = json.loads(payload)
            except ValueError:
                continue
            found = from_obj(obj)
            if found:
                # Anthropic: message_start carries input_tokens, message_delta
                # carries output_tokens -- merge them into one record.
                usage = {**(usage or {}), **found}
        return usage

    try:
        return from_obj(json.loads(text))
    except ValueError:
        return None


def extract_model(body):
    try:
        obj = json.loads(body)
        if isinstance(obj, dict):
            return obj.get("model")
    except ValueError:
        pass
    return None


def _post_webhook(line):
    try:
        u = urlsplit(WEBHOOK_URL)
        conn_cls = http.client.HTTPSConnection if u.scheme == "https" else http.client.HTTPConnection
        conn = conn_cls(u.hostname, u.port, timeout=5)
        path = (u.path or "/") + (("?" + u.query) if u.query else "")
        conn.request("POST", path, body=line.encode(), headers={"Content-Type": "application/json"})
        conn.getresponse().read()
        conn.close()
    except OSError as e:
        print(f"[proxy] webhook failed: {e}", flush=True)


def report(record):
    line = json.dumps(record, ensure_ascii=False)
    print(f"[usage] {line}", flush=True)
    if WEBHOOK_URL:
        threading.Thread(target=_post_webhook, args=(line,), daemon=True).start()


class ProxyHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    timeout = 600

    def log_message(self, fmt, *args):
        pass  # quiet; structured [usage] lines are the output

    def _relay(self):
        try:
            self._do_relay()
        except (BrokenPipeError, ConnectionResetError):
            pass
        except Exception as e:  # noqa: BLE001 -- never take the client down with us
            print(f"[proxy] error: {e!r}", flush=True)
            try:
                self.send_error(502, f"proxy error: {e}")
            except OSError:
                pass

    def _do_relay(self):
        length = self.headers.get("Content-Length")
        req_body = self.rfile.read(int(length)) if length else None

        path = UPSTREAM.path.rstrip("/") + self.path
        conn_cls = http.client.HTTPSConnection if UPSTREAM.scheme == "https" else http.client.HTTPConnection
        conn = conn_cls(UPSTREAM.hostname, UPSTREAM.port, timeout=600)

        headers = {k: v for k, v in self.headers.items() if k.lower() not in HOP_BY_HOP}
        headers["Host"] = UPSTREAM.hostname
        conn.request(self.command, path, body=req_body, headers=headers)

        resp = conn.getresponse()
        resp_content_type = resp.getheader("Content-Type", "")
        has_content_length = resp.getheader("Content-Length") is not None

        self.send_response(resp.status)
        for k, v in resp.getheaders():
            if k.lower() in RESPONSE_SKIP:
                continue
            self.send_header(k, v)
        if not has_content_length:
            # No framing to offer the client (typical for SSE): close to delimit.
            self.send_header("Connection", "close")
            self.close_connection = True
        self.end_headers()

        collected = bytearray()
        while True:
            chunk = resp.read1(65536)  # read1 returns as soon as data arrives (SSE-friendly)
            if not chunk:
                break
            if self.command != "HEAD":
                self.wfile.write(chunk)
                self.wfile.flush()
            collected.extend(chunk)
        conn.close()

        report(
            {
                "method": self.command,
                "path": self.path,
                "status": resp.status,
                "model": extract_model(req_body) if req_body else None,
                "usage": extract_usage(bytes(collected), resp_content_type),
                "response_bytes": len(collected),
            }
        )

    do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = do_HEAD = do_OPTIONS = _relay


def main():
    server = ThreadingHTTPServer((LISTEN_HOST, LISTEN_PORT), ProxyHandler)
    server.daemon_threads = True
    print(f"[proxy] listening on http://{LISTEN_HOST}:{LISTEN_PORT} -> {UPSTREAM.geturl()}", flush=True)
    if WEBHOOK_URL:
        print(f"[proxy] posting usage records to {WEBHOOK_URL}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
