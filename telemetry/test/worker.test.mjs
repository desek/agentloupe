// @agents-index Worker test of the telemetry endpoint under wrangler dev with a local D1 database: insert, increment, and rejection.
//
// Run: node --test telemetry/test/worker.test.mjs (telemetry.deploy.sh --test runs it with the pinned Node and wrangler).
// The command of wrangler comes from WRANGLER (default "wrangler"), split on spaces, for example
// "mise exec -- wrangler". It needs no Cloudflare account, no login, and no network to the account: the
// database lives in a temporary directory that the test removes.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { spawn, execFile } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const DIR = dirname(dirname(fileURLToPath(import.meta.url)));
const CONFIG = join(DIR, "wrangler.toml");
const DB = "agentloupe-telemetry";
const [CMD, ...PRE] = (process.env.WRANGLER || "wrangler").split(" ").filter(Boolean);
const ENV = { ...process.env, WRANGLER_SEND_METRICS: "false", NO_COLOR: "1", CI: "1" };
const SHA = "0123456789abcdef0123456789abcdef01234567";
const today = () => new Date().toISOString().slice(0, 10);

let persist;
let dev;
let base;

function wrangler(args) {
  return new Promise((resolve, reject) => {
    execFile(CMD, [...PRE, ...args], { cwd: DIR, env: ENV, maxBuffer: 1 << 24 }, (err, stdout, stderr) =>
      err ? reject(new Error(`wrangler ${args[0]} ${args[1] ?? ""} failed: ${stderr || stdout}`)) : resolve(stdout),
    );
  });
}

/** Every row of the table starts in the local database, read with wrangler d1 execute. */
async function rows() {
  const out = await wrangler([
    "d1", "execute", DB, "--local", "--persist-to", persist, "--config", CONFIG, "--json",
    "--command", "SELECT sha, firstseen, lastseen, count FROM starts ORDER BY sha",
  ]);
  return JSON.parse(out)[0].results;
}

function freePort() {
  return new Promise((resolve) => {
    const s = createServer();
    s.listen(0, "127.0.0.1", () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });
}

async function get(path, method = "GET") {
  const res = await fetch(base + path, { method, redirect: "manual" });
  const body = method === "HEAD" ? "" : await res.text();
  return { status: res.status, body };
}

before(async () => {
  persist = await mkdtemp(join(tmpdir(), "agentloupe-telemetry-"));
  await wrangler(["d1", "migrations", "apply", DB, "--local", "--persist-to", persist, "--config", CONFIG]);
  const port = await freePort();
  base = `http://127.0.0.1:${port}`;
  dev = spawn(CMD, [...PRE, "dev", "--local", "--ip", "127.0.0.1", "--port", String(port),
    "--persist-to", persist, "--config", CONFIG, "--show-interactive-dev-session=false"],
  { cwd: DIR, env: ENV, stdio: "ignore", detached: true });
  // A poll with a cap: the Worker answers 404 on / once it is up.
  const deadline = Date.now() + 60_000;
  for (;;) {
    try {
      if ((await fetch(base + "/")).status === 404) break;
    } catch {}
    if (Date.now() > deadline) throw new Error("wrangler dev did not answer within 60 seconds");
    await new Promise((r) => setTimeout(r, 250));
  }
});

after(async () => {
  if (dev) {
    try { process.kill(-dev.pid, "SIGTERM"); } catch {}
  }
  if (persist) await rm(persist, { recursive: true, force: true });
});

test("insert", async () => {
  assert.deepEqual(await rows(), []);
  const r = await get(`/${SHA}`);
  assert.equal(r.status, 204);
  assert.equal(r.body, "");
  assert.deepEqual(await rows(), [{ sha: SHA, firstseen: today(), lastseen: today(), count: 1 }]);
});

test("increment", async () => {
  const [first] = await rows();
  const r = await get(`/${SHA}`);
  assert.equal(r.status, 204);
  assert.equal(r.body, "");
  const all = await rows();
  assert.equal(all.length, 1);
  assert.deepEqual(all[0], { sha: SHA, firstseen: first.firstseen, lastseen: today(), count: 2 });
});

test("rejection", async () => {
  const before = await rows();
  const cases = [
    ["GET", "/"],
    ["GET", `/${SHA.slice(1)}`],
    ["GET", `/${SHA}0`],
    ["GET", `/${SHA.toUpperCase()}`],
    ["GET", `/${SHA}/x`],
    ["GET", `/${SHA}/`],
    ["HEAD", `/${SHA}`],
    ["POST", `/${SHA}`],
  ];
  for (const [method, path] of cases) {
    const r = await get(path, method);
    assert.equal(r.status, 404, `${method} ${path}`);
  }
  assert.deepEqual(await rows(), before);
  // A query on an accepted path counts, and the store holds no query.
  const other = "fedcba9876543210fedcba9876543210fedcba98";
  assert.equal((await get(`/${other}?a=b`)).status, 204);
  const all = await rows();
  assert.equal(all.length, 2);
  assert.deepEqual(all.find((x) => x.sha === other), { sha: other, firstseen: today(), lastseen: today(), count: 1 });
  assert.ok(!JSON.stringify(all).includes("a=b"));
});
