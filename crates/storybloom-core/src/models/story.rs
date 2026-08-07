//! Domain types for the Story Engine.
//!
//! These are deliberately independent of the OpenAI wire format (see
//! `crate::services::openai_client`) - callers build a [`StoryRequest`] and
//! get back a [`StoryResult`] without ever touching HTTP or JSON details.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------
// StoryType
// ---------------------------------------------------------------------

/// The genre/category of story to generate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryType {
    Adventure,
    Romance,
    Horror,
    Comedy,
    Drama,
    SciFi,
    Fantasy,
    Mystery,
    Thriller,
    Inspirational,
    /// Escape hatch for genres not covered above, e.g. "cozy mystery" or
    /// "cyberpunk noir". Kept as a variant (rather than making the whole
    /// type a plain `String`) so the common cases stay exhaustively
    /// matchable while still allowing arbitrary input.
    Custom(String),
}

impl fmt::Display for StoryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoryType::Adventure => write!(f, "Adventure"),
            StoryType::Romance => write!(f, "Romance"),
            StoryType::Horror => write!(f, "Horror"),
            StoryType::Comedy => write!(f, "Comedy"),
            StoryType::Drama => write!(f, "Drama"),
            StoryType::SciFi => write!(f, "Science Fiction"),
            StoryType::Fantasy => write!(f, "Fantasy"),
            StoryType::Mystery => write!(f, "Mystery"),
            StoryType::Thriller => write!(f, "Thriller"),
            StoryType::Inspirational => write!(f, "Inspirational"),
            StoryType::Custom(label) => write!(f, "{label}"),
        }
    }
}

// ---------------------------------------------------------------------
// Tone
// ---------------------------------------------------------------------

/// The emotional register the story should be written in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Lighthearted,
    Serious,
    Suspenseful,
    Humorous,
    Dark,
    Romantic,
    Inspirational,
    Whimsical,
    Melancholic,
    Custom(String),
}

impl fmt::Display for Tone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tone::Lighthearted => write!(f, "Lighthearted"),
            Tone::Serious => write!(f, "Serious"),
            Tone::Suspenseful => write!(f, "Suspenseful"),
            Tone::Humorous => write!(f, "Humorous"),
            Tone::Dark => write!(f, "Dark"),
            Tone::Romantic => write!(f, "Romantic"),
            Tone::Inspirational => write!(f, "Inspirational"),
            Tone::Whimsical => write!(f, "Whimsical"),
            Tone::Melancholic => write!(f, "Melancholic"),
            Tone::Custom(label) => write!(f, "{label}"),
        }
    }
}

// ---------------------------------------------------------------------
// StoryDuration
// ---------------------------------------------------------------------

/// Target narration length, expressed as presets rather than a raw number
/// so the engine can translate it into concrete prompt guidance (word
/// count) instead of every caller having to know that mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryDuration {
    /// A short-form story, ~30-60 seconds narrated aloud.
    Short,
    /// A mid-length story, ~2-3 minutes narrated aloud.
    Medium,
    /// A long-form story, ~5-8 minutes narrated aloud.
    Long,
}

impl StoryDuration {
    /// Approximate spoken length in seconds, assuming a natural narration
    /// pace (~150 words/minute). Used for prompt guidance and can also be
    /// surfaced to a UI later (e.g. "~45s").
    pub fn approx_seconds(&self) -> u32 {
        match self {
            StoryDuration::Short => 45,
            StoryDuration::Medium => 150,
            StoryDuration::Long => 420,
        }
    }

    /// `(min_words, max_words)` the generated `story` field should aim
    /// for, based on the same ~150 words/minute narration pace.
    pub fn target_word_count(&self) -> (u32, u32) {
        match self {
            StoryDuration::Short => (60, 130),
            StoryDuration::Medium => (250, 450),
            StoryDuration::Long => (700, 1200),
        }
    }
}

impl fmt::Display for StoryDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoryDuration::Short => write!(f, "short (~30-60s)"),
            StoryDuration::Medium => write!(f, "medium (~2-3 min)"),
            StoryDuration::Long => write!(f, "long (~5-8 min)"),
        }
    }
}

// ---------------------------------------------------------------------
// Hashtag
// ---------------------------------------------------------------------

/// A validated, normalized hashtag - always non-empty and always
/// `#`-prefixed with no internal whitespace, regardless of how the model
/// (or a future UI) supplied it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Hashtag(String);

impl Hashtag {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Hashtag {
    type Error = HashtagError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let trimmed = value.trim();
        let body = trimmed.strip_prefix('#').unwrap_or(trimmed);

        if body.is_empty() {
            return Err(HashtagError::Empty);
        }
        if body.chars().any(|c| c.is_whitespace() || c == '#') {
            return Err(HashtagError::InvalidCharacters(value));
        }

        Ok(Hashtag(format!("#{body}")))
    }
}

impl From<Hashtag> for String {
    fn from(hashtag: Hashtag) -> Self {
        hashtag.0
    }
}

impl fmt::Display for Hashtag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Error)]
pub enum HashtagError {
    #[error("hashtag must not be empty")]
    Empty,
    #[error("hashtag {0:?} contains whitespace or a stray '#'")]
    InvalidCharacters(String),
}

// ---------------------------------------------------------------------
// StoryRequest / StoryResult
// ---------------------------------------------------------------------

/// Input to the Story Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryRequest {
    pub story_type: StoryType,
    pub duration: StoryDuration,
    pub tone: Tone,
}

impl StoryRequest {
    pub fn new(story_type: StoryType, duration: StoryDuration, tone: Tone) -> Self {
        Self {
            story_type,
            duration,
            tone,
        }
    }
}

/// Output of the Story Engine. Deserialized directly from the model's
/// structured JSON response - see `services::story_engine`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryResult {
    pub title: String,
    pub story: String,
    pub description: String,
    pub hashtags: Vec<Hashtag>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashtag_normalizes_missing_prefix() {
        let tag = Hashtag::try_from("adventure".to_string()).unwrap();
        assert_eq!(tag.as_str(), "#adventure");
    }

    #[test]
    fn hashtag_keeps_existing_prefix() {
        let tag = Hashtag::try_from("#adventure".to_string()).unwrap();
        assert_eq!(tag.as_str(), "#adventure");
    }

    #[test]
    fn hashtag_rejects_whitespace() {
        assert!(Hashtag::try_from("not a tag".to_string()).is_err());
    }

    #[test]
    fn hashtag_rejects_empty() {
        assert!(Hashtag::try_from("#".to_string()).is_err());
        assert!(Hashtag::try_from(String::new()).is_err());
    }

    #[test]
    fn story_type_displays_custom_label_verbatim() {
        let story_type = StoryType::Custom("Cyberpunk Noir".to_string());
        assert_eq!(story_type.to_string(), "Cyberpunk Noir");
    }
}
