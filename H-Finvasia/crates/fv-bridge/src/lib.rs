//! UI-facing bridge between the market engine and the desktop shell.
//!
//! This crate intentionally has **no Tauri dependency**: `src-tauri` is a thin
//! adapter, and everything here is exercised by plain unit tests.

pub mod dto;
pub mod market;

pub use dto::{
    ChainRowDto, ContainerCardDto, ContainerRowDto, DeltaDto, IndexRowDto, InstrumentDto, PlanDto,
    PriceDto, QuoteDto, SourceKind, StatusDto, StockRowDto, Trend,
};
pub use market::{
    build_label, chain_window, MarketData, DEFAULT_CHAIN_LIMIT, DEFAULT_MAX_PER_FRAME,
    DEFAULT_WATCH_LIMIT, MAX_TOTAL_SUBSCRIPTIONS,
};

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
    fn watch_rows_keep_metadata_before_the_first_tick() {
        let mut md = market();
        md.set_master(master());
        md.set_watch(vec![SymbolKey::parse("NSE|2885").unwrap()]);

        let rows = md.watch_quotes();
        assert_eq!(rows.len(), 1, "a watched key always yields a row");
        let row = &rows[0];
        assert_eq!(row.trading_symbol.as_deref(), Some("RELIANCE-EQ"));
        assert!(row.ltp.is_none(), "no price has arrived yet");
        assert_eq!(row.underlying.as_deref(), Some("RELIANCE"));
        assert!(!row.has_fno, "RELIANCE has no legs in this fixture");
        assert!(!row.is_index);

        // The same row, once the underlying has derivatives.
        let mut md = market();
        md.set_master(master());
        md.set_watch(vec![SymbolKey::parse("NFO|158614").unwrap()]);
        let row = &md.watch_quotes()[0];
        assert!(row.ltp.is_none());
        assert_eq!(row.underlying.as_deref(), Some("ZYDUSLIFE"));
        assert!(row.has_fno);
    }

    #[test]
    fn open_interest_survives_the_dto_boundary() {
        let mut md = market();
        apply(
            &mut md,
            br#"{"t":"tk","e":"NFO","tk":"158614","lp":"12.35","v":"15230","oi":"1875","poi":"1620"}"#,
        );
        let q = md.snapshot("NFO|158614").expect("quote exists");
        assert_eq!(q.oi, Some(1875));
        assert_eq!(q.prev_oi, Some(1620));
    }

    #[test]
    fn stock_rows_cover_nse_cash_equities_and_carry_watch_state() {
        let mut md = market();
        md.set_master(master());
        let reliance = SymbolKey::parse("NSE|2885").unwrap();
        md.set_watch(vec![reliance.clone()]);
        apply(
            &mut md,
            br#"{"t":"tk","e":"NSE","tk":"2885","ts":"RELIANCE-EQ","lp":"1287.30","v":"980515"}"#,
        );

        let rows = md.stock_rows();
        assert_eq!(rows.len(), 1, "only the NSE cash equity qualifies");
        assert_eq!(rows[0].key, "NSE|2885");
        assert_eq!(rows[0].symbol, "RELIANCE");
        assert!(rows[0].live);
        assert_eq!(
            rows[0].quote.as_ref().unwrap().ltp.as_ref().unwrap().text,
            "1287.30"
        );

        md.remove_from_watch("NSE|2885");
        let rows = md.stock_rows();
        assert!(!rows[0].live);
        assert!(
            rows[0].quote.is_none(),
            "the quote is evicted with the watch"
        );
    }

    #[test]
    fn watched_stocks_and_search_stay_inside_the_cash_list() {
        let mut md = market();
        md.set_master(master());
        md.set_watch(vec![
            SymbolKey::parse("NSE|2885").unwrap(),
            SymbolKey::parse("NFO|158614").unwrap(),
        ]);

        let watched = md.watched_stock_rows();
        assert_eq!(watched.len(), 1, "the option is not a stock row");
        assert_eq!(watched[0].symbol, "RELIANCE");
        assert!(watched[0].live);

        let hits = md.search_stocks("reliance", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "NSE|2885");
        assert!(md.search_stocks("zyduslife", 10).is_empty());
        assert!(md.search_stocks("  ", 10).is_empty());
    }

    #[test]
    fn container_cards_keep_options_and_flag_live_members() {
        let mut md = market();
        md.set_master(master());
        md.set_watch(vec![SymbolKey::parse("NSE|2885").unwrap()]);
        apply(
            &mut md,
            br#"{"t":"tk","e":"NSE","tk":"2885","ts":"RELIANCE-EQ","lp":"1287.30","v":"980515"}"#,
        );

        let card = md.container_card(
            "Mixed",
            Some("#12AB34".into()),
            &["NSE|2885".into(), "NFO|158614".into(), "NSE|999999".into()],
            vec!["NFO|158614".into(), "NSE|2885".into()],
        );
        assert_eq!(card.custom_order, vec!["NFO|158614", "NSE|2885"]);

        assert_eq!(card.name, "Mixed");
        assert_eq!(card.count, 3, "the unresolved key still counts");
        assert_eq!(card.rows.len(), 2, "unresolved keys are omitted from rows");
        assert_eq!(card.rows[0].symbol, "RELIANCE");
        assert!(card.rows[0].live);
        assert_eq!(
            card.rows[0]
                .quote
                .as_ref()
                .unwrap()
                .ltp
                .as_ref()
                .unwrap()
                .text,
            "1287.30"
        );
        assert_eq!(card.rows[1].symbol, "ZYDUSLIFE23FEB27P1680");
        assert_eq!(card.rows[1].strike_type, "PE");
        assert_eq!(card.rows[1].lot_size, 450);
        assert!(!card.rows[1].live);
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
    fn chain_subscriptions_are_separate_from_the_watch_list() {
        let mut md = market();
        let watched = SymbolKey::parse("NSE|1").unwrap();
        let leg_a = SymbolKey::parse("NFO|100").unwrap();
        let leg_b = SymbolKey::parse("NFO|101").unwrap();

        md.set_watch(vec![watched.clone()]);
        let plan = md.set_chain(vec![leg_a.clone(), leg_b.clone()]);
        assert_eq!(plan.subscribe, vec!["NFO|100", "NFO|101"]);
        assert!(plan.unsubscribe.is_empty());
        assert_eq!(
            md.watched(),
            vec!["NSE|1"],
            "chain must not join the watch list"
        );
        assert_eq!(md.chain_len(), 2);
    }

    #[test]
    fn overlapping_key_is_not_unsubscribed_by_one_group() {
        let mut md = market();
        let shared = SymbolKey::parse("NSE|1").unwrap();
        md.set_watch(vec![shared.clone()]);
        md.set_chain(vec![shared.clone()]);

        // Dropping it from the watch list must keep the chain subscription.
        let plan = md.remove_from_watch("NSE|1");
        assert!(plan.unsubscribe.is_empty(), "still needed by the chain");
        assert_eq!(md.chain_len(), 1);

        // Removing it from the chain now unsubscribes for real.
        let plan = md.clear_chain();
        assert_eq!(plan.unsubscribe, vec!["NSE|1"]);
    }

    #[test]
    fn chain_rows_carry_metadata_and_quotes() {
        let mut md = market();
        md.set_master(master());
        let key = SymbolKey::parse("NFO|158614").unwrap();
        md.set_chain(vec![key.clone()]);
        apply(
            &mut md,
            br#"{"t":"tk","e":"NFO","tk":"158614","lp":"12.35","bp1":"12.30","sp1":"12.40"}"#,
        );

        let instrument = md.instrument("NFO|158614").unwrap();
        let _ = instrument;
        let master = md.master().unwrap();
        let raw = master.get(&key).unwrap();
        let row = md.chain_row(raw);
        assert_eq!(row.key, "NFO|158614");
        assert_eq!(row.strike, "1680.00");
        assert_eq!(row.strike_type, "PE");
        assert_eq!(
            row.quote.as_ref().unwrap().ltp.as_ref().unwrap().text,
            "12.35"
        );
    }

    #[test]
    fn chain_rows_are_returned_before_the_first_tick() {
        let mut md = market();
        md.set_master(master());
        let key = SymbolKey::parse("NFO|158614").unwrap();
        md.set_chain(vec![key]);
        let raw = md
            .master()
            .unwrap()
            .get(&SymbolKey::parse("NFO|158614").unwrap())
            .unwrap();
        let row = md.chain_row(raw);
        assert!(row.quote.is_none(), "no quote yet");
        assert_eq!(row.symbol, "ZYDUSLIFE23FEB27P1680");
    }

    #[test]
    fn chain_window_subscribes_only_the_strikes_around_the_money() {
        use fv_core::Price;
        use fv_master::{Instrument, StrikeType};

        let legs: Vec<Instrument> = [25000, 25500, 25600, 25700, 25800, 25900, 26000, 26500]
            .iter()
            .flat_map(|strike| {
                ["CE", "PE"].iter().map(move |side| Instrument {
                    key: SymbolKey::parse(&format!(
                        "NFO|{}{}",
                        strike,
                        if *side == "CE" { 1 } else { 2 }
                    ))
                    .unwrap(),
                    trading_symbol: format!("NIFTY{strike}{side}"),
                    name: Some("NIFTY".into()),
                    underlying: Some("NIFTY".into()),
                    expiry: Some("2026-12-01".into()),
                    strike: Price::parse(&strike.to_string()).unwrap(),
                    strike_type: if *side == "CE" {
                        StrikeType::Call
                    } else {
                        StrikeType::Put
                    },
                    lot_size: 65,
                    instrument_type: side.to_string(),
                    segment: Some("NFO-OPT".into()),
                    tick_size: None,
                })
            })
            .collect();

        let window = chain_window(&legs, 1, Some(Price::parse("25800").unwrap()));
        // 3 strikes (25700, 25800, 25900) x 2 sides.
        assert_eq!(window.len(), 6);
        assert!(window.iter().any(|k| k.token() == "258001"));
        assert!(window.iter().any(|k| k.token() == "258002"));
        assert!(
            !window.iter().any(|k| k.token() == "250001"),
            "far strike excluded"
        );

        // Without an ATM the window centres on the middle of the chain.
        let centred = chain_window(&legs, 0, None);
        assert_eq!(centred.len(), 2, "one strike, both sides");
    }

    #[test]
    fn subscription_budget_is_never_exceeded() {
        let mut md = market();
        let watch: Vec<SymbolKey> = (0..95)
            .map(|i| SymbolKey::parse(&format!("NSE|{i}")).unwrap())
            .collect();
        md.set_watch(watch);

        let chain: Vec<SymbolKey> = (0..20)
            .map(|i| SymbolKey::parse(&format!("NFO|{i}")).unwrap())
            .collect();
        let plan = md.set_chain(chain);

        assert!(plan.truncated, "overflow must be reported");
        assert_eq!(plan.rejected.len(), 15);
        assert_eq!(md.subscribed_len(), MAX_TOTAL_SUBSCRIPTIONS);
        assert_eq!(md.chain_len(), 5);
    }

    #[test]
    fn growing_the_watch_list_trims_the_chain() {
        let mut md = market();
        let chain: Vec<SymbolKey> = (0..10)
            .map(|i| SymbolKey::parse(&format!("NFO|{i}")).unwrap())
            .collect();
        md.set_chain(chain);

        let watch: Vec<SymbolKey> = (0..96)
            .map(|i| SymbolKey::parse(&format!("NSE|{i}")).unwrap())
            .collect();
        let plan = md.set_watch(watch);

        assert_eq!(md.chain_len(), 4, "chain yields to the watch list");
        assert!(
            !plan.unsubscribe.is_empty(),
            "trimmed legs are unsubscribed"
        );
        assert_eq!(md.subscribed_len(), MAX_TOTAL_SUBSCRIPTIONS);
    }

    #[test]
    fn status_reports_the_subscription_budget() {
        let mut md = market();
        md.set_watch(vec![SymbolKey::parse("NSE|1").unwrap()]);
        let status = md.status(Instant::now());
        assert_eq!(status.subscribed, 1);
        assert_eq!(status.subscription_limit, MAX_TOTAL_SUBSCRIPTIONS);
    }

    #[test]
    fn build_label_is_readable() {
        assert_eq!(build_label("0.2.0", "2026-11-26"), "0.2.0 (2026-11-26)");
    }
}
