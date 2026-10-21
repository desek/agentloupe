//! @agents-index Command-line surface of loupe: the clap definitions and the dispatch to each subcommand.

mod aggregate;
mod backfill;
mod config_show;
#[path = "day.approve.rs"]
mod day_approve;
#[path = "day.common.rs"]
mod day_common;
#[path = "day.delete.rs"]
mod day_delete;
#[path = "day.edit.rs"]
mod day_edit;
#[path = "day.restore.rs"]
mod day_restore;
mod gen_key;
mod parse;
mod price;
mod serve;
mod summarize;
#[path = "summarize.rebuild.rs"]
mod summarize_rebuild;
mod token;

use clap::{Args, Parser, Subcommand};
use std::process::ExitCode;

/// loupe records AI agent activity and publishes it as a daily line.
#[derive(Parser)]
#[command(name = "loupe", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Load the configuration, open and migrate the database, then listen on LOUPE_LISTEN.
    Serve(ConfigArgs),
    /// Print one new random 32-byte key in standard base64 with padding. Stores nothing.
    GenKey,
    /// Manage the proxy tokens that clients put first in the proxy base URL.
    ///
    /// Exit codes: 0 done, 1 the store refused (a name exists, an unknown name, a database error), 2 bad input.
    Token {
        #[command(subcommand)]
        command: TokenCommand,
    },
    /// Find the model calls in the stored exchanges and group them into sessions.
    ///
    /// Prints exchanges_read, calls_added, sessions_opened, sessions_closed as key: value lines.
    /// Exit codes: 0 done, 1 a database or runtime failure, 2 bad input or configuration.
    Parse {
        /// Run one parse pass and one idle step, then exit. Required.
        #[arg(long)]
        once: bool,
        /// Report the counts and write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Print a sentence instead of key: value lines.
        #[arg(long)]
        human: bool,
        #[command(flatten)]
        args: ConfigArgs,
    },
    /// Summarize the closed session segments, build a day line, or retry failed summaries.
    ///
    /// --once prints segments_read, done, skipped, failed, disabled, retry_later, bodies_deleted,
    /// bodies_kept_decrypt_failed as key: value lines. --day prints day, status, session_lines.
    /// --retry prints rows. --rebuild-days prints summaries_reset.
    /// Exit codes: 0 done, 1 a database or runtime failure, 2 bad input or configuration.
    Summarize {
        /// Run one summary pass and one retention pass, then exit.
        #[arg(long, conflicts_with_all = ["day", "retry", "rebuild_days"])]
        once: bool,
        /// Build the day line of this day, YYYY-MM-DD in LOUPE_TZ.
        #[arg(long, value_name = "YYYY-MM-DD", conflicts_with_all = ["retry", "rebuild_days"])]
        day: Option<String>,
        /// Set failed and disabled summaries back to pending with 0 attempts.
        #[arg(long, conflicts_with = "rebuild_days")]
        retry: bool,
        /// Reset the full session and session day summaries of each day in the range to pending,
        /// then rebuild them and the day line of each day. Days in LOUPE_TZ.
        #[arg(long, value_name = "YYYY-MM-DD..YYYY-MM-DD")]
        rebuild_days: Option<String>,
        /// Print the stored day line too (with --day).
        #[arg(long)]
        show_line: bool,
        /// Report the counts; send no model request and write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Print a sentence instead of key: value lines.
        #[arg(long)]
        human: bool,
        #[command(flatten)]
        args: ConfigArgs,
    },
    /// Inspect the embedded prices and price the calls that have no price commit.
    ///
    /// Exit codes: 0 done, 1 a database or runtime failure, 2 bad input or configuration.
    Price {
        #[command(subcommand)]
        command: PriceCommand,
    },
    /// Build one day now and write its record, also when the day is not due.
    ///
    /// Prints day, calls, sessions, cost_usd, unpriced_calls, summary (present or absent), revision,
    /// written as key: value lines. Exit codes: 0 success, 2 bad input (for example --date), 1 internal error.
    Aggregate {
        /// The day: YYYY-MM-DD, or today, in LOUPE_TZ. Default: the previous day in LOUPE_TZ.
        #[arg(long, value_name = "YYYY-MM-DD|today")]
        date: Option<String>,
        /// Print the same lines with written: false; write nothing and ask for no day line.
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        args: ConfigArgs,
    },
    /// Review, edit, delete, or restore the record of one day.
    ///
    /// Each command prints day, state, summary (present or absent), written as key: value lines.
    /// Exit codes: 0 success, 2 bad input or a refused change, 1 internal error.
    Day {
        #[command(subcommand)]
        command: DayCommand,
    },
    /// Read the session logs of Claude Code or Pi into the calls and the sessions, as calls of origin log.
    ///
    /// Prints files_read, entries_read, calls_added, calls_existing, calls_already_captured,
    /// sessions_already_captured, calls_out_of_range, entries_skipped, sessions_opened, days_queued,
    /// summaries_done, summaries_skipped, summaries_retry_later, summaries_failed, summaries_disabled
    /// as key: value lines. Exit codes: 0 done, 1 a database failure, 2 bad input or configuration,
    /// 3 the log directory is missing or unreadable.
    Backfill {
        /// The log source: claude-code or pi. Required.
        #[arg(long, value_name = "claude-code|pi")]
        source: Option<String>,
        /// The root directory of the logs. Default: ~/.claude/projects or ~/.pi/agent/sessions.
        #[arg(long, value_name = "DIR")]
        path: Option<String>,
        /// The first day of the range, YYYY-MM-DD in LOUPE_TZ, inclusive.
        #[arg(long, value_name = "YYYY-MM-DD")]
        from: Option<String>,
        /// The last day of the range, YYYY-MM-DD in LOUPE_TZ, inclusive.
        #[arg(long, value_name = "YYYY-MM-DD")]
        to: Option<String>,
        /// The agent label of the proxied Pi exchanges; Pi log sessions before its first exchange are backfilled.
        #[arg(long, value_name = "LABEL", default_value = "pi")]
        pi_label: String,
        /// Read the files and print the counters; write nothing and send no summary request.
        #[arg(long)]
        dry_run: bool,
        /// Print a table instead of key: value lines.
        #[arg(long)]
        human: bool,
        #[command(flatten)]
        args: ConfigArgs,
    },
    /// Inspect the configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Print each effective value with its source (flag, env, file, default). Secrets print as set or unset.
    Show {
        #[command(flatten)]
        args: ConfigArgs,
        /// Print a table instead of one JSON object per line.
        #[arg(long)]
        human: bool,
    },
}

