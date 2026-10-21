//! @agents-index Finds the log files of a source under its root: Claude Code main and subagent files, or Pi session files.

use super::Source;
use std::path::{Path, PathBuf};

/// One log file. The path stays in memory only; the backfill never stores or logs it.
#[derive(Debug, Clone, PartialEq)]
pub struct LogFile {
    pub path: PathBuf,
    /// For a Claude Code subagent file, the session ID of its main session (the name of its session directory).
    pub subagent_of: Option<String>,
}

fn jsonl(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    v.sort();
    v
}

fn dirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

/// The files of `source` under `root`, in path order. Fails only when `root` cannot be read.
pub fn files(source: Source, root: &Path) -> std::io::Result<Vec<LogFile>> {
    std::fs::read_dir(root)?;
    let mut out = vec![];
    for project in dirs(root) {
        for path in jsonl(&project) {
            out.push(LogFile {
                path,
                subagent_of: None,
            });
        }
        if source == Source::Pi {
            continue;
        }
        for session in dirs(&project) {
            let main = session
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
            for path in jsonl(&session.join("subagents")) {
                out.push(LogFile {
                    path,
                    subagent_of: main.clone(),
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "{}\n").unwrap();
    }

    #[test]
    fn finds_main_and_subagent_files() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        touch(&r.join("proj/s1.jsonl"));
        touch(&r.join("proj/s1/subagents/agent-a.jsonl"));
        touch(&r.join("proj/s1/tool-results/x.jsonl"));
        touch(&r.join("proj/notes.txt"));
        touch(&r.join("top.jsonl"));
        let f = files(Source::ClaudeCode, r).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].path, r.join("proj/s1.jsonl"));
        assert_eq!(f[0].subagent_of, None);
        assert_eq!(f[1].path, r.join("proj/s1/subagents/agent-a.jsonl"));
        assert_eq!(f[1].subagent_of.as_deref(), Some("s1"));
    }

    #[test]
    fn pi_reads_only_sessions_dir() {
        let t = tempfile::tempdir().unwrap();
        let agent = t.path().join("agent");
        touch(&agent.join("auth.json"));
        touch(&agent.join("other/x.jsonl"));
        touch(&agent.join("sessions/slug/a.jsonl"));
        touch(&agent.join("sessions/slug/deeper/b.jsonl"));
        touch(&agent.join("sessions/top.jsonl"));
        let f = files(Source::Pi, &agent.join("sessions")).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].path, agent.join("sessions/slug/a.jsonl"));
        assert!(files(Source::Pi, &t.path().join("missing")).is_err());
    }
}
