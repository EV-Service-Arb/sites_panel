# Running locally

sites_panel answers nothing but `/health`, `/api/internal`, `/api/hooks` and `/mfe` without the
conductor's assertion, so the stack runs from site_conductor (`nix run .#local-stack` there): the
conductor signs you in (dev sign-in), holds the source keys, and forwards to this service, which
it starts from this checkout.

What sites_panel needs from whoever starts it:

| Variable | Value |
| --- | --- |
| `PANEL_DB_PATH` | its SQLite file, created and migrated on start |
| `PANEL_DATA_KEY` | `sites_panel gen-data-key` |
| `PANEL_ASSERTION_KEYS` | the conductor's public key, `<kid>:<base64>` |
| `PANEL_MFE_DIR` | `frontend/dist` (`npm run build` in `frontend/`); unset, no `/mfe` |

```sh
cargo r -p panel_server -- serve            # 127.0.0.1:59121
cargo r -p panel_server -- place register aquafix royat
```

The landings read their places, pricing and experiments from `http://127.0.0.1:59121/api/internal`
(`SITES_PANEL_INTERNAL_URL`, which they extend with `/brands/<brand>/…`) and send their leads to the conductor's ingest.