/// The flags that every loupe day command takes.
#[derive(Args)]
struct DayArgs {
    /// The day, YYYY-MM-DD in LOUPE_TZ, or today.
    #[arg(long, value_name = "YYYY-MM-DD|today")]
    date: String,
    /// Print the lines with written: false and write nothing.
    #[arg(long)]
    dry_run: bool,
    #[command(flatten)]
    args: ConfigArgs,
}

#[derive(Subcommand)]
enum DayCommand {
    /// Set a pending_review day to published.
    Approve(DayArgs),
    /// Write TEXT as the summary of the day; it survives each rebuild.
    Edit {
        /// The summary text of the day; not empty.
        #[arg(long)]
        summary: String,
        #[command(flatten)]
        day: DayArgs,
    },
    /// Make the day a tombstone; the publisher never creates it again.
    Delete(DayArgs),
    /// Build the day again, clear the deletion, and publish it (or send it to review in manual).
    Restore(DayArgs),
}

#[derive(Subcommand)]
enum PriceCommand {
    /// Print the price entry that a call of MODEL through UPSTREAM gets, as key: value lines, or match: none.
    Show {
        /// The model name of the call, for example claude-haiku-5-5.
        #[arg(long)]
        model: String,
        /// The upstream name from the upstreams file, for example anthropic.
        #[arg(long)]
        upstream: String,
        #[command(flatten)]
        args: ConfigArgs,
    },
    /// Price only the calls with a null price_commit; prints calls_priced, calls_unpriced, price_commit.
    Backfill {
        /// Print the counts and write nothing.
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        args: ConfigArgs,
    },
}

#[derive(Subcommand)]
enum TokenCommand {
    /// Print one new raw token on stdout. Opens no database and stores nothing; use it for LOUPE_BOOTSTRAP_TOKEN.
    Generate,
    /// Make an active token, store only its SHA-256 hash, and print the raw token one time.
    Create {
        /// The token name; matches ^[a-z0-9][a-z0-9-]{0,31}$, never bootstrap, never a name that exists.
        #[arg(long)]
        name: String,
        /// A tag KEY=VALUE; repeat for more, at most 16.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<String>,
        /// Validate and print what would happen; write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Print a sentence instead of one JSON object.
        #[arg(long)]
        human: bool,
        #[command(flatten)]
        args: ConfigArgs,
    },
    /// Show each token with its name, tags, dates, and status; never the raw token or the hash.
    List {
        /// Print a table instead of one JSON object per line.
        #[arg(long)]
        human: bool,
        #[command(flatten)]
        args: ConfigArgs,
    },
    /// Set revoked_at on the token NAME; a second revoke keeps the first time.
    Revoke {
        /// The name of the token to revoke.
        name: String,
        /// Validate and print what would happen; write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Print a sentence instead of one JSON object.
        #[arg(long)]
        human: bool,
        #[command(flatten)]
        args: ConfigArgs,
    },
}

