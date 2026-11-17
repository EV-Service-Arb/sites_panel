```sh
# Settings come from the environment only: PANEL_DB_PATH, the SQLite file (created and migrated
# by whichever command opens it first), and PANEL_DATA_KEY (64 hex characters) that seals PII
# and the sources' HMAC secrets. `panel --print-required-vars` lists what production needs.
export PANEL_DATA_KEY="$(panel gen-data-key)"
export PANEL_DB_PATH=./panel.db

# Every command migrates on open; this one does nothing else.
panel migrate

# A source: its key may write events of one kind, for the brands named. The secret is printed once.
panel source add aquafix-site --kind site --brand aquafix
panel source add aquafix-tg --kind bot --brand aquafix      # a messenger bot: docs/BOT-API.md
panel source list
panel source revoke aquafix-site

# HTTP on 127.0.0.1:59120. Ingest alone, unless signing in is configured — all four or none:
#   PANEL_PUBLIC_ORIGIN=https://sa.evinvest.ltd   CONCIERGE_PUBLIC_ORIGIN=https://evinvest.ltd
#   CONCIERGE_GRPC_ADDR=http://concierge:55670    RP_CLIENT_SECRET_SA=<the secret concierge hashed>
# which adds /auth/login, /auth/callback, /auth/logout and the operator API under /api/v1.
# With signing in, TELEGRAM_BOT_TOKEN turns the bot on (long polling; TELEGRAM_BOT_USERNAME
# spares a getMe, TELEGRAM_LOCALE=ru|en picks its language, ru by default).
panel serve

# leads, calls and payments again from the journal, against the registry as it is now
panel rebuild-projections
```
