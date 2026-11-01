//! The environment: secrets and the deployment profile. Where to listen is a flag.

use panel::seal::DataKey;
use panel_core::notify::Locale;

ev_lib::settings! {
	/// Each secret is needed only by what uses it; a missing one fails that, with an error
	/// naming it, rather than the boot — except in production, where the only thing this
	/// binary runs is `serve`, which needs them.
	pub struct Settings {
		/// The SQLite file, e.g. `/data/sites_panel.db` (the image's, on the pod's volume,
		/// replicated by litestream). Created if missing and migrated on open, by whichever
		/// command opens it. A path, not a secret.
		#[required_in("production")]
		panel_db_path: Option<String>,
		/// 64 hex characters: the key PII and the notes are sealed with (`sites_panel
		/// gen-data-key` makes one). Losing it loses them; changing it strands them.
		#[secret]
		#[required_in("production")]
		panel_data_key: Option<String>,
		/// Unset: errors are logged, not reported.
		#[secret]
		sentry_dsn: Option<String>,
		/// The conductor's public keys its assertions are checked with, `<kid>:<base64 key>`
		/// comma-separated (several while a key rotates). `serve` refuses to start without them.
		#[required_in("production")]
		panel_assertion_keys: Option<String>,
		/// The language of the notes the conductor's Telegram sends: `ru` or `en`.
		telegram_locale: String = "ru",
		/// The front end's bundle (`frontend/` built, its `dist/`), served at `/mfe/*`; the image
		/// sets it. Unset: `serve` answers the API alone, and says so.
		panel_mfe_dir: Option<String>,
		/// The project key the landings send to PostHog with (`phc_…`, public — not a secret):
		/// `serve` sends each lead's life after the form there. Unset: nothing is sent, and
		/// `serve` says so.
		posthog_project_api_key: Option<String>,
		/// PostHog's capture host, the landings' too.
		posthog_host: String = "https://us.i.posthog.com",
		/// The PostHog project the landings send to (Service-Arb's, a number like 614067): the
		/// experiments link to their funnels in it. Unset: no links.
		posthog_project_id: Option<String>,
		/// The PostHog app's origin, for those links (not the capture host).
		posthog_app_host: String = "https://us.posthog.com",
		/// The Google OAuth client (Desktop kind), for the `google_calendar` booking adapter and
		/// `sites_panel booking google-authorize`. Unset, with the secret below: no Google
		/// Calendar pull. Each brand's calendar is `GOOGLE_CALENDAR_REFRESH_TOKEN_<BRAND>`
		/// (secret) and `GOOGLE_CALENDAR_ID_<BRAND>` (optional, `primary`), read apart: their
		/// names follow the brands.
		google_oauth_client_id: Option<String>,
		#[secret]
		google_oauth_client_secret: Option<String>,
		/// How often each brand's calendar is pulled, in minutes.
		google_calendar_sync_minutes: u32 = "5",
		/// banking's ScopeLedger listener, from inside the cluster, e.g.
		/// `https://banking-scope-ledger:50054`: the invoices are issued there. With the three
		/// files below, or none of them; unset, the invoice routes answer 503.
		scope_ledger_addr: Option<String>,
		/// Paths, not secrets: the CA banking's server certificate chains to, and the client
		/// certificate and key, mounted from the cluster's Secret.
		scope_ledger_tls_ca_pem_file: Option<String>,
		scope_ledger_tls_cert_pem_file: Option<String>,
		scope_ledger_tls_key_pem_file: Option<String>,
		app_env: String = "development",
	}
}

impl Settings {
	pub fn db_path(&self) -> eyre::Result<&std::path::Path> {
		let path = self.panel_db_path.as_deref().map(str::trim).filter(|p| !p.is_empty());
		Ok(std::path::Path::new(path.ok_or_else(|| eyre::eyre!("PANEL_DB_PATH must be set"))?))
	}

	pub fn data_key(&self) -> eyre::Result<DataKey> {
		let hex = self.panel_data_key.as_deref().ok_or_else(|| eyre::eyre!("PANEL_DATA_KEY must be set"))?;
		Ok(DataKey::from_hex(hex)?)
	}
}

