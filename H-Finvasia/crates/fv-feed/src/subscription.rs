//! Subscription planning.
//!
//! The legacy terminal issued one subscribe call per scrip (and one extra IV
//! subscribe plus a `Get Scrips Info` request per strike), then silently hit the
//! broker's cap — `Max Watch Scrips Sub Limit reached`.
//!
//! Here the watch set is diffed against the target and emitted as batched
//! `{"t":"t","k":"A|1#B|2"}` frames. Anything that would exceed the budget is
//! *reported*, never silently dropped.

use fv_core::SymbolKey;
use std::collections::BTreeSet;

/// Maximum instruments per subscribe frame. The venue does not publish a limit;
/// 500 keeps frames well under typical 16 KiB websocket soft limits.
pub const DEFAULT_CHUNK: usize = 500;

/// Result of reconciling the desired watch set against the active one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Instruments to subscribe, in deterministic order.
    pub subscribe: Vec<SymbolKey>,
    /// Instruments to unsubscribe, in deterministic order.
    pub unsubscribe: Vec<SymbolKey>,
    /// Targets that did not fit in the budget. Surfaced to the UI so a truncated
    /// option chain is visible instead of looking like missing data.
    pub rejected: Vec<SymbolKey>,
    /// True when `rejected` is non-empty.
    pub truncated: bool,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.subscribe.is_empty() && self.unsubscribe.is_empty()
    }
}

/// The set of instruments the app wants ticks for, with a hard budget.
#[derive(Debug, Clone)]
pub struct WatchSet {
    active: BTreeSet<SymbolKey>,
    max: usize,
}

impl WatchSet {
    pub fn new(max: usize) -> Self {
        WatchSet {
            active: BTreeSet::new(),
            max,
        }
    }

    pub fn max(&self) -> usize {
        self.max
    }

    pub fn active(&self) -> &BTreeSet<SymbolKey> {
        &self.active
    }

