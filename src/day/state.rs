//! @agents-index The day review setting (auto or manual) and the three states of a days row.

pub const PENDING_REVIEW: &str = "pending_review";
pub const PUBLISHED: &str = "published";
pub const DELETED: &str = "deleted";

/// LOUPE_DAY_REVIEW: in manual, a new day waits for `loupe day approve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Review {
    Auto,
    Manual,
}

impl Review {
    /// The configuration value; anything other than manual is auto, because validation runs first.
    pub fn parse(v: &str) -> Review {
        if v == "manual" {
            Review::Manual
        } else {
            Review::Auto
        }
    }

    /// The state of the first write of a day.
    pub fn first_state(self) -> &'static str {
        match self {
            Review::Auto => PUBLISHED,
            Review::Manual => PENDING_REVIEW,
        }
    }
}