impl Settings {
	/// `PANEL_ASSERTION_KEYS`: without them nothing but the open routes could answer.
	pub fn assertion_keys(&self) -> eyre::Result<sa_auth::Keys> {
		let raw = self
			.panel_assertion_keys
			.as_deref()
			.ok_or_else(|| eyre::eyre!("PANEL_ASSERTION_KEYS must be set: the conductor's public keys"))?;
		raw.parse().map_err(|e| eyre::eyre!("PANEL_ASSERTION_KEYS: {e}"))
	}

	pub fn locale(&self) -> eyre::Result<Locale> {
		Ok(self.telegram_locale.parse()?)
	}

	/// The front end's bundle, when `PANEL_MFE_DIR` names it: a directory with its entry in it.
	pub fn mfe_dir(&self) -> eyre::Result<Option<std::path::PathBuf>> {
		let Some(dir) = self.panel_mfe_dir.as_deref().map(str::trim).filter(|d| !d.is_empty()) else {
			return Ok(None);
		};
		let dir = std::path::PathBuf::from(dir);
		eyre::ensure!(dir.join("mfe-sites.js").is_file(), "PANEL_MFE_DIR {} holds no mfe-sites.js", dir.display());
		Ok(Some(dir))
	}
}

/// Sending events to PostHog, when it is configured.
#[derive(Debug)]
pub struct CaptureSettings {
	pub host: String,
	pub key: String,
}

impl Settings {
	/// `None` without `POSTHOG_PROJECT_API_KEY`: nothing is sent.
	pub fn capture(&self) -> eyre::Result<Option<CaptureSettings>> {
		let Some(key) = self.posthog_project_api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) else {
			return Ok(None);
		};
		// A personal key (`phx_`) here would be a secret in the wrong place, and one PostHog
		// refuses for capture anyway.
		eyre::ensure!(
			key.starts_with("phc_") && key.len() <= 128 && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
			"POSTHOG_PROJECT_API_KEY is a project key, phc_…"
		);
		Ok(Some(CaptureSettings {
			host: origin("POSTHOG_HOST", &self.posthog_host, &self.app_env)?,
			key: key.to_owned(),
		}))
	}

	/// The PostHog project the experiments link into; `None` without `POSTHOG_PROJECT_ID`.
	pub fn posthog_project(&self) -> eyre::Result<Option<panel::posthog::PosthogProject>> {
		let Some(project) = self.posthog_project_id.as_deref().map(str::trim).filter(|p| !p.is_empty()) else {
			return Ok(None);
		};
		eyre::ensure!(project.len() <= 32 && project.bytes().all(|b| b.is_ascii_digit()), "POSTHOG_PROJECT_ID is a number, e.g. 614067");
		Ok(Some(panel::posthog::PosthogProject {
			app_host: origin("POSTHOG_APP_HOST", &self.posthog_app_host, &self.app_env)?,
			project_id: project.to_owned(),
		}))
	}
}

/// An origin alone, `https` in production; without its trailing slash.
fn origin(var: &str, raw: &str, app_env: &str) -> eyre::Result<String> {
	let raw = raw.trim().trim_end_matches('/');
	let url = url::Url::parse(raw).map_err(|e| eyre::eyre!("{var} is not a URL: {e}"))?;
	eyre::ensure!(url.scheme() == "https" || app_env != "production", "{var} must be https in production");
	eyre::ensure!(
		matches!(url.scheme(), "http" | "https") && url.path() == "/" && url.query().is_none() && url.fragment().is_none(),
		"{var} is an origin alone, e.g. https://us.posthog.com"
	);
	Ok(raw.to_owned())
}

/// The prefix of a brand's Google Calendar refresh token: `…_VIFNET`.
pub const GOOGLE_REFRESH_PREFIX: &str = "GOOGLE_CALENDAR_REFRESH_TOKEN_";
/// The prefix of a brand's calendar id.
pub const GOOGLE_CALENDAR_ID_PREFIX: &str = "GOOGLE_CALENDAR_ID_";

