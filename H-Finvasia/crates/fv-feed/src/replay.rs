//! Offline replay of captured frames.
//!
//! This is how the feed layer is tested and benchmarked without credentials or
//! market hours: capture a session as JSONL (one frame per line) and replay it.
//! It is also the mechanism behind `fv-cli replay` and `fv-cli bench`.

use crate::book::QuoteBook;
use crate::frame::{parse, Frame, FrameKind};
use crate::FeedError;
use std::io::BufRead;
use std::path::Path;

/// An in-memory list of raw frames.
#[derive(Debug, Default, Clone)]
pub struct Replay {
    frames: Vec<Vec<u8>>,
}

impl Replay {
    pub fn new(frames: Vec<Vec<u8>>) -> Self {
        Replay { frames }
    }

    /// Parse JSONL. Blank lines and `#` comments are ignored, so fixtures can be
    /// annotated.
    pub fn from_jsonl_str(text: &str) -> Self {
        let frames = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.as_bytes().to_vec())
            .collect();
        Replay { frames }
    }

    pub fn from_reader(reader: impl BufRead) -> Result<Self, FeedError> {
        let mut frames = Vec::new();
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            frames.push(line.as_bytes().to_vec());
        }
        Ok(Replay { frames })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FeedError> {
        let file = std::fs::File::open(path)?;
        Self::from_reader(std::io::BufReader::new(file))
    }

    pub fn frames(&self) -> &[Vec<u8>] {
        &self.frames
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// Outcome of replaying a frame list.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReplayStats {
    pub total: usize,
    pub parsed: usize,
    pub errors: usize,
    pub snapshots: usize,
    pub diffs: usize,
    pub orders: usize,
    pub acks: usize,
    pub unknown: usize,
    /// Frames that actually mutated the quote book.
    pub applied: usize,
    /// Distinct instruments seen.
    pub instruments: usize,
}

impl ReplayStats {
    pub fn error_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.errors as f64 / self.total as f64
        }
    }
}

/// Replay into a quote book, counting everything.
pub fn replay_into(book: &mut QuoteBook, frames: &[Vec<u8>], now_ms: i64) -> ReplayStats {
    let mut stats = ReplayStats::default();
    for (i, raw) in frames.iter().enumerate() {
        stats.total += 1;
        match parse(raw) {
            Ok(frame) => {
                stats.parsed += 1;
                match &frame {
                    Frame::Snapshot(_) => stats.snapshots += 1,
                    Frame::Diff(_) => stats.diffs += 1,
                    Frame::Order(_) => stats.orders += 1,
                    Frame::Ack(_) => stats.acks += 1,
                    Frame::Unknown { .. } => stats.unknown += 1,
                }
                if !matches!(
                    book.apply(&frame, now_ms + i as i64),
                    crate::book::Applied::Ignored
                ) {
                    stats.applied += 1;
                }
            }
            Err(_) => stats.errors += 1,
        }
    }
    stats.instruments = book.len();
    stats
}

/// Convenience for classification without parsing, used by capture tooling.
pub fn classify(raw: &[u8]) -> FrameKind {
    crate::frame::peek_kind(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/sample_feed.jsonl");

    #[test]
    fn jsonl_skips_comments_and_blanks() {
        let r = Replay::from_jsonl_str(
            "# a comment\n\n{\"t\":\"tk\",\"e\":\"NSE\",\"tk\":\"1\",\"lp\":\"10.00\"}\n",
        );
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn fixture_replays_cleanly() {
        let replay = Replay::from_jsonl_str(FIXTURE);
        let mut book = QuoteBook::new();
        let stats = replay_into(&mut book, replay.frames(), 1_000);
        assert_eq!(stats.errors, 0, "every fixture frame must parse");
        assert!(stats.snapshots > 0);
        assert!(stats.diffs > 0);
        assert!(stats.orders > 0);
        assert!(book.len() >= 2);
    }

    #[test]
    fn error_rate_is_reported_for_bad_input() {
        let mut book = QuoteBook::new();
        let frames = vec![
            br#"{"t":"tk","e":"NSE","tk":"1","lp":"10.00"}"#.to_vec(),
            b"{broken".to_vec(),
        ];
        let stats = replay_into(&mut book, &frames, 0);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.errors, 1);
        assert!((stats.error_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn classification_matches_parser() {
        assert_eq!(
            classify(br#"{"t":"om","norenordno":"1"}"#),
            FrameKind::OrderUpdate
        );
    }
}
