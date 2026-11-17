//! Composition root: settings, error monitoring and logs, then the CLI over the engine's
//! `Panel`.

// `ev_lib::settings!` expands once per field and recurses; the settings outgrew the default 128.
#![recursion_limit = "256"]

mod settings;

use std::{net::SocketAddr, sync::Arc};

use clap::{Parser, Subcommand};
use ev_lib::error_monitoring;
use eyre::WrapErr;
use panel::{Panel, seal::DataKey, store::Store};
use panel_core::{ids::LocationId, place::Editor};
use panel_server::{
	capture::{self, CaptureApi},
	conductor, google_calendar, http,
	scope_ledger::{self, ScopeLedger},
};
use sa_core::ids::BrandId;

use crate::settings::Settings;

#[derive(Parser)]
#[command(
	name = "sites_panel",
	version,
	about = "The Service-Arb sites: sa.funnel.v1 ingest, the event journal, the funnel projections, places, pricing, experiments, booking and invoices"
)]
struct Cli {
	#[command(subcommand)]
	cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
	/// Create the database at PANEL_DB_PATH if need be and apply the migrations it lacks, then
	/// exit. Every command does as much on open, `serve` included; this one does nothing else.
	Migrate,
	/// The HTTP API.
	Serve {
		/// The dev shell sets PANEL_BIND to this machine's port.
		#[arg(long, env = "PANEL_BIND")]
		bind: SocketAddr,
	},
	/// Rebuild leads, calls and payments from the journal, judging every event against the
	/// registry as it is now.
	RebuildProjections,
	/// A place's live settings, what the sites lay over their baked config.
	#[command(subcommand)]
	Place(PlaceCmd),
	/// Print a fresh PANEL_DATA_KEY.
	GenDataKey,
}

#[derive(Subcommand)]
enum PlaceCmd {
	/// Make a place known to the panel, as adding it on the Locations screen does; nothing
	/// changes for one it knows. Journaled as `by = cli`.
	Register { brand: String, slug: String },
}

// Sentry must be initialised before the async runtime starts, hence no #[tokio::main].
fn main() -> eyre::Result<()> {
	color_eyre::install()?;

	// The deploy contract, straight out of the image: the gitops preflight diffs it with the
	// cluster Secret's keys, so a missing variable is caught before the rollout.
	if let Some(profile) = settings::print_required_vars_for() {
		for var in Settings::required_var_names(&profile) {
			println!("{var}");
		}
		return Ok(());
	}
	if std::env::args().nth(1).as_deref() == Some("--print-provided-paths") {
		for path in http::PROVIDED_PATHS {
			println!("{path}");
		}
		return Ok(());
	}
	// Exits 78 (EX_CONFIG) on a bad environment, before anything else is built.
	let settings = ev_lib::settings::or_exit(Settings::from_env());

	// Held for the life of main: dropping it flushes. A no-op without SENTRY_DSN.
	let _sentry = error_monitoring::init(&error_monitoring::Config {
		dsn: settings.sentry_dsn.clone(),
		environment: settings.app_env.clone(),
		release: error_monitoring::release_name!().map(|r| r.into_owned()),
		// the same name OTEL uses, so an issue and its trace agree on the service
		service: std::env::var("OTEL_SERVICE_NAME").ok().filter(|s| !s.trim().is_empty()),
		traces_sample_rate: error_monitoring::Config::traces_sample_rate_for(&settings.app_env),
	});
	let cli = Cli::parse();
	init_tracing()?;

	tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.wrap_err("building the tokio runtime")?
		.block_on(run(cli, settings))
}

/// Logs go to stderr, so a CLI command's stdout stays its output.
fn init_tracing() -> eyre::Result<()> {
	use tracing_subscriber::{EnvFilter, fmt, prelude::*};

	let filter = EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new(option_env!("LOG_DIRECTIVES").unwrap_or("info")))?;
	tracing_subscriber::registry()
		.with(filter)
		.with(fmt::layer().with_writer(std::io::stderr))
		.with(error_monitoring::tracing_layer())
		.init();
	Ok(())
}

async fn run(cli: Cli, settings: Settings) -> eyre::Result<()> {
	let connect = || async { eyre::Ok(Panel::new(Store::open(settings.db_path()?).await?, settings.data_key()?).with_posthog_project(settings.posthog_project()?)) };
	match cli.cmd {
		Cmd::Migrate => {
			Store::open(settings.db_path()?).await?.pool().close().await;
			Ok(())
		}
		Cmd::Serve { bind } => {
			let keys = settings.assertion_keys()?;
			let locale = settings.locale()?;
			let mfe = settings.mfe_dir()?;
			let capture = settings.capture()?.map(|c| CaptureApi::new(&c.host, &c.key)).transpose()?;
			let google = google(&settings)?;
			let ledger = settings.scope_ledger()?.map(|c| ScopeLedger::connect(&c)).transpose()?.map(Arc::new);
			let panel = connect().await?.with_capture(capture.is_some());
			let peers = Peers { capture, google, ledger };
			serve(panel, keys, locale, peers, mfe, bind).await
		}
		Cmd::RebuildProjections => {
			let r = connect().await?.rebuild_projections().await?;
			println!(
				"{} events: {} registered, {} unregistered, {} invalid; {} leads",
				r.events, r.registered, r.unregistered, r.invalid, r.leads
			);
			Ok(())
		}
		Cmd::Place(cmd) => place(&connect().await?, cmd).await,
		Cmd::GenDataKey => {
			println!("{}", DataKey::generate_hex()?);
			Ok(())
		}
	}
}

