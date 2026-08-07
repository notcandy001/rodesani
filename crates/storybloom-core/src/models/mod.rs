//! Domain models.
//!
//! Plain data types representing StoryBloom's domain. Kept free of
//! `sqlx`/persistence specifics where possible so the domain shape doesn't
//! leak storage details.

pub mod story;

pub use story::{Hashtag, HashtagError, StoryDuration, StoryRequest, StoryResult, StoryType, Tone};

