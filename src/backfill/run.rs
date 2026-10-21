//! @agents-index One backfill run: reads the files, sorts the calls, adds them in one transaction per file, closes idle segments.

use super::discover::{self, LogFile};
use super::entry::Candidate;
use super::{Source, claude, pi, processed, read};
use crate::session::store::Scope;
use crate::session::{append, time};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

/// What a run needs from the configuration.
pub struct Settings {
    pub account_key: [u8; 32],
    pub idle_secs: u64,
    pub tz: chrono_tz::Tz,
    /// Upstream name to its litellm_provider, the same map that prices a proxied call.
    pub providers: BTreeMap<String, String>,
}

/// The choices of one run.
pub struct Options {
    pub source: Source,
    pub root: PathBuf,
    /// The inclusive start and exclusive end of the range, in Unix milliseconds.
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
    pub pi_label: String,
    /// Each file becomes a savepoint inside the outer transaction of a dry run, which rolls back.
    pub nested: bool,
}

/// The counters that `loupe backfill` prints, in the printed order.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Counts {
    pub files_read: u64,
    pub entries_read: u64,
    pub calls_added: u64,
    pub calls_existing: u64,
    pub calls_already_captured: u64,
    pub sessions_already_captured: u64,
    pub calls_out_of_range: u64,
    pub entries_skipped: u64,
    pub sessions_opened: u64,
    pub days_queued: u64,
    pub summaries_done: u64,
    pub summaries_skipped: u64,
    pub summaries_retry_later: u64,
    pub summaries_failed: u64,
    pub summaries_disabled: u64,
}

impl Counts {
    pub fn pairs(&self) -> Vec<(&'static str, u64)> {
        vec![
            ("files_read", self.files_read),
            ("entries_read", self.entries_read),
            ("calls_added", self.calls_added),
            ("calls_existing", self.calls_existing),
            ("calls_already_captured", self.calls_already_captured),
            ("sessions_already_captured", self.sessions_already_captured),
            ("calls_out_of_range", self.calls_out_of_range),
            ("entries_skipped", self.entries_skipped),
            ("sessions_opened", self.sessions_opened),
            ("days_queued", self.days_queued),
            ("summaries_done", self.summaries_done),
            ("summaries_skipped", self.summaries_skipped),
            ("summaries_retry_later", self.summaries_retry_later),
            ("summaries_failed", self.summaries_failed),
            ("summaries_disabled", self.summaries_disabled),
        ]
    }
}

/// The result of a run: the counters, the files, and the files of each session key for the summaries.
pub struct Outcome {
    pub counts: Counts,
    pub files: Vec<LogFile>,
    pub index: HashMap<String, Vec<usize>>,
    /// The warning line of the Pi rule, when no exchange has the Pi label but other exchanges exist.
    pub warning: Option<String>,
}

#[derive(Debug)]
pub enum Error {
    /// The root directory cannot be read.
    Dir(std::io::Error),
    Db(rusqlite::Error),
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e)
    }
}

/// Groups the calls into units: the files that share a session scope go together, so each unit is one
/// transaction and its calls run in time order. A tie keeps the file and entry order.
fn units(calls: Vec<Candidate>) -> Vec<Vec<Candidate>> {
    let mut parent: HashMap<usize, usize> = HashMap::new();
    fn root(p: &mut HashMap<usize, usize>, f: usize) -> usize {
        let up = *p.entry(f).or_insert(f);
        if up == f {
            return f;
        }
        let r = root(p, up);
        p.insert(f, r);
        r
    }
    let mut first: HashMap<&str, usize> = HashMap::new();
    for c in &calls {
        let a = root(&mut parent, c.file);
        if let Some(&f) = first.get(c.scope_key.as_str()) {
            let b = root(&mut parent, f);
            parent.insert(a.max(b), a.min(b));
        } else {
            first.insert(&c.scope_key, c.file);
        }
    }
    let mut groups: BTreeMap<usize, Vec<Candidate>> = BTreeMap::new();
    let roots: Vec<usize> = calls.iter().map(|c| root(&mut parent, c.file)).collect();
    for (c, r) in calls.into_iter().zip(roots) {
        groups.entry(r).or_default().push(c);
    }
    let mut out: Vec<Vec<Candidate>> = groups.into_values().collect();
    for u in &mut out {
        u.sort_by_key(|c| c.at_ms);
    }
    out.sort_by_key(|u| u.first().map_or(0, |c| c.at_ms));
    out
}

fn begin(conn: &Connection, nested: bool) -> rusqlite::Result<()> {
    conn.execute_batch(if nested {
        "SAVEPOINT unit"
    } else {
        "BEGIN IMMEDIATE"
    })
}

