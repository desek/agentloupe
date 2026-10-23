#!/usr/bin/env python3
# @agents-index Local probe of the start request: a fake telemetry endpoint that records each request, and a start delay measurement of loupe serve.
#
# Usage: telemetry.probe.py fake --log FILE [--port N] [--mode answer|hold|redirect] [--status CODE] [--human]
#        telemetry.probe.py startdelay --bin PATH [--runs N] [--env NAME=VALUE ...] [--human]
#   fake        Run a fake endpoint on 127.0.0.1 until SIGTERM or SIGINT. Each request becomes one JSON
#               line in FILE with the method, the path, the query, the header names and values, and the
#               body size. --mode answer gives --status (default 204) with no body; hold reads the
#               request and never answers; redirect answers 302 to /redirected on the same host.
#   --port N    The port of the fake endpoint. Default 0, a free port that the kernel picks.
#   startdelay  Start BIN serve --runs times (default 5) with two keys from BIN gen-key, a new data
#               directory, and a free port, and measure the time from the process start to the first
#               200 of GET /healthz. Each run stops the server with SIGTERM.
#   --env       One more environment value for each start, for example a run-time setting. Repeatable.
#   --human     Print a friendly view for a person. Also the default when stdout is a terminal.
#   -h, --help  Print this help.
#
# Purpose: the scenario of the start request (docs/specification/telemetry.md) needs a local receiver
# and a start delay figure; this script gives both with the Python standard library only. It never
# prints a key and never stores a request body.
#
# Output (stdout, key: value):
#   fake:       "port: <n> mode: <mode> log: <file>" once the endpoint listens.
#   startdelay: one "run: <i> ms: <n>" line per start, then "median_ms: <n> runs: <n>".
# Exit codes: 0 done, 1 a start failed or the endpoint could not bind, 2 bad usage.
# Reference: docs/specification/telemetry.md, docs/specification/operations.md (Start telemetry targets).
import argparse
import http.server
import json
import os
import signal
import socket
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


def fail(msg, code):
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(code)


def fake(args, human):
    lock = threading.Lock()
    mode, status, log = args.mode, args.status, args.log

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *a):  # the default access log goes to stderr; the JSON file is the record
            pass

        def record(self):
            size = int(self.headers.get("content-length") or 0)
            body = self.rfile.read(size) if size else b""
            path, _, query = self.path.partition("?")
            line = {
                "method": self.command,
                "path": path,
                "query": query,
                "headers": {k.lower(): v for k, v in self.headers.items()},
                "body_bytes": len(body),
                "time": time.time(),
            }
            with lock, open(log, "a") as f:
                f.write(json.dumps(line) + "\n")

        def answer(self):
            self.record()
            if mode == "hold":
                time.sleep(3600)
                return
            if mode == "redirect":
                self.send_response(302)
                self.send_header("location", f"http://{self.headers.get('host')}/redirected")
            else:
                self.send_response(status)
            self.send_header("content-length", "0")
            self.end_headers()

        do_GET = do_POST = do_HEAD = do_PUT = do_DELETE = answer

    class Server(http.server.ThreadingHTTPServer):
        daemon_threads = True

    try:
        srv = Server(("127.0.0.1", args.port), Handler)
    except OSError as e:
        fail(f"the fake endpoint could not bind 127.0.0.1:{args.port} ({e.strerror}). Fix: pass a free --port or 0. Verify: the script prints a port line.", 1)
    open(log, "a").close()
    port = srv.server_address[1]
    print(f"Fake endpoint on 127.0.0.1:{port}, mode {mode}, log {log}." if human else f"port: {port} mode: {mode} log: {log}", flush=True)
    signal.signal(signal.SIGTERM, lambda *_: threading.Thread(target=srv.shutdown).start())
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def startdelay(args, human):
    if not os.access(args.bin, os.X_OK):
        fail(f"{args.bin} is not an executable file. Fix: pass --bin with the path of a built loupe, for example target/debug/loupe. Verify: <bin> --version prints a version.", 2)
    extra = {}
    for kv in args.env or []:
        name, sep, value = kv.partition("=")
        if not sep or not name:
            fail(f"--env {name} has no NAME=VALUE form. Fix: pass --env NAME=VALUE. Verify: re-run.", 2)
        extra[name] = value
    keys = [subprocess.run([args.bin, "gen-key"], capture_output=True, text=True, check=True).stdout.strip() for _ in range(2)]
    times = []
    for i in range(1, args.runs + 1):
        port = free_port()
        with tempfile.TemporaryDirectory() as data:
            env = {"PATH": os.environ.get("PATH", ""), "LOUPE_LISTEN": f"127.0.0.1:{port}", "LOUPE_DATA_DIR": data,
                   "LOUPE_DATA_KEY": keys[0], "LOUPE_ACCOUNT_KEY": keys[1], **extra}
            t0 = time.monotonic()
            p = subprocess.Popen([args.bin, "serve"], cwd=ROOT, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            ms = None
            while time.monotonic() - t0 < 30:
                if p.poll() is not None:
                    break
                try:
                    with urllib.request.urlopen(f"http://127.0.0.1:{port}/healthz", timeout=1) as r:
                        if r.status == 200:
                            ms = (time.monotonic() - t0) * 1000
                            break
                except OSError:
                    time.sleep(0.002)
            p.send_signal(signal.SIGTERM)
            try:
                p.wait(timeout=15)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait()
        if ms is None:
            fail(f"start {i} gave no 200 of GET /healthz within 30 seconds. Fix: run {args.bin} serve by hand and read its log. Verify: re-run.", 1)
        times.append(ms)
        print(f"Start {i}: {ms:.1f} ms." if human else f"run: {i} ms: {ms:.1f}", flush=True)
    med = statistics.median(times)
    print(f"Median {med:.1f} ms over {len(times)} starts." if human else f"median_ms: {med:.1f} runs: {len(times)}")


def main():
    p = argparse.ArgumentParser(add_help=False)
    p.add_argument("command", nargs="?")
    p.add_argument("--log")
    p.add_argument("--port", type=int, default=0)
    p.add_argument("--mode", choices=["answer", "hold", "redirect"], default="answer")
    p.add_argument("--status", type=int, default=204)
    p.add_argument("--bin")
    p.add_argument("--runs", type=int, default=5)
    p.add_argument("--env", action="append")
    p.add_argument("--human", action="store_true")
    p.add_argument("-h", "--help", action="store_true")
    args, rest = p.parse_known_args()
    if args.help:
        with open(__file__) as f:
            for n, line in enumerate(f):
                if n >= 3 and line.startswith("#"):
                    print(line[2:].rstrip())
                elif n >= 3:
                    break
        return
    if rest:
        fail(f"unknown argument {rest[0]}. Fix: run --help for the flags. Verify: re-run.", 2)
    human = args.human or sys.stdout.isatty()
    if args.command == "fake":
        if not args.log:
            fail("fake needs --log FILE. Fix: pass --log with a path for the request lines. Verify: the script prints a port line.", 2)
        fake(args, human)
    elif args.command == "startdelay":
        if not args.bin:
            fail("startdelay needs --bin PATH. Fix: pass the path of a built loupe. Verify: the script prints run lines.", 2)
        startdelay(args, human)
    else:
        fail("a command is required: fake or startdelay. Fix: run --help. Verify: re-run with a command.", 2)


if __name__ == "__main__":
    main()
