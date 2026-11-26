//! UI-facing bridge between the market engine and the desktop shell.
//!
//! This crate intentionally has **no Tauri dependency**: `src-tauri` is a thin
//! adapter, and everything here is exercised by plain unit tests.

pub mod dto;
pub mod market;

pub use dto::{DeltaDto, InstrumentDto, PlanDto, PriceDto, QuoteDto, SourceKind, StatusDto, Trend};
pub use market::{build_label, MarketData, DEFAULT_MAX_PER_FRAME, DEFAULT_WATCH_LIMIT};

#[cfg(test)]
mod tests {
    use super::*;
    use fv_core::SymbolKey;
    use fv_master::{Instrument, MemoryMaster, StrikeType};
    use std::sync::Arc;
    use std::time::Instant;

    fn market() -> MarketData {
        MarketData::new("test")
    }

    fn apply(md: &mut MarketData, raw: &[u8]) {
        md.apply_raw(raw, 0).expect("frame must decode");
    }

    fn master() -> Arc<MemoryMaster> {
        let items = vec![
            Instrument {
                key: SymbolKey::parse("NSE|2885").unwrap(),
                trading_symbol: "RELIANCE-EQ".into(),
                name: Some("Reliance Industries".into()),
                underlying: Some("RELIANCE".into()),
                expiry: None,
                strike: fv_core::Price::ZERO,
                strike_type: StrikeType::Other,
                lot_size: 1,
                instrument_type: "EQ".into(),
                segment: Some("NSE-EQ".into()),
                tick_size: None,
            },
            Instrument {
                key: SymbolKey::parse("NFO|158614").unwrap(),
                trading_symbol: "ZYDUSLIFE23FEB27P1680".into(),
                name: Some("ZYDUSLIFE".into()),
                underlying: Some("ZYDUSLIFE".into()),
                expiry: Some("2027-02-23".into()),
                strike: fv_core::Price::parse("1680").unwrap(),
                strike_type: StrikeType::Put,
                lot_size: 450,
                instrument_type: "PE".into(),
                segment: Some("NFO-OPT".into()),
                tick_size: None,
            },
        ];
        Arc::new(MemoryMaster::build(items).expect("master builds"))
    }

    #[test]
    fn snapshot_then_diff_produces_a_merged_quote() {
        let mut md = market();
        apply(
            &mut md,
            br#"{"t":"tk","e":"NSE","tk":"22","ts":"ACC-EQ","pp":"2","lp":"1287.30","o":"1280.00","v":"980515","bp1":"1287.25","sp1":"1287.35"}"#,
        );
        apply(
            &mut md,
            br#"{"t":"tf","e":"NSE","tk":"22","lp":"1288.00","v":"981000"}"#,
        );

        let q = md.snapshot("NSE|22").expect("quote exists");
        assert_eq!(q.ltp.as_ref().unwrap().text, "1288.00");
        assert_eq!(
            q.open.as_ref().unwrap().text,
            "1280.00",
            "must be preserved"
        );
        assert_eq!(q.volume, Some(981_000));
        assert_eq!(q.bid.as_ref().unwrap().text, "1287.25");
        assert_eq!(q.trading_symbol.as_deref(), Some("ACC-EQ"));
    }

    #[test]
    fn price_precision_is_honoured() {
        let mut md = market();
        apply(
            &mut md,
            br#"{"t":"tk","e":"CDS","tk":"1","pp":"4","lp":"83.5125"}"#,
        );
        let q = md.snapshot("CDS|1").unwrap();
        assert_eq!(q.ltp.as_ref().unwrap().text, "83.5125");
    }

    #[test]
    fn trend_follows_change_against_previous_close() {
        let mut md = market();
        apply(
            &mut md,
            br#"{"t":"tk","e":"NSE","tk":"1","pp":"2","lp":"101.50","c":"100.00"}"#,
        );
        assert_eq!(md.snapshot("NSE|1").unwrap().trend, Trend::Up);

        let mut md = market();
        apply(
            &mut md,
            br#"{"t":"tk","e":"NSE","tk":"1","pp":"2","lp":"99.00","c":"100.00"}"#,
        );
        assert_eq!(md.snapshot("NSE|1").unwrap().trend, Trend::Down);

        let mut md = market();
        apply(
            &mut md,
            br#"{"t":"tk","e":"NSE","tk":"1","pp":"2","lp":"100.00","c":"100.00"}"#,
        );
        assert_eq!(md.snapshot("NSE|1").unwrap().trend, Trend::Flat);
    }

    #[test]
    fn change_is_none_without_previous_close() {
        let mut md = market();
        apply(
            &mut md,
            br#"{"t":"tk","e":"NSE","tk":"1","pp":"2","lp":"101.50"}"#,
        );
        let q = md.snapshot("NSE|1").unwrap();
        assert!(q.change.is_none());
        assert_eq!(q.trend, Trend::Flat);
    }

