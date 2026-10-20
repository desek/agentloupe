//! @agents-index The gen-key subcommand: prints a fresh 32-byte key from the OS random source in padded base64.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::process::ExitCode;

/// Makes one new key; the caller decides where it goes, so nothing is stored here.
pub fn new_key() -> Result<String, getrandom::Error> {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf)?;
    Ok(STANDARD.encode(buf))
}

pub fn run() -> ExitCode {
    match new_key() {
        Ok(key) => {
            println!("{key}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!(
                "error: the random source of the operating system failed: {e}. Fix: run loupe gen-key again, or check the OS entropy source. Verify: the output decodes to 32 bytes."
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_key_is_32_bytes_and_unique() {
        let a = new_key().unwrap();
        let b = new_key().unwrap();
        assert_eq!(STANDARD.decode(&a).unwrap().len(), 32);
        assert_eq!(STANDARD.decode(&b).unwrap().len(), 32);
        assert_ne!(a, b);
    }
}
