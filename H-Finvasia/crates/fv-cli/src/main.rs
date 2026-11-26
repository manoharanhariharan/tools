//! Headless harness.
//!
//! Deliberately has no UI: the point is to prove the ingest, merge, pacing and
//! cache layers independently of Tauri, and to provide the M1/M2/M5/M7
//! measurements from `fv-features.md` §7.7 in CI.

use clap::{Parser, Subcommand};
#[cfg(feature = "ws")]
use fv_core::SymbolKey;
use fv_engine::Coalescer;

/// Covered by tests; the same file is `include!`d by `build.rs`.
#[cfg(test)]
mod date;
use fv_feed::{parse, replay_into, QuoteBook, Replay};
use std::path::PathBuf;
use std::time::Instant;

/// Reported by `--version`. `scripts/release.ps1` parses the `build YYYY-MM-DD`
/// portion to name the artifact and to stamp `BUILD.txt`.
const BUILD_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " build ", env!("FV_BUILD_DATE"));

#[derive(Parser, Debug)]
#[command(
    name = "h-finvasia",
    version = BUILD_VERSION,
    about = "H-Finvasia harness (replay / bench / master / doctor / feed)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Replay a captured JSONL feed and print merge statistics.
    Replay {
        file: PathBuf,
        /// UI refresh rate used to compute coalesced batches.
        #[arg(long, default_value_t = 30)]
        hz: u32,
        /// Number of instruments to print after replay.
        #[arg(long, default_value_t = 5)]
        show: usize,
    },
    /// Measure decode throughput (the M7 JSON-budget measurement).
    Bench {
        file: PathBuf,
        #[arg(long, default_value_t = 200)]
        iters: u32,
    },
    /// Query the local instrument master (requires `--features mongo` + a DB).
    Master {
        #[command(subcommand)]
        command: MasterCommand,
    },
    /// Check platform, build features and local data sources.
    Doctor,
    /// Connect to the live feed and print the first frames (requires credentials).
    #[cfg(feature = "ws")]
    Feed {
        #[arg(long)]
        uid: String,
        #[arg(long)]
        actid: String,
        /// Access token from the OAuth flow. Prefer `FV_ACCESS_TOKEN`.
        #[arg(long, env = "FV_ACCESS_TOKEN", hide_env_values = true)]
        token: String,
        /// Comma-separated `EXCHANGE|TOKEN` list.
        #[arg(long, value_delimiter = ',')]
        scrips: Vec<String>,
        /// Stop after this many frames (0 = run until interrupted).
        #[arg(long, default_value_t = 20)]
        frames: u64,
    },
}

#[derive(Subcommand, Debug)]
enum MasterCommand {
    /// Substring search over trading symbol / underlying / name.
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Option chain for an underlying and expiry.
    Chain {
        underlying: String,
        #[arg(long)]
        expiry: String,
    },
    /// Counts, exchanges and underlyings.
    Stats,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Replay { file, hz, show } => cmd_replay(&file, hz, show)?,
        Command::Bench { file, iters } => cmd_bench(&file, iters)?,
        Command::Master { command } => cmd_master(command).await?,
        Command::Doctor => cmd_doctor().await?,
        #[cfg(feature = "ws")]
        Command::Feed {
            uid,
            actid,
            token,
            scrips,
            frames,
        } => cmd_feed(uid, actid, token, scrips, frames).await?,
    }
    Ok(())
}

fn cmd_replay(file: &PathBuf, hz: u32, show: usize) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let replay = Replay::from_path(file)?;
    let load = started.elapsed();

    let mut book = QuoteBook::new();
    let t0 = Instant::now();
    let stats = replay_into(&mut book, replay.frames(), 0);
    let merge = t0.elapsed();

    let mut coalescer = Coalescer::new(hz, 600);
    coalescer.mark_all(book.keys_sorted());
    let mut batches = 0usize;
    let mut emitted = 0usize;
    let now = Instant::now();
    while let Some(batch) = coalescer.flush_now(now, 0, &book) {
        batches += 1;
        emitted += batch.quotes.len();
    }

    println!("replay {}", file.display());
    println!("  load            {:?}", load);
    println!("  frames          {}", stats.total);
    println!("  parsed          {}", stats.parsed);
    println!(
        "  errors          {} ({:.3} %)",
        stats.errors,
        stats.error_rate() * 100.0
    );
    println!("  snapshots       {}", stats.snapshots);
    println!("  diffs           {}", stats.diffs);
    println!("  orders          {}", stats.orders);
    println!("  acks            {}", stats.acks);
    println!("  unknown         {}", stats.unknown);
    println!("  applied         {}", stats.applied);
    println!("  instruments     {}", stats.instruments);
    println!("  merge time      {:?}", merge);
    println!(
        "  delta batches   {} @ {} Hz ({} quotes)",
        batches, hz, emitted
    );

    for key in book.keys_sorted().into_iter().take(show) {
        if let Some(q) = book.get(&key) {
            println!(
                "  {:<14} ltp={:>12} chg={:>8} vol={:>10} bid={:>10} ask={:>10}",
                q.key.as_wire(),
                q.ltp.map(|p| p.format(2)).unwrap_or_else(|| "-".into()),
                q.change()
                    .map(|p| p.format(2))
                    .unwrap_or_else(|| "-".into()),
                q.volume
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".into()),
                q.best_bid
                    .map(|p| p.format(2))
                    .unwrap_or_else(|| "-".into()),
                q.best_ask
                    .map(|p| p.format(2))
                    .unwrap_or_else(|| "-".into()),
            );
        }
    }
    Ok(())
}

