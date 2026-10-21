#!/usr/bin/env python3
# @agents-index Forwards summary requests to the Anthropic API and records the kind and character count of each request, never its text.
#
# Usage: summary.recorder.py PORT OUT_TSV [UPSTREAM]
#   PORT       Loopback port to listen on.
#   OUT_TSV    File that gets one line per request: kind, request characters, HTTP status.
#   UPSTREAM   Base URL to forward to. Default https://api.anthropic.com.
# The kind is full, day, line, or other, read from the system prompt. The request characters are the
# characters of the system text plus the user message. No text, header, or credential is written.
import http.server
import json
import sys
import urllib.error
import urllib.request

PORT, OUT = int(sys.argv[1]), sys.argv[2]
UPSTREAM = sys.argv[3] if len(sys.argv) > 3 else "https://api.anthropic.com"


def kind(system: str) -> str:
    if "from its start up to the end of one day" in system:
        return "full"
    if "what one coding-agent session did on one day" in system:
        return "day"
    if "one coding-agent session for a public activity log" in system:
        return "line"
    return "other"


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("content-length", 0)))
        try:
            req = json.loads(body)
            system = " ".join(b.get("text", "") for b in req.get("system", []) if "billing-header" not in b.get("text", ""))
            user = "".join(m.get("content", "") for m in req.get("messages", []) if isinstance(m.get("content"), str))
            k, chars = kind(system), len(system) + len(user)
        except Exception:
            k, chars = "other", len(body)
        headers = {h: v for h, v in self.headers.items() if h.lower() not in ("host", "content-length", "accept-encoding")}
        r = urllib.request.Request(UPSTREAM + self.path, data=body, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(r, timeout=60) as resp:
                status, data, rh = resp.status, resp.read(), resp.headers
        except urllib.error.HTTPError as e:
            status, data, rh = e.code, e.read(), e.headers
        except Exception:
            status, data, rh = 502, b'{"type":"error"}', {}
        with open(OUT, "a") as f:
            f.write(f"{k}\t{chars}\t{status}\n")
        self.send_response(status)
        self.send_header("content-type", "application/json")
        if rh and rh.get("retry-after"):
            self.send_header("retry-after", rh.get("retry-after"))
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
