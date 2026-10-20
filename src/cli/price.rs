//! @agents-index The price subcommands: show prints the matched price entry, backfill prices the calls with a null price commit.

use super::ConfigArgs;
use crate::config::loader::{self, Loaded};
use crate::price::embed::PRICE_COMMIT;
use crate::price::index::Index;
use crate::price::lookup::find;
use crate::proxy::upstreams;
use std::collections::BTreeMap;
use std::process::ExitCode;

const EXIT_CONFIG: u8 = 2;
const EXIT_RUNTIME: u8 = 1;

/// The base fields of the five token terms; the function reads these and their tier variants.
const BASES: [&str; 5] = [
    "input_cost_per_token",
    "cache_read_input_token_cost",
    "cache_creation_input_token_cost",
    "cache_creation_input_token_cost_above_1hr",
    "output_cost_per_token",
];

fn fail(code: u8, message: String) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(code)
}

fn value(cfg: &Loaded, name: &str) -> String {
    cfg.get(name)
        .and_then(|e| e.value.as_ref())
        .map(|v| v.expose().to_string())
        .unwrap_or_default()
}

/// The configuration and the upstream to provider map, or the exit code of the failure.
fn setup(args: &ConfigArgs) -> Result<(Loaded, BTreeMap<String, String>), ExitCode> {
    let cfg = loader::load_from_process(&args.as_flags())
        .map_err(|e| fail(EXIT_CONFIG, e.to_string()))?;
    let file = value(&cfg, "LOUPE_UPSTREAMS_FILE");
    let u = upstreams::load(std::path::Path::new(&file)).map_err(|e| fail(EXIT_CONFIG, e))?;
    Ok((cfg, upstreams::providers(&u)))
}

/// Prints the entry that a call of `model` through `upstream` gets, as key: value lines.
pub fn show(args: &ConfigArgs, model: &str, upstream: &str) -> ExitCode {
    let providers = match setup(args) {
        Ok((_, p)) => p,
        Err(code) => return code,
    };
    let Some(provider) = providers.get(upstream) else {
        let known: Vec<&String> = providers.keys().collect();
        return fail(
            EXIT_CONFIG,
            format!(
                "error: the upstream {upstream} is not in the upstreams file. Fix: pass --upstream with one of {known:?}, or add an entry with base_url and litellm_provider to the file of LOUPE_UPSTREAMS_FILE. Reference: docs/specification/proxy.md, section Allowlist. Verify: loupe price show prints match."
            ),
        );
    };
    println!("price_commit: {PRICE_COMMIT}");
    let Some(entry) = find(Index::embedded(), model, provider) else {
        println!("match: none");
        println!("provider: {provider}");
        return ExitCode::SUCCESS;
    };
    println!("match: {}", entry.key);
    println!("provider: {}", entry.provider);
    for (k, v) in &entry.fields {
        if BASES.iter().any(|b| crate::price::tier::is_read(k, b)) {
            println!("{k}: {v}");
        }
    }
    ExitCode::SUCCESS
}

/// Prices the calls with a null price_commit; prints calls_priced, calls_unpriced, price_commit.
pub fn backfill(args: &ConfigArgs, dry_run: bool) -> ExitCode {
    let (cfg, providers) = match setup(args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let data_dir = std::path::PathBuf::from(value(&cfg, "LOUPE_DATA_DIR"));
    let db_fail = |e: String| {
        fail(
            EXIT_RUNTIME,
            format!(
                "error: the backfill of the database under {} failed: {e}. Fix 1: start loupe serve one time so that the database exists and is migrated. Fix 2: point LOUPE_DATA_DIR or --data-dir at the data dir of loupe serve. Reference: docs/specification/cost.md. Verify: loupe price backfill --dry-run exits 0.",
                data_dir.display()
            ),
        )
    };
    if !dry_run && let Err(e) = crate::log::init(&data_dir) {
        return db_fail(e.to_string());
    }
    let mut conn = match crate::store::open::open(&data_dir) {
        Ok(c) => c,
        Err(e) => return db_fail(e.to_string()),
    };
    if !dry_run && let Err(e) = crate::store::migrate::migrate(&mut conn) {
        return db_fail(e.to_string());
    }
    match crate::parse::backfill::run(&conn, &providers, dry_run) {
        Ok(c) => {
            println!("calls_priced: {}", c.calls_priced);
            println!("calls_unpriced: {}", c.calls_unpriced);
            println!("price_commit: {PRICE_COMMIT}");
            ExitCode::SUCCESS
        }
        Err(e) => db_fail(e.to_string()),
    }
}
