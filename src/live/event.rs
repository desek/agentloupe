//! @agents-index The live events, the envelope, and the serializers that write only the public allowlist of each kind.

use rust_decimal::Decimal;
use serde_json::{Value, json};

/// The version of the envelope; a removed or renamed field increments it.
pub const VERSION: u64 = 1;

/// The public allowlist: for each kind, the only keys of `data`. A test compares each frame to it.
pub const ALLOWLIST: &[(&str, &[&str])] = &[
    ("exchange.start", &["ts"]),
    ("exchange.end", &["ts", "status"]),
    (
        "call.stored",
        &[
            "ts",
            "input",
            "output",
            "cache_read",
            "cache_write",
            "reasoning",
            "cost_usd",
        ],
    ),
    ("segments.open", &["ts", "count"]),
    ("heartbeat", &["ts"]),
    (
        "snapshot",
        &[
            "date",
            "calls",
            "tokens",
            "cost_usd",
            "segments_open",
            "in_flight",
            "live_enabled",
        ],
    ),
    (
        "calls.batch",
        &[
            "ts",
            "calls",
            "input",
            "output",
            "cache_read",
            "cache_write",
            "reasoning",
            "cost_usd",
        ],
    ),
    ("exchanges.batch", &["ts", "started", "ended", "in_flight"]),
];

/// The five token counts of a call or of a sum of calls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub reasoning: i64,
}

impl Tokens {
    pub fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
        self.reasoning += o.reasoning;
    }

    fn fields(&self, m: &mut serde_json::Map<String, Value>) {
        m.insert("input".into(), self.input.into());
        m.insert("output".into(), self.output.into());
        m.insert("cache_read".into(), self.cache_read.into());
        m.insert("cache_write".into(), self.cache_write.into());
        m.insert("reasoning".into(), self.reasoning.into());
    }

    fn object(&self) -> Value {
        let mut m = serde_json::Map::new();
        self.fields(&mut m);
        Value::Object(m)
    }
}

/// One event that a sender puts on the hub. `ts` is Unix milliseconds. The event holds no
/// agent, path, body, or key, so nothing private can reach a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    ExchangeStart {
        ts: i64,
    },
    ExchangeEnd {
        ts: i64,
        status: u16,
    },
    CallStored {
        ts: i64,
        tokens: Tokens,
        cost_usd: Option<Decimal>,
    },
    SegmentsOpen {
        ts: i64,
        count: i64,
    },
}

/// The totals of the current day that the first frame of a socket carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub date: String,
    pub calls: i64,
    pub tokens: Tokens,
    pub cost_usd: Decimal,
    pub segments_open: i64,
    pub in_flight: i64,
}

/// The sum of the `call.stored` events above the rate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CallsBatch {
    pub calls: i64,
    pub tokens: Tokens,
    /// None until one priced call is in the sum.
    pub cost_usd: Option<Decimal>,
}

/// The exchange events above the rate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExchangesBatch {
    pub started: i64,
    pub ended: i64,
    pub in_flight: i64,
}

/// One JSON text frame: `{"v":1,"kind":...,"data":...}`.
pub fn envelope(kind: &str, data: Value) -> String {
    json!({"v": VERSION, "kind": kind, "data": data}).to_string()
}

/// The cost as the public API writes a decimal, or null.
fn cost(c: Option<Decimal>) -> Value {
    c.map_or(Value::Null, |d| json!(d))
}

impl Event {
    pub fn kind(&self) -> &'static str {
        match self {
            Event::ExchangeStart { .. } => "exchange.start",
            Event::ExchangeEnd { .. } => "exchange.end",
            Event::CallStored { .. } => "call.stored",
            Event::SegmentsOpen { .. } => "segments.open",
        }
    }

    /// The public frame of the event.
    pub fn frame(&self) -> String {
        let data = match self {
            Event::ExchangeStart { ts } => json!({"ts": ts}),
            Event::ExchangeEnd { ts, status } => json!({"ts": ts, "status": status}),
            Event::CallStored {
                ts,
                tokens,
                cost_usd,
            } => {
                let mut m = serde_json::Map::new();
                m.insert("ts".into(), (*ts).into());
                tokens.fields(&mut m);
                m.insert("cost_usd".into(), cost(*cost_usd));
                Value::Object(m)
            }
            Event::SegmentsOpen { ts, count } => json!({"ts": ts, "count": count}),
        };
        envelope(self.kind(), data)
    }
}

pub fn heartbeat(ts: i64) -> String {
    envelope("heartbeat", json!({"ts": ts}))
}

impl Snapshot {
    pub fn frame(&self) -> String {
        envelope(
            "snapshot",
            json!({
                "date": self.date, "calls": self.calls, "tokens": self.tokens.object(),
                "cost_usd": json!(self.cost_usd), "segments_open": self.segments_open,
                "in_flight": self.in_flight, "live_enabled": true,
            }),
        )
    }
}

impl CallsBatch {
    pub fn add(&mut self, tokens: &Tokens, c: Option<Decimal>) {
        self.calls += 1;
        self.tokens.add(tokens);
        if let Some(c) = c {
            self.cost_usd = Some(self.cost_usd.unwrap_or_default() + c);
        }
    }

    pub fn frame(&self, ts: i64) -> String {
        let mut m = serde_json::Map::new();
        m.insert("ts".into(), ts.into());
        m.insert("calls".into(), self.calls.into());
        self.tokens.fields(&mut m);
        m.insert("cost_usd".into(), cost(self.cost_usd));
        envelope("calls.batch", Value::Object(m))
    }
}

impl ExchangesBatch {
    pub fn frame(&self, ts: i64) -> String {
        envelope(
            "exchanges.batch",
            json!({"ts": ts, "started": self.started, "ended": self.ended, "in_flight": self.in_flight}),
        )
    }
}