/// The Google Calendar adapter, when it is configured.
pub struct GoogleSettings {
	pub client_id: String,
	pub client_secret: String,
	pub every_minutes: u32,
	/// Each brand with a refresh token, and its calendar.
	pub calendars: std::collections::BTreeMap<panel_core::ids::BrandId, panel_server::google_calendar::BrandCalendar>,
}

impl std::fmt::Debug for GoogleSettings {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("GoogleSettings")
			.field("client_id", &self.client_id)
			.field("every_minutes", &self.every_minutes)
			.field("calendars", &self.calendars)
			.finish_non_exhaustive()
	}
}

/// A brand from the end of a variable's name: `VIFNET` → `vifnet`. A brand whose id has a `-`
/// cannot be named so (none has).
fn brand_of_suffix(suffix: &str) -> eyre::Result<panel_core::ids::BrandId> {
	panel_core::ids::BrandId::parse(&suffix.to_ascii_lowercase()).map_err(|_| eyre::eyre!("{GOOGLE_REFRESH_PREFIX}{suffix}: {suffix} does not name a brand"))
}

impl Settings {
	/// The OAuth client alone, for the CLI's consent: both or neither.
	pub fn google_client(&self) -> eyre::Result<Option<(String, String)>> {
		let set = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
		match (set(&self.google_oauth_client_id), set(&self.google_oauth_client_secret)) {
			(None, None) => Ok(None),
			(Some(id), Some(secret)) => Ok(Some((id, secret))),
			(Some(_), None) => eyre::bail!("GOOGLE_OAUTH_CLIENT_ID needs GOOGLE_OAUTH_CLIENT_SECRET too"),
			(None, Some(_)) => eyre::bail!("GOOGLE_OAUTH_CLIENT_SECRET needs GOOGLE_OAUTH_CLIENT_ID too"),
		}
	}

	/// `None` without the OAuth client or without any brand's token: the pull is off. `vars`:
	/// the environment's variables (the brands' are read from their names).
	pub fn google(&self, vars: impl IntoIterator<Item = (String, String)>) -> eyre::Result<Option<GoogleSettings>> {
		let vars: Vec<(String, String)> = vars.into_iter().filter(|(_, v)| !v.trim().is_empty()).collect();
		let mut calendars = std::collections::BTreeMap::new();
		for (name, token) in &vars {
			let Some(suffix) = name.strip_prefix(GOOGLE_REFRESH_PREFIX) else { continue };
			let brand = brand_of_suffix(suffix)?;
			let id_var = format!("{GOOGLE_CALENDAR_ID_PREFIX}{suffix}");
			let calendar_id = vars.iter().find(|(k, _)| *k == id_var).map_or_else(|| "primary".to_owned(), |(_, v)| v.trim().to_owned());
			calendars.insert(
				brand,
				panel_server::google_calendar::BrandCalendar {
					refresh_token: zeroize::Zeroizing::new(token.trim().to_owned()),
					calendar_id,
				},
			);
		}
		let client = self.google_client()?;
		match (client, calendars.is_empty()) {
			(None, true) => Ok(None),
			(None, false) => eyre::bail!("a {GOOGLE_REFRESH_PREFIX}<BRAND> is set: GOOGLE_OAUTH_CLIENT_ID and GOOGLE_OAUTH_CLIENT_SECRET are needed too"),
			(Some(_), true) => Ok(None),
			(Some((client_id, client_secret)), false) => {
				eyre::ensure!(self.google_calendar_sync_minutes >= 1, "GOOGLE_CALENDAR_SYNC_MINUTES is 1 or more");
				Ok(Some(GoogleSettings {
					client_id,
					client_secret,
					every_minutes: self.google_calendar_sync_minutes,
					calendars,
				}))
			}
		}
	}
}

impl Settings {
	/// banking's ScopeLedger: `None` when none of its variables is set; some but not all is a
	/// mistake, named. The PEM files are read here, at boot: a file that is missing fails the
	/// boot, not the first invoice.
	pub fn scope_ledger(&self) -> eyre::Result<Option<panel_server::scope_ledger::ScopeLedgerConfig>> {
		self.scope_ledger_from(|path| std::fs::read_to_string(path).map_err(|e| eyre::eyre!("{path}: {e}")))
	}

