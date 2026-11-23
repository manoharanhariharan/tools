use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use daybook::config::Config;
use daybook::db;
use daybook::{AppState, build_router};

#[derive(Debug, Parser)]
#[command(
    name = "daybook",
    version,
    about = "In-house daily employee earnings ledger",
    subcommand_required = true,
    arg_required_else_help = true
)]
struct Cli {
    /// Directory for the SQLite database and server data.
    #[arg(long, global = true, env = "DAYBOOK_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the HTTP server (LAN + mobile clients).
    Serve {
        /// Listen address, e.g. 0.0.0.0:8787.
        #[arg(long, env = "DAYBOOK_BIND")]
        bind: Option<String>,
    },
    /// Administrative tasks.
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
    /// TLS certificates for HTTPS on the LAN (required for PWA install).
    Cert {
        #[command(subcommand)]
        command: CertCommand,
    },
    /// Print resolved configuration and environment health.
    Doctor,
}

#[derive(Debug, Subcommand)]
enum AdminCommand {
    /// Set (or reset) the admin password. All sessions are revoked.
    SetPassword,
}

#[derive(Debug, Subcommand)]
enum CertCommand {
    /// Generate a local CA and a server certificate.
    Init {
        /// Host name or IP address the phone will use, repeatable or comma-separated.
        #[arg(long = "host", value_delimiter = ',')]
        hosts: Vec<String>,
        /// Directory for the generated PEM files.
        #[arg(long, default_value = "certs")]
        out: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("daybook=info,tower_http=info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Serve { bind } => serve(bind, cli.data_dir).await,
        Command::Admin { command } => match command {
            AdminCommand::SetPassword => set_password(cli.data_dir).await,
        },
        Command::Cert { command } => match command {
            CertCommand::Init { hosts, out } => init_certs(hosts, out),
        },
        Command::Doctor => doctor(cli.data_dir).await,
    }
}

async fn serve(bind: Option<String>, data_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let config = Arc::new(Config::resolve(bind, data_dir)?);
    config.ensure_data_dir()?;
    let init = db::open(&config.data_dir).await?;

    if let Some(password) = &init.generated_password {
        println!();
        println!("==========================================================");
        println!("  Daybook first-run admin password (shown once, save it):");
        println!("      {password}");
        println!("==========================================================");
        println!();
    }

    let pool = init.pool.clone();
    let state = AppState::new(pool.clone(), Arc::clone(&config));
    let app = build_router(state);
    let cleanup = db::spawn_session_cleanup(pool.clone());

    tracing::info!(
        address = %config.bind,
        tls = config.is_tls(),
        data_dir = %config.data_dir.display(),
        "daybook listening"
    );
    if !config.is_tls() {
        tracing::warn!(
            "serving plain HTTP; browser service-worker install requires HTTPS on a LAN address \
             (run `daybook cert init` and set DAYBOOK_TLS_CERT / DAYBOOK_TLS_KEY)"
        );
    }

    let result = if let Some(tls) = &config.tls {
        let tls_config =
            axum_server::tls_rustls::RustlsConfig::from_pem_file(&tls.cert, &tls.key)
                .await
                .with_context(|| {
                    format!(
                        "cannot load TLS certificate {} / key {}",
                        tls.cert.display(),
                        tls.key.display()
                    )
                })?;

        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            shutdown_handle.graceful_shutdown(Some(Duration::from_secs(10)));
        });

        axum_server::bind_rustls(config.bind, tls_config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map(|_| ())
            .map_err(|err| anyhow::anyhow!("TLS server error: {err}"))
    } else {
        let listener = tokio::net::TcpListener::bind(config.bind)
            .await
            .with_context(|| format!("cannot bind {}", config.bind))?;
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|err| anyhow::anyhow!("server error: {err}"))
    };

    cleanup.abort();
    pool.close().await;
    result?;
    tracing::info!("daybook stopped");
    Ok(())
}

async fn set_password(data_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let config = Config::resolve(None, data_dir)?;
    config.ensure_data_dir()?;
    let init = db::open(&config.data_dir).await?;

    let password = rpassword::prompt_password("New admin password: ")?;
    let confirm = rpassword::prompt_password("Confirm password: ")?;
    if password != confirm {
        anyhow::bail!("passwords do not match");
    }
    db::set_admin_password(&init.pool, &password).await?;
    init.pool.close().await;
    println!("Admin password updated. All existing sessions were revoked.");
    Ok(())
}

fn init_certs(hosts: Vec<String>, out: PathBuf) -> anyhow::Result<()> {
    let mut hosts = hosts;
    if hosts.is_empty() {
        hosts.push("localhost".to_string());
        if let Some(ip) = daybook::cert::detect_lan_ipv4() {
            hosts.push(ip.to_string());
        }
        let hostname = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .ok();
        if let Some(name) = hostname {
            let name = name.to_lowercase();
            if !name.is_empty() {
                hosts.push(name.clone());
                hosts.push(format!("{name}.local"));
            }
        }
    }

    let generated = daybook::cert::generate(&out, &hosts)?;

    println!("Generated certificates");
    println!("  CA certificate     : {}", generated.ca_cert.display());
    println!("  Server certificate : {}", generated.cert.display());
    println!("  Server private key : {}", generated.key.display());
    println!("  Hosts (SANs)       : {}", generated.hosts.join(", "));
    println!();
    println!("Next steps");
    println!("  1. Install the CA certificate on each phone (never the private key):");
    println!("       Android: Settings > Security > Encryption & credentials > Install a certificate > CA certificate");
    println!("       iPhone : open the .crt, install the profile, then Settings > General > About >");
    println!("                Certificate Trust Settings > enable full trust for 'Daybook Local CA'");
    println!("  2. Start the server over HTTPS:");
    println!("       PowerShell: $env:DAYBOOK_TLS_CERT='{}'; $env:DAYBOOK_TLS_KEY='{}'; daybook serve", generated.cert.display(), generated.key.display());
    println!("  3. Open https://<one-of-the-hosts>:8787 on the phone and use 'Add to Home Screen'.");
    Ok(())
}

async fn doctor(data_dir: Option<PathBuf>) -> anyhow::Result<()> {    let config = Config::resolve(None, data_dir)?;
    println!("Daybook doctor");
    println!("  data directory : {}", config.data_dir.display());
    println!("  database       : {}", config.db_path().display());
    println!("  listen address : {}", config.bind);
    println!(
        "  TLS            : {}",
        if config.is_tls() { "enabled" } else { "disabled" }
    );
    println!("  web assets     : {}", if config.web_dir.is_some() { "from disk (dev)" } else { "embedded" });

    match daybook::reports::pdf::PdfFont::discover(config.pdf_font.as_deref()) {
        Ok(font) => println!(
            "  PDF font       : {} (₹ glyph: {})",
            font.path.display(),
            if font.has_rupee_glyph { "yes" } else { "no, using Rs" }
        ),
        Err(err) => println!("  PDF font       : unavailable ({err})"),
    }

    if config.db_path().exists() {
        let init = db::open(&config.data_dir).await?;
        let employees: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM employees WHERE deleted_at IS NULL")
                .fetch_one(&init.pool)
                .await?;
        let entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE deleted_at IS NULL")
            .fetch_one(&init.pool)
            .await?;
        let seq = db::current_seq(&init.pool).await?;
        println!("  employees      : {employees}");
        println!("  entries        : {entries}");
        println!("  sync sequence  : {seq}");
        init.pool.close().await;
    } else {
        println!("  database       : not initialised yet (run `daybook serve` once)");
    }
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