/// Flags that override the environment, the file for the environment, and the defaults.
#[derive(Args, Clone, Default)]
pub struct ConfigArgs {
    /// Name of the environment; selects config/<env>.toml. Overrides LOUPE_ENV. Default: local.
    #[arg(long)]
    pub env: Option<String>,
    /// Address to listen on, as host:port. Overrides LOUPE_LISTEN.
    #[arg(long)]
    pub listen: Option<String>,
    /// Directory for the database and the logs. Overrides LOUPE_DATA_DIR.
    #[arg(long)]
    pub data_dir: Option<String>,
    /// Seconds without a call before a session ends. Overrides LOUPE_IDLE_SECS.
    #[arg(long)]
    pub idle_secs: Option<String>,
}

impl ConfigArgs {
    /// The flags as variable name and value pairs, the shape that the loader takes.
    pub fn as_flags(&self) -> Vec<(String, String)> {
        [
            ("LOUPE_ENV", &self.env),
            ("LOUPE_LISTEN", &self.listen),
            ("LOUPE_DATA_DIR", &self.data_dir),
            ("LOUPE_IDLE_SECS", &self.idle_secs),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.clone().map(|v| (k.to_string(), v)))
        .collect()
    }
}

/// Parses the command line and runs the selected subcommand.
pub fn run() -> ExitCode {
    match Cli::parse().command {
        Command::Serve(args) => serve::run(&args),
        Command::GenKey => gen_key::run(),
        Command::Parse {
            once,
            dry_run,
            human,
            args,
        } => parse::run(&args, once, dry_run, human),
        Command::Summarize {
            once,
            day,
            retry,
            rebuild_days,
            show_line,
            dry_run,
            human,
            args,
        } => {
            let mode = match (once, day, retry, rebuild_days) {
                (true, _, _, _) => Some(summarize::Mode::Once),
                (_, Some(d), _, _) => Some(summarize::Mode::Day(d)),
                (_, _, true, _) => Some(summarize::Mode::Retry),
                (_, _, _, Some(r)) => Some(summarize::Mode::Rebuild(r)),
                _ => None,
            };
            summarize::run(&args, mode, dry_run, show_line, human)
        }
        Command::Price { command } => match command {
            PriceCommand::Show {
                model,
                upstream,
                args,
            } => price::show(&args, &model, &upstream),
            PriceCommand::Backfill { dry_run, args } => price::backfill(&args, dry_run),
        },
        Command::Aggregate {
            date,
            dry_run,
            args,
        } => aggregate::run(&args, date.as_deref(), dry_run),
        Command::Day { command } => match command {
            DayCommand::Approve(d) => day_approve::run(&d.args, &d.date, d.dry_run),
            DayCommand::Edit { summary, day: d } => {
                day_edit::run(&d.args, &d.date, &summary, d.dry_run)
            }
            DayCommand::Delete(d) => day_delete::run(&d.args, &d.date, d.dry_run),
            DayCommand::Restore(d) => day_restore::run(&d.args, &d.date, d.dry_run),
        },
        Command::Backfill {
            source,
            path,
            from,
            to,
            pi_label,
            dry_run,
            human,
            args,
        } => backfill::run(
            &args,
            &backfill::Flags {
                source,
                path,
                from,
                to,
                pi_label,
                dry_run,
                human,
            },
        ),
        Command::Config {
            command: ConfigCommand::Show { args, human },
        } => config_show::run(&args, human),
        Command::Token { command } => match command {
            TokenCommand::Generate => token::generate(),
            TokenCommand::Create {
                name,
                tags,
                dry_run,
                human,
                args,
            } => token::create(&args, &name, &tags, dry_run, human),
            TokenCommand::List { human, args } => token::list(&args, human),
            TokenCommand::Revoke {
                name,
                dry_run,
                human,
                args,
            } => token::revoke(&args, &name, dry_run, human),
        },
    }
}
