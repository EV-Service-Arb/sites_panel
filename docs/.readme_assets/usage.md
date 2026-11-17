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

# A place's live settings (see "Place settings"): set some fields, clear others, the rest stay
sites_panel place register aquafix royat       # known to the panel, nothing set; a no-op when known
sites_panel place set aquafix royat --phone +33423500640 --whatsapp +33612345678 \
  --hours 'Mo-Fr 08:00-19:00,Sa 09:00-12:00' --service-area 'Royat,Chamalières'
sites_panel place set aquafix royat --clear whatsapp
sites_panel place set aquafix royat --telegram aquafix_devis_bot --messengers whatsapp=on,telegram=off
sites_panel place set aquafix royat --clear telegram --clear messengers
sites_panel place show aquafix royat
sites_panel place history aquafix royat        # every change, newest first, with its id
sites_panel place revert aquafix royat <change id>   # the settings that change found, back
sites_panel place withdraw aquafix royat       # the sites answer it as gone (404)
sites_panel place restore aquafix royat
```