async fn place(panel: &Panel, cmd: PlaceCmd) -> eyre::Result<()> {
	let PlaceCmd::Register { brand, slug } = cmd;
	let (brand, slug) = (BrandId::parse(&brand)?, LocationId::parse(&slug)?);
	let (_, added) = panel.register_place(&Editor::Cli, &brand, &slug, jiff::Timestamp::now()).await?;
	if !added {
		eprintln!("{brand}/{slug} was known already: nothing changed");
	}
	Ok(())
}

/// The Google Calendar adapter, how often it pulls, and the owners' consents, when configured.
fn google(settings: &Settings) -> eyre::Result<Option<Google>> {
	let Some(g) = settings.google()? else { return Ok(None) };
	let api = google_calendar::GoogleCalendar::new(google_calendar::TOKEN_URL, google_calendar::API_BASE, &g.client_id, &g.client_secret, Default::default())?;
	Ok(Some(Google {
		consents: Arc::new(google_calendar::Consents::new(api.clone(), g.redirect_uri)),
		api,
		every: jiff::SignedDuration::from_mins(i64::from(g.every_minutes)),
	}))
}

struct Google {
	api: google_calendar::GoogleCalendar,
	every: jiff::SignedDuration,
	consents: Arc<google_calendar::Consents>,
}

/// The services `serve` calls out to, each when configured.
struct Peers {
	/// PostHog (`POSTHOG_*`).
	capture: Option<CaptureApi>,
	/// Google Calendar (`GOOGLE_*`).
	google: Option<Google>,
	/// banking's ScopeLedger (`SCOPE_LEDGER_*`): the invoices.
	ledger: Option<Arc<ScopeLedger>>,
}

/// The HTTP server and the work beside it, all in this one future: the work stops when the
/// server has.
async fn serve(panel: Panel, keys: sa_auth::Keys, locale: sa_core::notify::Locale, peers: Peers, mfe: Option<std::path::PathBuf>, bind: SocketAddr) -> eyre::Result<()> {
	let (stop, stopped) = tokio::sync::watch::channel(false);
	let bus = panel.bus().clone();
	let capture_work = async {
		match peers.capture {
			Some(api) => {
				tracing::info!(?api, "sending lead events to posthog");
				capture::run(panel.clone(), api, stopped.clone()).await;
			}
			None => tracing::warn!("POSTHOG_PROJECT_API_KEY unset: nothing is sent to PostHog"),
		}
	};
	let booking_work = async {
		match &peers.google {
			Some(g) => {
				tracing::info!(minutes = g.every.as_mins(), "google calendar booking pull on, for every brand whose owner consented");
				google_calendar::run(panel.clone(), g.api.clone(), g.every, stopped.clone()).await;
			}
			None => tracing::warn!("GOOGLE_OAUTH_* unset: no Google Calendar bookings pulled, no consent asked"),
		}
	};
	let invoice_work = async {
		match &peers.ledger {
			Some(ledger) => {
				tracing::info!(?ledger, "invoices on: issued through banking's ScopeLedger, payments confirmed from its money events");
				scope_ledger::run(panel.clone(), ledger.clone(), stopped.clone()).await;
			}
			None => tracing::warn!("SCOPE_LEDGER_* unset: no invoices, /api/v1/invoices answers 503"),
		}
	};
	let notify_work = conductor::fan_out(panel.clone(), locale, stopped.clone());
	if mfe.is_none() {
		tracing::warn!("PANEL_MFE_DIR unset: no front end bundle at /mfe");
	}
	let sites = http::Sites {
		panel: panel.clone(),
		keys,
		locale,
		hooks: panel::booking::PushSources::default(),
		mfe,
	};
	let app = http::router(sites, http::Limits::default());
	// The invoice routes find the ledger here; without it they answer 503.
	let app = match peers.ledger.clone() {
		Some(ledger) => app.layer(axum::Extension(ledger)),
		None => app,
	};
	// The consent routes find the owners' consents here; without them they answer 503.
	let app = match &peers.google {
		Some(g) => app.layer(axum::Extension(g.consents.clone())),
		None => app,
	};
	let listener = tokio::net::TcpListener::bind(bind).await.wrap_err_with(|| format!("binding {bind}"))?;
	tracing::info!(%bind, "serving");
	let server = async {
		// The change feed is a response that never ends: it is told to end before the graceful
		// shutdown waits for every response to.
		let shutdown = async move {
			shutdown_signal().await;
			bus.going_away();
		};
		let served = axum::serve(listener, app).with_graceful_shutdown(shutdown).await.wrap_err("HTTP server");
		// Nobody may be listening any more: the work has stopped then.
		let _workers_gone = stop.send(true);
		served
	};
	let (served, ..) = tokio::join!(server, capture_work, booking_work, invoice_work, notify_work);
	served
}

async fn shutdown_signal() {
	let ctrl_c = async {
		if let Err(e) = tokio::signal::ctrl_c().await {
			tracing::error!(error = %e, "listening for Ctrl-C");
			std::future::pending::<()>().await;
		}
	};
	#[cfg(unix)]
	let term = async {
		match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
			Ok(mut s) => {
				s.recv().await;
			}
			Err(e) => {
				tracing::error!(error = %e, "listening for SIGTERM");
				std::future::pending::<()>().await;
			}
		}
	};
	#[cfg(not(unix))]
	let term = std::future::pending::<()>();
	tokio::select! {
		() = ctrl_c => {}
		() = term => {}
	}
	tracing::info!("shutting down");
}