fn end(conn: &Connection, nested: bool, ok: bool) -> rusqlite::Result<()> {
    conn.execute_batch(match (nested, ok) {
        (true, true) => "RELEASE unit",
        (true, false) => "ROLLBACK TO unit; RELEASE unit",
        (false, true) => "COMMIT",
        (false, false) => "ROLLBACK",
    })
}

/// Runs `f` in one transaction that commits on success and rolls back on error.
fn atomic(
    conn: &Connection,
    nested: bool,
    f: impl FnOnce() -> rusqlite::Result<()>,
) -> rusqlite::Result<()> {
    begin(conn, nested)?;
    match f() {
        Ok(()) => end(conn, nested, true),
        Err(e) => {
            let _ = end(conn, nested, false);
            Err(e)
        }
    }
}

/// Adds one call of origin log with its segment, price, and day.
fn add(
    conn: &Connection,
    s: &Settings,
    source: Source,
    c: &Candidate,
    counts: &mut Counts,
    days: &mut HashSet<String>,
) -> rusqlite::Result<()> {
    let scope = Scope {
        agent_label: source.name().into(),
        proxy_token_id: None,
        key_hash: c.scope_key.clone(),
    };
    let seg = append::append(conn, &scope, "log", c.at_ms, s.idle_secs, None, s.tz)?;
    let u = &c.usage;
    let priced = crate::parse::cost::price(&s.providers, &c.upstream, u);
    conn.execute(
        "INSERT INTO calls (exchange_id, call_index, started_at, agent_label, upstream, proxy_token_id, session_id, account_id, model, service_tier, status, input, cache_read, cache_write_5m, cache_write_1h, output, cost_usd, price_commit, unpriced_reason, origin, log_key) \
         VALUES (NULL, 0, ?1, ?2, ?3, NULL, ?4, NULL, ?5, ?6, 200, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 'log', ?15)",
        params![
            time::text(c.at_ms), source.name(), c.upstream, seg.session_id, u.model, u.service_tier,
            u.input, u.cache_read, u.cache_write_5m, u.cache_write_1h, u.output,
            priced.cost_usd, priced.price_commit, priced.unpriced_reason, c.log_key
        ],
    )?;
    // No warn line per unpriced call: a backfill log line holds no model or upstream, and the reason is in the row.
    counts.calls_added += 1;
    counts.sessions_opened += seg.opened;
    days.insert(crate::day::boundary::day_of(c.at_ms, s.tz).to_string());
    Ok(())
}

/// Applies the calls of one unit in time order: the processed rule first, then the range, then the log key.
fn apply(
    conn: &Connection,
    s: &Settings,
    o: &Options,
    unit: &[Candidate],
    counts: &mut Counts,
    days: &mut HashSet<String>,
    skipped: &mut HashSet<String>,
) -> rusqlite::Result<()> {
    let mut local = counts.clone();
    let mut local_days = days.clone();
    atomic(conn, o.nested, || {
        for c in unit {
            if c.captured {
                local.calls_already_captured += 1;
                if skipped.insert(c.scope_key.clone()) {
                    local.sessions_already_captured += 1;
                }
                continue;
            }
            if o.from_ms.is_some_and(|f| c.at_ms < f) || o.to_ms.is_some_and(|t| c.at_ms >= t) {
                local.calls_out_of_range += 1;
                continue;
            }
            let exists: bool = conn.query_row(
                "SELECT EXISTS (SELECT 1 FROM calls WHERE log_key = ?1)",
                [&c.log_key],
                |r| r.get(0),
            )?;
            if exists {
                local.calls_existing += 1;
                continue;
            }
            add(conn, s, o.source, c, &mut local, &mut local_days)?;
        }
        Ok(())
    })?;
    *counts = local;
    *days = local_days;
    Ok(())
}