	fn scope_ledger_from(&self, read: impl Fn(&str) -> eyre::Result<String>) -> eyre::Result<Option<panel_server::scope_ledger::ScopeLedgerConfig>> {
		let set = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
		let vars = [
			("SCOPE_LEDGER_ADDR", set(&self.scope_ledger_addr)),
			("SCOPE_LEDGER_TLS_CA_PEM_FILE", set(&self.scope_ledger_tls_ca_pem_file)),
			("SCOPE_LEDGER_TLS_CERT_PEM_FILE", set(&self.scope_ledger_tls_cert_pem_file)),
			("SCOPE_LEDGER_TLS_KEY_PEM_FILE", set(&self.scope_ledger_tls_key_pem_file)),
		];
		let missing: Vec<&str> = vars.iter().filter(|(_, v)| v.is_none()).map(|(name, _)| *name).collect();
		if missing.len() == vars.len() {
			return Ok(None);
		}
		eyre::ensure!(missing.is_empty(), "the ScopeLedger needs {} too", missing.join(", "));
		let [addr, ca, cert, key] = vars.map(|(_, v)| v.unwrap_or_default());
		// Never plain: the service token rides every call.
		let url = url::Url::parse(&addr).map_err(|e| eyre::eyre!("SCOPE_LEDGER_ADDR is not a URL: {e}"))?;
		eyre::ensure!(
			url.scheme() == "https" && url.host_str().is_some_and(|h| !h.is_empty()) && url.path() == "/" && url.query().is_none(),
			"SCOPE_LEDGER_ADDR must be a bare https:// address, not {addr}"
		);
		let pem = |var: &str, path: &str| -> eyre::Result<String> {
			let text = read(path).map_err(|e| eyre::eyre!("{var}: {e:#}"))?;
			eyre::ensure!(text.contains("-----BEGIN "), "{var}: {path} holds no PEM");
			Ok(text)
		};
		Ok(Some(panel_server::scope_ledger::ScopeLedgerConfig {
			ca_pem: pem("SCOPE_LEDGER_TLS_CA_PEM_FILE", &ca)?,
			client_cert_pem: pem("SCOPE_LEDGER_TLS_CERT_PEM_FILE", &cert)?,
			client_key_pem: zeroize::Zeroizing::new(pem("SCOPE_LEDGER_TLS_KEY_PEM_FILE", &key)?),
			addr,
		}))
	}
}

