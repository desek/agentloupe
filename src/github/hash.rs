//! @agents-index The pull request hash: HMAC-SHA256 under LOUPE_ACCOUNT_KEY of owner/repo#number in lowercase, as lowercase hex.

/// The only identity of a pull request that the binary stores.
pub fn pr_hash(key: &[u8; 32], owner: &str, repo: &str, number: u64) -> String {
    let text = format!("{owner}/{repo}#{number}").to_lowercase();
    crate::capture::credential::hmac_hex(key, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::pr_hash;

    #[test]
    fn hash_is_hmac_of_lowercase_ref() {
        let a = pr_hash(&[7; 32], "Owner", "Repo", 7);
        let b = pr_hash(&[7; 32], "owner", "repo", 7);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
        assert_ne!(a, pr_hash(&[8; 32], "owner", "repo", 7));
    }
}
