//! @agents-index Bounded copy of one forwarded body: keeps at most the capture limit, counts the full size.

/// The capture of one body. The forwarded stream never waits for it and never sees it.
#[derive(Debug, Default)]
pub struct Tap {
    pub kept: Vec<u8>,
    /// The full size as forwarded, also past the capture limit.
    pub size: u64,
    pub truncated: bool,
    limit: usize,
}

impl Tap {
    pub fn new(limit: usize) -> Self {
        Tap {
            limit,
            ..Tap::default()
        }
    }

    /// Copies what still fits under the capture limit; sets the flag on the first byte that does not.
    pub fn push(&mut self, chunk: &[u8]) {
        self.size += chunk.len() as u64;
        let room = self.limit.saturating_sub(self.kept.len());
        if chunk.len() > room {
            self.truncated = true;
        }
        self.kept.extend_from_slice(&chunk[..chunk.len().min(room)]);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn truncates_at_limit_and_counts_size() {
        let mut t = super::Tap::new(1 << 20);
        let chunk = vec![7u8; 64 * 1024];
        for _ in 0..48 {
            t.push(&chunk);
        }
        assert_eq!(t.kept.len(), 1 << 20);
        assert!(t.truncated);
        assert_eq!(t.size, 3 << 20);
    }
}