    #[test]
    fn watch_change_reports_a_minimal_plan() {
        let mut md = market();
        let a = SymbolKey::parse("NSE|1").unwrap();
        let b = SymbolKey::parse("NSE|2").unwrap();
        let c = SymbolKey::parse("NSE|3").unwrap();

        let plan = md.set_watch(vec![a.clone(), b.clone()]);
        assert_eq!(plan.subscribe, vec!["NSE|1", "NSE|2"]);
        assert!(plan.unsubscribe.is_empty());

        let plan = md.set_watch(vec![b.clone(), c.clone()]);
        assert_eq!(plan.subscribe, vec!["NSE|3"]);
        assert_eq!(plan.unsubscribe, vec!["NSE|1"]);
    }

    #[test]
    fn removing_from_watch_drops_the_quote_to_bound_memory() {
        let mut md = market();
        md.add_to_watch(SymbolKey::parse("NSE|1").unwrap());
        md.add_to_watch(SymbolKey::parse("NSE|2").unwrap());
        apply(&mut md, br#"{"t":"tk","e":"NSE","tk":"1","lp":"10.00"}"#);
        apply(&mut md, br#"{"t":"tk","e":"NSE","tk":"2","lp":"20.00"}"#);
        assert_eq!(md.watch_quotes().len(), 2);

        md.remove_from_watch("NSE|1");
        assert!(md.snapshot("NSE|1").is_none(), "quote must be evicted");
        assert_eq!(md.watch_quotes().len(), 1);
    }

    #[test]
    fn watch_overflow_is_reported_not_silently_dropped() {
        let mut md = market().with_watch_limit(2);
        let keys: Vec<SymbolKey> = (1..=4)
            .map(|i| SymbolKey::parse(&format!("NSE|{i}")).unwrap())
            .collect();
        let plan = md.set_watch(keys);
        assert!(plan.truncated);
        assert_eq!(plan.rejected, vec!["NSE|3", "NSE|4"]);
        assert_eq!(md.status(Instant::now()).watched, 2);
    }

    #[test]
    fn delta_batches_are_bounded_and_carry_the_remainder() {
        let mut md = market();
        let keys: Vec<SymbolKey> = (1..=10)
            .map(|i| SymbolKey::parse(&format!("NSE|{i}")).unwrap())
            .collect();
        md.set_watch(keys.clone());
        for key in &keys {
            let raw = format!(
                r#"{{"t":"tk","e":"{}","tk":"{}","lp":"10.00"}}"#,
                key.exchange(),
                key.token()
            );
            md.apply_raw(raw.as_bytes(), 0).unwrap();
        }

        let now = Instant::now();
        let first = md.flush_now(now, 0).expect("batch");
        assert!(first.quotes.len() <= DEFAULT_MAX_PER_FRAME);
        assert_eq!(first.pending, 10 - first.quotes.len());
        let mut total = first.quotes.len();
        while let Some(batch) = md.flush_now(now, 0) {
            total += batch.quotes.len();
        }
        assert_eq!(total, 10, "every quote must eventually be emitted");
    }

    #[test]
    fn status_counts_frames_and_decode_errors() {
        let mut md = market();
        apply(&mut md, br#"{"t":"tk","e":"NSE","tk":"1","lp":"10.00"}"#);
        let bad = md.apply_raw(b"{not json", 0);
        assert!(bad.is_err());

        let status = md.status(Instant::now());
        assert_eq!(status.frames_total, 1);
        assert_eq!(status.frames_error, 1);
        assert!(status.last_error.is_some());
        assert_eq!(status.backend, "in-memory");
        assert!(!status.master_loaded);
        assert_eq!(status.source, SourceKind::Idle);
    }

    #[test]
    fn search_requires_a_loaded_master() {
        let md = market();
        assert!(md.search("RELIANCE", 10).is_empty());
        assert!(md.instrument("NSE|2885").is_none());
    }

    #[test]
    fn search_and_lookup_work_against_the_master() {
        let mut md = market();
        md.set_master(master());
        md.set_backend("mongo");

        let hits = md.search("RELIANCE", 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "NSE|2885");
        assert_eq!(hits[0].strike_type, "XX");

        let inst = md.instrument("NFO|158614").unwrap();
        assert_eq!(inst.strike, "1680.00");
        assert_eq!(inst.strike_type, "PE");
        assert_eq!(inst.lot_size, 450);

        let status = md.status(Instant::now());
        assert!(status.master_loaded);
        assert_eq!(status.instruments, 2);
        assert_eq!(status.backend, "mongo");
    }

    #[test]
    fn source_transitions_track_connectivity() {
        let mut md = market();
        assert!(!md.status(Instant::now()).connected);
        md.set_source(SourceKind::Replay);
        let s = md.status(Instant::now());
        assert!(s.connected);
        assert_eq!(s.source, SourceKind::Replay);
        md.set_source(SourceKind::Idle);
        assert!(!md.status(Instant::now()).connected);
    }

    #[test]
    fn build_label_is_readable() {
        assert_eq!(build_label("0.2.0", "2026-11-26"), "0.2.0 (2026-11-26)");
    }
}