fn cmd_bench(file: &PathBuf, iters: u32) -> Result<(), Box<dyn std::error::Error>> {
    let replay = Replay::from_path(file)?;
    let frames = replay.frames();
    if frames.is_empty() {
        return Err("fixture is empty".into());
    }
    let iters = iters.max(1);

    // Fast-path classification only.
    let t = Instant::now();
    let mut classified = 0u64;
    for _ in 0..iters {
        for raw in frames {
            if fv_feed::peek_kind(raw) != fv_feed::FrameKind::Unknown {
                classified += 1;
            }
        }
    }
    let classify_elapsed = t.elapsed();

    // Full decode.
    let t = Instant::now();
    let mut parsed = 0u64;
    for _ in 0..iters {
        for raw in frames {
            if parse(raw).is_ok() {
                parsed += 1;
            }
        }
    }
    let parse_elapsed = t.elapsed();

    // Decode + merge.
    let t = Instant::now();
    let mut book = QuoteBook::new();
    let mut applied = 0u64;
    for _ in 0..iters {
        for raw in frames {
            if let Ok(frame) = parse(raw) {
                if !matches!(book.apply(&frame, 0), fv_feed::Applied::Ignored) {
                    applied += 1;
                }
            }
        }
    }
    let merge_elapsed = t.elapsed();

    let bytes: usize = frames.iter().map(|f| f.len()).sum();
    let per_iter_bytes = bytes as f64;

    println!("bench {}", file.display());
    println!("  frames/iter     {}", frames.len());
    println!("  bytes/iter      {}", bytes);
    println!();
    report("classify (t-field only)", classified, classify_elapsed);
    report("parse only", parsed, parse_elapsed);
    report("parse + merge", applied, merge_elapsed);
    println!();
    println!(
        "  effective rate  {:.1} MB/s (parse+merge)",
        (per_iter_bytes * iters as f64) / merge_elapsed.as_secs_f64() / 1_000_000.0
    );

    fn report(label: &str, count: u64, elapsed: std::time::Duration) {
        let secs = elapsed.as_secs_f64().max(1e-9);
        println!(
            "  {:<24} {:>10} ops in {:>10.3} ms  => {:>12.0} ops/s  ({:.2} µs/op)",
            label,
            count,
            secs * 1000.0,
            count as f64 / secs,
            secs * 1e6 / count.max(1) as f64
        );
    }
    Ok(())
}

async fn cmd_master(command: MasterCommand) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(feature = "mongo"))]
    {
        let _ = command;
        return Err("rebuild with `--features mongo` to query the instrument master".into());
    }

    #[cfg(feature = "mongo")]
    {
        use fv_master::mongo::{load_master, MasterConfig};

        let cfg = MasterConfig::from_env();
        println!("master {} / {} @ {}", cfg.database, cfg.collection, cfg.uri);
        let started = Instant::now();
        let master = load_master(&cfg).await?;
        println!(
            "  loaded {} instruments in {:?}",
            master.len(),
            started.elapsed()
        );

        match command {
            MasterCommand::Stats => {
                println!("  underlyings     {}", master.underlyings().len());
                for exch in ["NSE", "BSE", "NFO", "BFO", "MCX", "CDS"] {
                    println!("  {:<14} {}", exch, master.on_exchange(exch).len());
                }
                println!("  NIFTY expiries  {:?}", master.expiries("NIFTY"));
            }
            MasterCommand::Search { query, limit, json } => {
                let hits = master.search(&query, limit);
                if json {
                    let rows: Vec<serde_json::Value> = hits
                        .iter()
                        .map(|(i, rank)| {
                            serde_json::json!({
                                "key": i.key.as_wire(),
                                "trading_symbol": i.trading_symbol,
                                "underlying": i.underlying,
                                "expiry": i.expiry,
                                "strike": i.strike.format(2),
                                "strike_type": i.strike_type.as_str(),
                                "lot_size": i.lot_size,
                                "rank": format!("{rank:?}"),
                            })
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&rows)?);
                } else {
                    println!("  {} hits for {:?}", hits.len(), query);
                    for (inst, rank) in hits {
                        println!(
                            "  {:<14} {:<28} {:<12} {:>10} {:<3} lot={:<6} {:?}",
                            inst.key.as_wire(),
                            inst.trading_symbol,
                            inst.underlying.clone().unwrap_or_default(),
                            inst.strike.format(2),
                            inst.strike_type.as_str(),
                            inst.lot_size,
                            rank
                        );
                    }
                }
            }
            MasterCommand::Chain { underlying, expiry } => {
                let chain = master.option_chain(&underlying, &expiry);
                println!("  {} legs for {} {}", chain.len(), underlying, expiry);
                for inst in chain {
                    println!(
                        "  {:<14} {:<28} {:>10} {}",
                        inst.key.as_wire(),
                        inst.trading_symbol,
                        inst.strike.format(2),
                        inst.strike_type.as_str()
                    );
                }
            }
        }
        Ok(())
    }
}

