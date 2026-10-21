#!/usr/bin/env python3
"""@agents-index Runs the session days scenario: loupe backfill of a synthetic three-day session against a fake summary upstream.

Usage: python3 -I .agents/scripts/summary.session_days.scenario.py [--binary PATH]
  --binary  The loupe binary. Default target/release/loupe.

The script writes a Claude Code log of one session with calls on 2026-03-02, 03, and 04 (UTC)
with a marker word in each prompt, starts a fake Anthropic Messages upstream on a free port, runs
`loupe backfill` and `loupe summarize --day` for each day, then prints key=value lines:
full_done, day_done, day_lines (session_lines per day), marker_in_db, status. Exit 0 when the
success condition holds, 1 when it does not, 2 on bad usage. Scratch files go to a temp dir that
the script removes; no server stays up after exit.
"""
import argparse, http.server, json, os, pathlib, sqlite3, subprocess, sys, tempfile, threading

ROOT = pathlib.Path(__file__).resolve().parents[2]
KEY = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8="
MARK = "QZMARKERSCN"
DAYS = ["2026-03-02", "2026-03-03", "2026-03-04"]
TEXT = "The session worked on the parser module. It added tests for the edge cases. The tests passed."


class Fake(http.server.BaseHTTPRequestHandler):
    """Answers every POST with one fixed scrubbed paragraph in the Messages format."""

    def do_POST(self):
        self.rfile.read(int(self.headers.get("content-length", 0)))
        body = json.dumps({"id": "msg_f", "type": "message", "role": "assistant", "model": "claude-haiku-5-5",
                           "content": [{"type": "text", "text": TEXT}], "stop_reason": "end_turn",
                           "usage": {"input_tokens": 10, "output_tokens": 20}}).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *a):
        pass


def log_lines():
    """One user turn and one assistant call per day, with a conversation above the threshold."""
    out = []
    for i, d in enumerate(DAYS):
        prompt = f"please improve the {MARK} parser module " + "and explain the change in detail " * 12
        out.append({"type": "user", "sessionId": "scn-sess", "uuid": f"u{i}", "timestamp": f"{d}T10:00:00.000Z",
                    "cwd": "/tmp/scn", "message": {"role": "user", "content": prompt}})
        out.append({"type": "assistant", "sessionId": "scn-sess", "uuid": f"a{i}", "timestamp": f"{d}T10:00:05.000Z",
                    "message": {"id": f"msg_scn_{i}", "role": "assistant", "model": "claude-haiku-5-5",
                                "content": [{"type": "text", "text": "I changed the parser module and the tests " * 10}],
                                "usage": {"input_tokens": 2, "output_tokens": 3}}})
    return "\n".join(json.dumps(o) for o in out) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", default=str(ROOT / "target/release/loupe"))
    a = ap.parse_args()
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Fake)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    with tempfile.TemporaryDirectory() as tmp:
        t = pathlib.Path(tmp)
        logs = t / "logs/proj"
        logs.mkdir(parents=True)
        (logs / "scn-sess.jsonl").write_text(log_lines())
        data = t / "data"
        data.mkdir()
        env = {"PATH": os.environ["PATH"], "HOME": tmp, "LOUPE_DATA_DIR": str(data), "LOUPE_DATA_KEY": KEY,
               "LOUPE_ACCOUNT_KEY": KEY, "LOUPE_TZ": "UTC", "LOUPE_SUMMARY_API_KEY": "sk-test-scn",
               "LOUPE_SUMMARY_BASE_URL": f"http://127.0.0.1:{srv.server_port}"}
        run = lambda *args: subprocess.run([a.binary, *args], env=env, cwd=ROOT, capture_output=True, text=True)
        bf = run("backfill", "--source", "claude-code", "--path", str(t / "logs"))
        if bf.returncode != 0:
            print(f"status=failed step=backfill exit={bf.returncode}", file=sys.stderr)
            srv.shutdown()
            return 1
        lines = []
        for d in DAYS:
            r = run("summarize", "--day", d)
            lines.append(next((l.split(":", 1)[1].strip() for l in r.stdout.splitlines() if l.startswith("session_lines")), "none"))
        db = sqlite3.connect(data / "loupe.db")
        q = lambda s: db.execute(s).fetchone()[0]
        full = q("SELECT count(*) FROM session_full_summaries WHERE status='done'")
        day = q("SELECT count(*) FROM session_day_summaries WHERE status='done'")
        db.close()
        dump = b"".join(p.read_bytes() for p in data.rglob("*") if p.is_file())
        marker = int(MARK.encode() in dump or MARK.encode() in (bf.stdout + bf.stderr).encode())
        ok = full == 3 and day == 3 and lines == ["1", "1", "1"] and marker == 0
        print(f"full_done={full} day_done={day} day_lines={','.join(lines)} marker_in_db={marker} status={'passed' if ok else 'failed'}")
    srv.shutdown()
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
