```sh
# Settings come from the environment only: PANEL_DB_PATH, the SQLite file (created and migrated
# by whichever command opens it first), and PANEL_DATA_KEY (64 hex characters) that seals PII
# and the queued notes. `sites_panel --print-required-vars` lists what production needs.
export PANEL_DATA_KEY="$(sites_panel gen-data-key)"
export PANEL_DB_PATH=./panel.db

# Every command migrates on open; this one does nothing else.
sites_panel migrate

# Source keys are the conductor's (site_conductor, `POST /api/v1/sources`); none are issued here.

# HTTP behind the conductor, trusting only its assertions: PANEL_ASSERTION_KEYS are its public
# keys. TELEGRAM_LOCALE=ru|en is the notes' buttons' language.
sites_panel serve --bind 127.0.0.1:59120       # or PANEL_BIND

# leads, calls and payments again from the journal, against the registry as it is now
sites_panel rebuild-projections
```