    pub fn len(&self) -> usize {
        self.active.len()
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    /// Compute (and commit) the minimal set of subscribe/unsubscribe operations
    /// that moves `active` to `target`.
    ///
    /// `target` is capped at `max`; the overflow is returned in `rejected`.
    /// Ordering is deterministic, which makes plans diffable in tests and logs.
    pub fn reconcile(&mut self, target: &BTreeSet<SymbolKey>) -> Plan {
        let mut capped: BTreeSet<SymbolKey> = target.iter().take(self.max).cloned().collect();
        let rejected: Vec<SymbolKey> = target.iter().skip(self.max).cloned().collect();

        let subscribe: Vec<SymbolKey> = capped.difference(&self.active).cloned().collect();
        let unsubscribe: Vec<SymbolKey> = self.active.difference(&capped).cloned().collect();

        self.active = std::mem::take(&mut capped);

        Plan {
            truncated: !rejected.is_empty(),
            subscribe,
            unsubscribe,
            rejected,
        }
    }

    /// Drop everything (logout / disconnect).
    pub fn clear(&mut self) -> Plan {
        let unsubscribe: Vec<SymbolKey> = self.active.iter().cloned().collect();
        self.active.clear();
        Plan {
            unsubscribe,
            ..Default::default()
        }
    }
}

/// Build one or more subscribe frames for `keys`, chunked to `chunk` entries.
pub fn subscribe_frames(keys: &[SymbolKey], chunk: usize) -> Vec<String> {
    build_frames("t", keys, chunk)
}

/// Build one or more unsubscribe frames for `keys`, chunked to `chunk` entries.
pub fn unsubscribe_frames(keys: &[SymbolKey], chunk: usize) -> Vec<String> {
    build_frames("u", keys, chunk)
}

fn build_frames(kind: &str, keys: &[SymbolKey], chunk: usize) -> Vec<String> {
    let chunk = chunk.max(1);
    if keys.is_empty() {
        return Vec::new();
    }
    keys.chunks(chunk)
        .map(|group| {
            let joined = group
                .iter()
                .map(|k| k.as_wire())
                .collect::<Vec<_>>()
                .join("#");
            format!(r#"{{"t":"{kind}","k":"{joined}"}}"#)
        })
        .collect()
}

/// The connect frame sent immediately after the socket opens.
///
/// This is the **legacy** Noren handshake: `t:"c"` with `susertoken`. The newer
/// OAuth gateways use `t:"a"` + `accesstoken`; both are accepted server-side
/// depending on how the session was minted, but the accounts in use here are
/// minted through the legacy path.
pub fn connect_frame(uid: &str, actid: &str, token: &str, source: &str) -> String {
    format!(
        r#"{{"t":"c","uid":"{}","actid":"{}","susertoken":"{}","source":"{}"}}"#,
        json_escape(uid),
        json_escape(actid),
        json_escape(token),
        json_escape(source)
    )
}

/// Keepalive. The venue drops idle sockets; send this every few seconds.
pub const HEARTBEAT_FRAME: &str = r#"{"t":"h"}"#;

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(wire: &[&str]) -> BTreeSet<SymbolKey> {
        wire.iter().map(|s| SymbolKey::parse(s).unwrap()).collect()
    }

    #[test]
    fn first_reconcile_subscribes_everything() {
        let mut ws = WatchSet::new(100);
        let plan = ws.reconcile(&keys(&["NSE|22", "NFO|158614"]));
        assert_eq!(
            plan.subscribe
                .iter()
                .map(|k| k.as_wire())
                .collect::<Vec<_>>(),
            vec!["NFO|158614", "NSE|22"] // BTreeSet order
        );
        assert!(plan.unsubscribe.is_empty());
        assert!(!plan.truncated);
        assert_eq!(ws.len(), 2);
    }

    #[test]
    fn second_reconcile_is_a_noop() {
        let mut ws = WatchSet::new(100);
        let target = keys(&["NSE|22"]);
        ws.reconcile(&target);
        let plan = ws.reconcile(&target);
        assert!(plan.is_empty());
    }

    #[test]
    fn reconcile_emits_minimal_diff() {
        let mut ws = WatchSet::new(100);
        ws.reconcile(&keys(&["NSE|1", "NSE|2", "NSE|3"]));
        let plan = ws.reconcile(&keys(&["NSE|2", "NSE|3", "NSE|4"]));
        assert_eq!(
            plan.subscribe
                .iter()
                .map(|k| k.as_wire())
                .collect::<Vec<_>>(),
            vec!["NSE|4"]
        );
        assert_eq!(
            plan.unsubscribe
                .iter()
                .map(|k| k.as_wire())
                .collect::<Vec<_>>(),
            vec!["NSE|1"]
        );
    }

    #[test]
    fn budget_overflow_is_reported_not_silently_dropped() {
        let mut ws = WatchSet::new(2);
        let plan = ws.reconcile(&keys(&["NSE|1", "NSE|2", "NSE|3", "NSE|4"]));
        assert_eq!(ws.len(), 2);
        assert!(plan.truncated);
        assert_eq!(
            plan.rejected
                .iter()
                .map(|k| k.as_wire())
                .collect::<Vec<_>>(),
            vec!["NSE|3", "NSE|4"]
        );
    }

    #[test]
    fn shrinking_the_budget_unsubscribes_the_excess() {
        let mut ws = WatchSet::new(3);
        ws.reconcile(&keys(&["NSE|1", "NSE|2", "NSE|3"]));
        let plan = ws.reconcile(&keys(&["NSE|1"]));
        assert_eq!(plan.unsubscribe.len(), 2);
        assert_eq!(ws.len(), 1);
    }

    #[test]
    fn clear_unsubscribes_everything() {
        let mut ws = WatchSet::new(10);
        ws.reconcile(&keys(&["NSE|1", "NSE|2"]));
        let plan = ws.clear();
        assert_eq!(plan.unsubscribe.len(), 2);
        assert!(ws.is_empty());
    }

    #[test]
    fn subscribe_frames_are_batched_and_joined() {
        let k = keys(&["NSE|22", "BSE|508123", "NSE|NIFTY"]);
        let list: Vec<SymbolKey> = k.into_iter().collect();
        let frames = subscribe_frames(&list, 2);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], r#"{"t":"t","k":"BSE|508123#NSE|22"}"#);
        assert_eq!(frames[1], r#"{"t":"t","k":"NSE|NIFTY"}"#);
    }

    #[test]
    fn empty_key_list_produces_no_frames() {
        assert!(subscribe_frames(&[], 500).is_empty());
        assert!(unsubscribe_frames(&[], 500).is_empty());
    }

    #[test]
    fn connect_frame_uses_the_legacy_handshake() {
        let frame = connect_frame("AB1234", "AB1234", "tok", "WEB");
        let value: serde_json::Value = serde_json::from_str(&frame).unwrap();
        assert_eq!(value["t"], "c");
        assert_eq!(value["susertoken"], "tok");
        assert_eq!(value["source"], "WEB");
        assert_eq!(value["actid"], "AB1234");
    }

    #[test]
    fn connect_frame_escapes_dangerous_characters() {
        let frame = connect_frame("ab\"c", "ac", "to\\ken", "API");
        assert!(frame.contains(r#""uid":"ab\"c""#));
        assert!(frame.contains(r#""susertoken":"to\\ken""#));
        let value: serde_json::Value = serde_json::from_str(&frame).unwrap();
        assert_eq!(value["t"], "c");
        assert_eq!(value["source"], "API");
    }

    #[test]
    fn heartbeat_frame_is_minimal_json() {
        let value: serde_json::Value = serde_json::from_str(HEARTBEAT_FRAME).unwrap();
        assert_eq!(value["t"], "h");
    }
}