/// Closes each open backfilled segment of the source whose last call plus the idle time is before `start_ms`.
fn close(conn: &Connection, s: &Settings, o: &Options, start_ms: i64) -> rusqlite::Result<()> {
    let idle_ms = s.idle_secs as i64 * 1000;
    atomic(conn, o.nested, || {
        let open: Vec<(i64, String)> = conn
            .prepare("SELECT id, last_call_at FROM sessions WHERE state = 'open' AND key_source = 'log' AND agent_label = ?1")?
            .query_map([o.source.name()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let now = time::text(start_ms);
        for (id, last) in open {
            let end = time::ms(&last) + idle_ms;
            if end < start_ms {
                crate::session::store::close(conn, id, end)?;
                crate::day::queue::put_session_days(
                    conn,
                    id,
                    s.tz,
                    crate::day::queue::SESSION_CLOSED,
                    &now,
                )?;
            }
        }
        Ok(())
    })
}

/// Reads every file of the source, then adds the calls. Each file holds its entries in memory only while it is read.
pub fn run(conn: &Connection, s: &Settings, o: &Options) -> Result<Outcome, Error> {
    let start_ms = chrono::Utc::now().timestamp_millis();
    let files = discover::files(o.source, &o.root).map_err(Error::Dir)?;
    let mut counts = Counts::default();
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();
    let mut all = vec![];
    let mut warning = None;
    let boundary = match o.source {
        Source::Pi => {
            let b = processed::pi_boundary(conn, &o.pi_label)?;
            if b.at_ms.is_none() && !b.others.is_empty() {
                warning = Some(format!(
                    "warning: no exchange has the agent label {}, so every Pi log session is backfilled. Fix: pass --pi-label with the label of the proxied Pi exchanges. Labels that exist: {}.",
                    o.pi_label,
                    b.others.join(", ")
                ));
            }
            Some(b)
        }
        Source::ClaudeCode => None,
    };
    let mut decided: HashMap<String, bool> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        let Ok(lines) = read::file(&f.path, o.source.name(), i, true) else {
            tracing::warn!(
                source = o.source.name(),
                file_index = i,
                "log file not readable; skipped"
            );
            continue;
        };
        counts.files_read += 1;
        counts.entries_read += lines.read;
        counts.entries_skipped += lines.skipped;
        let mut fc = match o.source {
            Source::ClaudeCode => claude::calls(&s.account_key, i, f, &lines.entries),
            Source::Pi => pi::calls(&s.account_key, i, &lines.entries),
        };
        drop(lines);
        for k in &fc.keys {
            index.entry(k.clone()).or_default().push(i);
        }
        for c in &mut fc.calls {
            c.captured = match &boundary {
                Some(b) => processed::pi_captured(fc.last_ms, b),
                None => match decided.get(&c.main_key) {
                    Some(v) => *v,
                    None => {
                        let v = processed::captured(conn, &c.main_key)?;
                        decided.insert(c.main_key.clone(), v);
                        v
                    }
                },
            };
        }
        all.extend(fc.calls);
    }
    let mut days = HashSet::new();
    let mut skipped = HashSet::new();
    for unit in units(all) {
        apply(conn, s, o, &unit, &mut counts, &mut days, &mut skipped)?;
    }
    counts.days_queued = days.len() as u64;
    close(conn, s, o, start_ms)?;
    tracing::info!(
        source = o.source.name(),
        files_read = counts.files_read,
        entries_read = counts.entries_read,
        calls_added = counts.calls_added,
        calls_existing = counts.calls_existing,
        calls_already_captured = counts.calls_already_captured,
        entries_skipped = counts.entries_skipped,
        "backfill calls done"
    );
    Ok(Outcome {
        counts,
        files,
        index,
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::usage::Usage;
    use crate::session::segment::tests::db;

    fn cand(file: usize, at: i64, n: usize) -> Candidate {
        Candidate {
            file,
            scope_key: "k".into(),
            main_key: "k".into(),
            log_key: format!("l{n}"),
            at_ms: at,
            upstream: "anthropic".into(),
            usage: Usage {
                input: 1,
                model: Some("m".into()),
                ..Default::default()
            },
            captured: false,
        }
    }

    fn segments(calls: Vec<Candidate>) -> Vec<(i64, i64, String)> {
        let c = db();
        let s = Settings {
            account_key: [0; 32],
            idle_secs: 60,
            tz: chrono_tz::UTC,
            providers: BTreeMap::new(),
        };
        let o = Options {
            source: Source::Pi,
            root: ".".into(),
            from_ms: None,
            to_ms: None,
            pi_label: "pi".into(),
            nested: false,
        };
        let (mut n, mut d, mut k) = (Counts::default(), HashSet::new(), HashSet::new());
        for u in units(calls) {
            apply(&c, &s, &o, &u, &mut n, &mut d, &mut k).unwrap();
        }
        c.prepare("SELECT segment, call_count, started_at FROM sessions ORDER BY segment")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn calls_sorted_across_files() {
        let a = vec![
            cand(0, 0, 1),
            cand(0, 200_000, 2),
            cand(1, 30_000, 3),
            cand(1, 230_000, 4),
        ];
        let b = vec![
            cand(0, 30_000, 3),
            cand(0, 230_000, 4),
            cand(1, 0, 1),
            cand(1, 200_000, 2),
        ];
        let sa = segments(a);
        assert_eq!(sa.len(), 2);
        assert_eq!((sa[0].1, sa[1].1), (2, 2));
        assert_eq!(sa, segments(b));
    }
}