/// `--print-required-vars[=PROFILE]` (default `production`): the variables a deploy into
/// that profile must provide, one per line, for the gitops preflight. Read before clap,
/// which would otherwise demand a subcommand.
pub fn print_required_vars_for() -> Option<String> {
	const FLAG: &str = "--print-required-vars";
	let mut args = std::env::args().skip(1);
	let arg = args.next()?;
	match arg.split_once('=') {
		Some((FLAG, profile)) => Some(profile.to_owned()),
		Some(_) => None,
		None if arg == FLAG => Some(args.next().unwrap_or_else(|| "production".to_owned())),
		None => None,
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

	/// The env surface is the deploy contract (the image env and the cluster Secret use these
	/// names); a rename here must be deliberate.
	#[test]
	fn env_surface_matches_the_deploy_contract() {
		assert_eq!(
			Settings::var_names(),
			[
				"PANEL_DB_PATH",
				"PANEL_DATA_KEY",
				"SENTRY_DSN",
				"PANEL_ASSERTION_KEYS",
				"TELEGRAM_LOCALE",
				"PANEL_MFE_DIR",
				"POSTHOG_PROJECT_API_KEY",
				"POSTHOG_HOST",
				"POSTHOG_PROJECT_ID",
				"POSTHOG_APP_HOST",
				"GOOGLE_OAUTH_CLIENT_ID",
				"GOOGLE_OAUTH_CLIENT_SECRET",
				"GOOGLE_CALENDAR_SYNC_MINUTES",
				"SCOPE_LEDGER_ADDR",
				"SCOPE_LEDGER_TLS_CA_PEM_FILE",
				"SCOPE_LEDGER_TLS_CERT_PEM_FILE",
				"SCOPE_LEDGER_TLS_KEY_PEM_FILE",
				"APP_ENV"
			]
		);
		assert_eq!(Settings::required_var_names("production"), ["PANEL_DB_PATH", "PANEL_DATA_KEY", "PANEL_ASSERTION_KEYS"]);
		assert!(Settings::required_var_names("development").is_empty());
	}

	fn from(vars: &[(&str, &str)]) -> Result<Settings, ev_lib::settings::SettingsError> {
		let map: HashMap<String, String> = vars.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
		Settings::from_source(|k| map.get(k).cloned())
	}

	#[test]
	fn secrets_are_lazy_and_never_print() {
		let s = from(&[]).unwrap();
		assert_eq!(format!("{:#}", s.data_key().unwrap_err()), "PANEL_DATA_KEY must be set");
		assert!(from(&[("PANEL_DATA_KEY", "short")]).unwrap().data_key().is_err());
		assert!(from(&[("APP_ENV", "production")]).is_err());
		let key = "ab".repeat(32);
		let s = from(&[("PANEL_DATA_KEY", &key), ("PANEL_DB_PATH", "/data/sites_panel.db")]).unwrap();
		assert!(s.data_key().is_ok());
		assert_eq!(s.db_path().unwrap(), std::path::Path::new("/data/sites_panel.db"));
		assert_eq!(format!("{}", from(&[]).unwrap().db_path().unwrap_err()), "PANEL_DB_PATH must be set");
		let shown = format!("{s:?}");
		assert!(!shown.contains(&key), "{shown}");
	}

	#[test]
	fn capture_is_off_without_a_project_key() {
		assert!(from(&[]).unwrap().capture().unwrap().is_none());
		let on = from(&[("POSTHOG_PROJECT_API_KEY", "phc_abc123")]).unwrap().capture().unwrap().unwrap();
		assert_eq!((on.host.as_str(), on.key.as_str()), ("https://us.i.posthog.com", "phc_abc123"));
		assert!(from(&[("POSTHOG_PROJECT_API_KEY", "phx_personal")]).unwrap().capture().is_err(), "a personal key is refused");
		let plain = from(&[("POSTHOG_PROJECT_API_KEY", "phc_x"), ("POSTHOG_HOST", "http://127.0.0.1:1"), ("APP_ENV", "development")]).unwrap();
		assert!(plain.capture().is_ok(), "plain http outside production");
	}

	#[test]
	fn the_posthog_links_need_the_project() {
		assert!(from(&[]).unwrap().posthog_project().unwrap().is_none());
		let p = from(&[("POSTHOG_PROJECT_ID", "614067")]).unwrap().posthog_project().unwrap().unwrap();
		assert_eq!((p.app_host.as_str(), p.project_id.as_str()), ("https://us.posthog.com", "614067"));
		assert!(from(&[("POSTHOG_PROJECT_ID", "../1")]).unwrap().posthog_project().is_err());
		assert!(
			from(&[("POSTHOG_PROJECT_ID", "1"), ("POSTHOG_APP_HOST", "https://us.posthog.com/api")])
				.unwrap()
				.posthog_project()
				.is_err()
		);
		let eu = from(&[("POSTHOG_PROJECT_ID", "1"), ("POSTHOG_APP_HOST", "https://eu.posthog.com/")]).unwrap();
		assert_eq!(eu.posthog_project().unwrap().unwrap().app_host, "https://eu.posthog.com");
	}

	#[test]
	fn google_calendar_is_off_without_a_client_or_a_token() {
		let vars = |v: &[(&str, &str)]| v.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect::<Vec<_>>();
		let token = "1//refresh-secret";
		assert!(from(&[]).unwrap().google(vars(&[])).unwrap().is_none());
		let client = from(&[("GOOGLE_OAUTH_CLIENT_ID", "cid"), ("GOOGLE_OAUTH_CLIENT_SECRET", "csecret")]).unwrap();
		assert!(client.google(vars(&[])).unwrap().is_none(), "no brand: off");
		let on = client
			.google(vars(&[
				("GOOGLE_CALENDAR_REFRESH_TOKEN_VIFNET", token),
				("GOOGLE_CALENDAR_ID_VIFNET", "bookings@group.calendar.google.com"),
				("GOOGLE_CALENDAR_REFRESH_TOKEN_AQUAFIX", token),
			]))
			.unwrap()
			.unwrap();
		let brands: Vec<&str> = on.calendars.keys().map(|b| b.as_str()).collect();
		assert_eq!(brands, ["aquafix", "vifnet"]);
		assert_eq!(
			on.calendars.values().map(|c| c.calendar_id.as_str()).collect::<Vec<_>>(),
			["primary", "bookings@group.calendar.google.com"]
		);
		assert_eq!(on.every_minutes, 5);
		assert!(!format!("{on:?} {client:?}").contains(token) && !format!("{client:?}").contains("csecret"), "secrets never print");
		let e = from(&[]).unwrap().google(vars(&[("GOOGLE_CALENDAR_REFRESH_TOKEN_VIFNET", token)])).unwrap_err();
		assert!(format!("{e}").contains("GOOGLE_OAUTH_CLIENT_ID"), "{e}");
		assert!(from(&[("GOOGLE_OAUTH_CLIENT_ID", "cid")]).unwrap().google_client().is_err());
		assert!(client.google(vars(&[("GOOGLE_CALENDAR_REFRESH_TOKEN_VIF NET", token)])).is_err());
	}

	#[test]
	fn the_scope_ledger_is_all_or_nothing_over_https() {
		let files = |path: &str| -> eyre::Result<String> {
			match path {
				"/etc/settings/ca" => Ok("-----BEGIN CERTIFICATE-----\nca\n".into()),
				"/etc/settings/cert" => Ok("-----BEGIN CERTIFICATE-----\ncert\n".into()),
				"/etc/settings/key" => Ok("-----BEGIN PRIVATE KEY-----\nkey-secret\n".into()),
				"/etc/settings/empty" => Ok(String::new()),
				_ => eyre::bail!("no such file"),
			}
		};
		let with = |extra: &[(&str, &str)]| {
			let mut vars = vec![
				("SCOPE_LEDGER_ADDR", "https://banking-scope-ledger:50054"),
				("SCOPE_LEDGER_TLS_CA_PEM_FILE", "/etc/settings/ca"),
				("SCOPE_LEDGER_TLS_CERT_PEM_FILE", "/etc/settings/cert"),
				("SCOPE_LEDGER_TLS_KEY_PEM_FILE", "/etc/settings/key"),
			];
			vars.extend_from_slice(extra);
			from(&vars).unwrap().scope_ledger_from(files)
		};
		assert!(from(&[]).unwrap().scope_ledger_from(files).unwrap().is_none());
		let on = with(&[]).unwrap().unwrap();
		assert_eq!(on.addr, "https://banking-scope-ledger:50054");
		assert!(on.client_key_pem.contains("key-secret"));
		assert!(!format!("{on:?}").contains("key-secret"), "the key never prints");

		let e = format!("{}", from(&[("SCOPE_LEDGER_ADDR", "https://banking:50054")]).unwrap().scope_ledger_from(files).unwrap_err());
		assert!(e.contains("SCOPE_LEDGER_TLS_CA_PEM_FILE, SCOPE_LEDGER_TLS_CERT_PEM_FILE, SCOPE_LEDGER_TLS_KEY_PEM_FILE"), "{e}");
		for addr in ["http://banking:50054", "https://banking:50054/v1", "banking:50054"] {
			let e = format!("{}", with(&[("SCOPE_LEDGER_ADDR", addr)]).unwrap_err());
			assert!(e.contains("SCOPE_LEDGER_ADDR"), "{addr}: {e}");
		}
		let e = format!("{}", with(&[("SCOPE_LEDGER_TLS_KEY_PEM_FILE", "/etc/settings/missing")]).unwrap_err());
		assert!(e.contains("SCOPE_LEDGER_TLS_KEY_PEM_FILE") && e.contains("no such file"), "{e}");
		let e = format!("{}", with(&[("SCOPE_LEDGER_TLS_CA_PEM_FILE", "/etc/settings/empty")]).unwrap_err());
		assert!(e.contains("holds no PEM"), "{e}");
	}
}
