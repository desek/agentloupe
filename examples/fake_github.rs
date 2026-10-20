//! @agents-index Runs the fake GitHub API on a port with N merged pull requests of one day, for github.bench.sh.
//!
//! Usage: cargo run --example fake_github -- <port> <merged> <day> <token>
//! The token user has the login `bench`; each pull request is opened and merged on `day`.

#[path = "../tests/common/fake_github.rs"]
mod fake_github;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, port, merged, day, token] = a.as_slice() else {
        eprintln!(
            "error: needs <port> <merged> <day> <token>. Fix: run fake_github 8790 50 2026-10-18 t. Verify: GET /user returns a login"
        );
        std::process::exit(2);
    };
    let mut w = fake_github::world(token, "bench");
    let n: u64 = merged.parse().expect("merged is a whole number");
    w.prs = (1..=n)
        .map(|i| fake_github::pr("bench", i, day, Some(day)))
        .collect();
    let f = fake_github::start_on(port.parse().expect("port is a number"), w);
    eprintln!("fake github listening on {}", f.url);
    loop {
        std::thread::park();
    }
}