async fn cmd_doctor() -> Result<(), Box<dyn std::error::Error>> {
    println!("h-finvasia doctor");
    println!(
        "  os              {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("  family          {}", std::env::consts::FAMILY);
    println!("  ws feature      {}", cfg!(feature = "ws"));
    println!("  mongo feature   {}", cfg!(feature = "mongo"));

    // Feed layer self-check using the bundled fixture: proves the crate works
    // on this platform even with no network and no database.
    let fixture = include_str!("../../../crates/fv-feed/tests/fixtures/sample_feed.jsonl");
    let replay = Replay::from_jsonl_str(fixture);
    let mut book = QuoteBook::new();
    let stats = replay_into(&mut book, replay.frames(), 0);
    println!(
        "  feed self-test  {} frames, {} instruments, {} errors",
        stats.total,
        book.len(),
        stats.errors
    );
    if stats.errors > 0 {
        return Err("feed fixture failed to replay".into());
    }

    let store = fv_store::Store::from_env().await;
    let persistent = store.is_persistent();
    match store.ping().await {
        Ok(()) => println!("  cache store     {} (reachable)", backend(persistent)),
        Err(e) => println!(
            "  cache store     {} (unreachable: {e})",
            backend(persistent)
        ),
    }
    // Explicit teardown: the Mongo driver must not outlive the runtime.
    store.shutdown().await;

    if let Ok(uri) = std::env::var("FV_MONGO_URI") {
        println!("  FV_MONGO_URI    set");
        let _ = uri;
    } else {
        println!("  FV_MONGO_URI    not set (default mongodb://127.0.0.1:27017)");
    }
    println!(
        "  access token    {}",
        if std::env::var("FV_ACCESS_TOKEN").is_ok() {
            "set"
        } else {
            "not set"
        }
    );
    Ok(())
}

fn backend(persistent: bool) -> &'static str {
    if persistent {
        "mongodb"
    } else {
        "in-memory"
    }
}

#[cfg(feature = "ws")]
async fn cmd_feed(
    uid: String,
    actid: String,
    token: String,
    scrips: Vec<String>,
    frames: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    use fv_feed::ws::{run_session, WsConfig};

    let keys: Vec<SymbolKey> = scrips
        .iter()
        .map(|s| SymbolKey::parse(s))
        .collect::<Result<Vec<_>, _>>()?;
    if keys.is_empty() {
        return Err("pass at least one --scrips NSE|22".into());
    }

    let cfg = WsConfig::new(uid, actid, token);
    let mut seen = 0u64;
    let mut book = QuoteBook::new();
    let mut coalescer = Coalescer::new(30, 600);

    run_session(&cfg, &keys, |frame| {
        if seen < frames || frames == 0 {
            match &frame {
                fv_feed::Frame::Order(o) => {
                    println!(
                        "ORDER {} {} {:?} {:?}",
                        o.noren_order_no.as_deref().unwrap_or("-"),
                        o.status.as_deref().unwrap_or("-"),
                        o.transaction_type,
                        o.price.map(|p| p.format(2))
                    );
                }
                other => {
                    if let fv_feed::Applied::Ignored = book.apply(other, 0) {
                    } else {
                        coalescer.mark_all(book.keys_sorted());
                    }
                }
            }
        }
        seen += 1;
    })
    .await?;

    println!("received {} frames, {} instruments", seen, book.len());
    Ok(())
}
