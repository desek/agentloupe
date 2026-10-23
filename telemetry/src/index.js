// @agents-index The Worker of the start request: GET /<40 lowercase hex> is one atomic upsert and 204; every other request is 404 with no write.
//
// It stores only the commit, two UTC dates, and a count. It reads no header, stores no query, logs
// nothing, and has no read route. docs/specification/telemetry.md (Telemetry endpoint) states the rules.

const SHA = /^\/([0-9a-f]{40})$/;

// One statement: a new commit gets a row with count 1; a known commit gets today as lastseen and count + 1,
// and keeps firstseen.
const UPSERT =
  "INSERT INTO starts (sha, firstseen, lastseen, count) VALUES (?1, ?2, ?2, 1) " +
  "ON CONFLICT(sha) DO UPDATE SET lastseen = excluded.lastseen, count = starts.count + 1";

export default {
  async fetch(request, env) {
    const match = request.method === "GET" ? SHA.exec(new URL(request.url).pathname) : null;
    if (!match) {
      return new Response(null, { status: 404 });
    }
    const today = new Date().toISOString().slice(0, 10);
    try {
      await env.DB.prepare(UPSERT).bind(match[1], today).run();
    } catch {
      return new Response(null, { status: 503 });
    }
    return new Response(null, { status: 204 });
  },
};
