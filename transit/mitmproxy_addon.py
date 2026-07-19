"""mitmproxy addon variant of transit (alternative to proxy.py).

Install & run:
    pip install mitmproxy
    mitmdump --mode reverse:https://api.moonshot.ai@8787 -s mitmproxy_addon.py

mitmproxy buffers streamed responses by default, so the response hook sees
the full SSE body. Point the CLI at http://127.0.0.1:8787/v1 as usual.
"""

import json

from mitmproxy import http


def extract_usage(text, content_type):
    def from_obj(obj):
        if not isinstance(obj, dict):
            return None
        if isinstance(obj.get("usage"), dict):
            return obj["usage"]
        resp = obj.get("response")
        if isinstance(resp, dict) and isinstance(resp.get("usage"), dict):
            return resp["usage"]
        if obj.get("type") == "message_start":
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
                usage = {**(usage or {}), **found}
        return usage

    try:
        return from_obj(json.loads(text))
    except ValueError:
        return None


WATCH_HOSTS = ("api.kimi.com", "api.anthropic.com", "api.openai.com", "api.moonshot.ai", "api.moonshot.cn")


def response(flow: http.HTTPFlow) -> None:
    if not any(h in flow.request.pretty_host for h in WATCH_HOSTS):
        return
    content_type = flow.response.headers.get("content-type", "")
    usage = extract_usage(flow.response.get_text() or "", content_type)
    model = None
    try:
        model = json.loads(flow.request.get_text() or "{}").get("model")
    except ValueError:
        pass
    record = {
        "method": flow.request.method,
        "host": flow.request.pretty_host,
        "path": flow.request.path,
        "status": flow.response.status_code,
        "model": model,
        "usage": usage,
    }
    print(f"[usage] {json.dumps(record, ensure_ascii=False)}", flush=True)
