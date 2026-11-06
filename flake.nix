{
  nixConfig = {
    extra-substituters = [ "https://valeratrades.cachix.org" ];
    extra-trusted-public-keys = [ "valeratrades.cachix.org-1:gXVwhzO5YB+BaiEJYT48qZgzdaErGQew6xtZcz4Fo1Q=" ];
  };

  inputs = {
    v_flakes.url = "github:valeratrades/v_flakes?ref=v1.6";
  };

  outputs = { self, v_flakes }:
    let
      # the first of sites' ten in SA_PORT_RANGE: site_conductor's docs/port_range.md
      SA_PORTS = 52820;
      inherit (v_flakes) flake-utils pre-commit-hooks;
      manifest = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package;
      # The binary (crates/panel_server, `[[bin]] name`), the image and the release all go
      # by this name; the workspace root has no package of its own.
      pname = "sites_panel";
    in
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import v_flakes.default_nixpkgs { inherit system; };
        lib = pkgs.lib;
        rust = v_flakes.rs.default_nightly system;
        # mold is Linux-only — the adapter throws at *eval* time on Darwin, which
        # would break every target on a mac, not just the ones that link.
        stdenv = if pkgs.stdenv.isDarwin then pkgs.stdenv else pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv;

        # Single source for the container's exposed port and the prod bind.
        port = 59121;

        # `nix run .#gen`: the TS mirror of the Rust (crates/panel_gen) — the front end's and
        # `@evinvest/sites-contract`'s — committed.
        gen = pkgs.writeShellApplication {
          name = "gen";
          runtimeInputs = [ rust pkgs.git ];
          text = ''
            cd "$(git rev-parse --show-toplevel)"
            exec cargo run -q -p panel_gen "$@"
          '';
        };

        pre-commit-check = pre-commit-hooks.lib.${system}.run (
          let base = v_flakes.files.preCommit { inherit pkgs; stripClaudeSignature = true; }; in
          base // {
            hooks = base.hooks // {
              # Regenerate and re-stage rather than assert: a stale mirror type-checks against
              # itself and fails at runtime, in the browser.
              generated = {
                enable = true;
                name = "regenerate derived files";
                entry = "bash -c '${gen}/bin/gen && git add -A frontend/src/entities/lead/model/generated.ts frontend/src/shared/config/generated.ts frontend/src/shared/api/generated.ts ts/sites-contract/src/generated'";
                pass_filenames = false;
                require_serial = true;
              };
            };
          }
        );
        rs = v_flakes.rs {
          inherit pkgs rust;
          build.workspace = {
            "./crates/panel_server" = [ "git_version" "log_directives" ];
          };
        };
        github = v_flakes.github {
          inherit pkgs pname rs;
          enable = true;
          lastSupportedVersion = "nightly-2026-09-03";
          containerRelease = { registry = "ghcr.io/ev-service-arb"; };
          # ts/sites-contract: `@evinvest/sites-contract-vX.Y.Z` tags publish it
          npmRelease = {
            scope = "evinvest";
            packagesDir = "ts";
          };
          jobs.default = true;
          lfs = false;
        };
        readme = v_flakes.readme-fw {
          inherit pkgs pname;
          defaults = true;
          lastSupportedVersion = "nightly-1.100";
          rootDir = ./.;
          badges = [ "msrv" "loc" "ci" ];
        };
        combined = v_flakes.utils.combine { inherit rust; modules = [ rs github readme ]; };

        build_rust = v_flakes.rs.build_nightly system;
        rustPlatform = pkgs.makeRustPlatform { rustc = build_rust; cargo = build_rust; inherit stdenv; };
        # `.cargo` holds dev-only accelerators (sccache rustc-wrapper, cranelift,
        # mold) the hermetic sandbox lacks — drop it so the pure build uses nix's
        # own toolchain instead of failing on a missing `sccache` on PATH.
        pureSrc = lib.cleanSourceWith {
          src = lib.cleanSource ./.;
          filter = path: _type: baseNameOf path != ".cargo";
        };

        bin = rustPlatform.buildRustPackage {
          inherit pname;
          version = manifest.version;
          src = pureSrc;
          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          cargoBuildFlags = [ "-p" "panel_server" ];
          nativeBuildInputs = with pkgs; [ pkg-config ];
          # ev_lib's `sentry` turns on reqwest's native-tls, which is OpenSSL on Linux
          # (Security.framework on Darwin, which needs nothing here).
          buildInputs = lib.optionals pkgs.stdenv.isLinux [ pkgs.openssl ];
          # `cargo test` runs in the devShell and CI; the tests bind loopback servers (fake
          # PostHog and Google), which the Darwin sandbox refuses unless asked
          doCheck = false;
          auditable = false; # cargo-auditable doesn't support edition 2024
        };

        # The front end's bundle (`npm run build` → dist/: mfe-sites.js, mfe-sites.css and hashed
        # chunks), served by `sites_panel serve` at /mfe/* from PANEL_MFE_DIR.
        frontend = pkgs.buildNpmPackage {
          pname = "${pname}-frontend";
          version = manifest.version;
          # with the sources of @evinvest/sites-contract, which frontend/ imports by path
          src = lib.cleanSourceWith {
            src = lib.cleanSource ./.;
            filter =
              path: type:
              let
                rel = lib.removePrefix "${toString ./.}/" (toString path);
                under = dir: rel == dir || lib.hasPrefix "${dir}/" rel;
              in
              !(builtins.elem (baseNameOf path) [ "node_modules" "dist" ".build" ])
              && (under "frontend" || under "ts/sites-contract/src" || (type == "directory" && builtins.elem rel [ "ts" "ts/sites-contract" ]));
          };
          sourceRoot = "source/frontend";
          npmDepsHash = "sha256-5Yd0nJnTKvE70mh7azistHWKjdx1MKBUd4Uyjv2Wayk=";
          installPhase = ''
            runHook preInstall
            cp -r dist "$out"
            runHook postInstall
          '';
        };

        # The database: one SQLite file on the pod's volume, created and migrated by `serve` on
        # start (no init container, no migrate step), replicated off the pod by the cluster's
        # litestream — which is what `sqlite` below asks for. One writer: one pod, replaced
        # (Recreate), never two at once on the volume.
        dbPath = "/data/sites_panel.db";

        # What the deploy must provide, beyond what `container.implement` records. Plain
        # data on the contract, read by the devops generator; `checks.contract-env` holds
        # `requiredEnv` to the binary's own `--print-required-vars`.
        deploy = {
          # Settings::required_var_names("production"): the pod exits 78 without any of them.
          # PANEL_DB_PATH is the container's own env (below); the rest the deploy supplies.
          requiredEnv = [ "PANEL_DB_PATH" "PANEL_DATA_KEY" "PANEL_ASSERTION_KEYS" ];
          # from the sops-backed Secret, never literal env
          secretEnv = [
            "PANEL_DATA_KEY"
            "SENTRY_DSN"
            "GOOGLE_OAUTH_CLIENT_SECRET"
            "GOOGLE_CALENDAR_REFRESH_TOKEN_AQUAFIX"
            "GOOGLE_CALENDAR_REFRESH_TOKEN_VIFNET"
          ];
          # PANEL_ASSERTION_KEYS: the conductor's public keys (not secret), `<kid>:<base64>`, comma-separated.
          # POSTHOG_PROJECT_API_KEY (phc_, public like the landings', not a secret; + POSTHOG_HOST):
          # serve sends the leads' life to PostHog; unset, nothing is sent (serve warns).
          # POSTHOG_PROJECT_ID (+ POSTHOG_APP_HOST): the experiments link to their funnels in
          # PostHog; unset, no links. Links are opened by the browser: no egress for them.
          # Google Calendar booking pull (docs/ARCHITECTURE.md, Booking): the OAuth client both or
          # neither; a brand's calendar is pulled only with its refresh token
          # (`sites_panel booking google-authorize <brand>` makes one); its calendar id defaults to
          # `primary`. A token without the client fails the boot.
          # SCOPE_LEDGER_* (docs/ARCHITECTURE.md, Invoices): banking's ScopeLedger over mTLS, the
          # address and the three *_PEM_FILE paths all or none; unset, /api/v1/invoices answers
          # 503. The *_PEM_FILE values are literal paths (`/etc/settings/<VAR>`) to files mounted
          # from the Secret, as invest's are: the paths are not secrets, so not in secretEnv.
          optionalEnv = [
            "SENTRY_DSN"
            "TELEGRAM_LOCALE"
            "POSTHOG_PROJECT_API_KEY"
            "POSTHOG_HOST"
            "POSTHOG_PROJECT_ID"
            "POSTHOG_APP_HOST"
            "GOOGLE_OAUTH_CLIENT_ID"
            "GOOGLE_OAUTH_CLIENT_SECRET"
            "GOOGLE_CALENDAR_SYNC_MINUTES"
            "GOOGLE_CALENDAR_REFRESH_TOKEN_AQUAFIX"
            "GOOGLE_CALENDAR_ID_AQUAFIX"
            "GOOGLE_CALENDAR_REFRESH_TOKEN_VIFNET"
            "GOOGLE_CALENDAR_ID_VIFNET"
            "SCOPE_LEDGER_ADDR"
            "SCOPE_LEDGER_TLS_CA_PEM_FILE"
            "SCOPE_LEDGER_TLS_CERT_PEM_FILE"
            "SCOPE_LEDGER_TLS_KEY_PEM_FILE"
          ];
          # No public route: the conductor forwards /api/sites/*, ingest, /api/hooks and /sites/mfe/*
          # here by service DNS; the landings read /api/internal the same way.
          egress = {
            # banking's ScopeLedger
            grpcEnv = [ "SCOPE_LEDGER_ADDR" ];
            # PostHog's capture host (POSTHOG_HOST's default — follow it if it is pointed
            # elsewhere); Google's token endpoint and Calendar API for the booking pull
            hosts = [ "us.i.posthog.com:443" "oauth2.googleapis.com:443" "www.googleapis.com:443" ];
          };
        };

        containerStd = v_flakes.container.implement {
          inherit pkgs pname;
          containers."" = {
            inherit port;
            mounts = [ "/data" ];
            sqlite = [ dbPath ];
            healthPath = "/health";
            # a source that gets a 5xx retries from its outbox; nothing is lost while down
            criticality = "normal";
            entrypoint = [ "${bin}/bin/${pname}" "serve" "--bind" "0.0.0.0:${toString port}" ];
            # /bin/sites_panel, for `kubectl exec … sites_panel place set` and the like
            contents = [ bin ];
            # the production guards (`#[required_in("production")]`) are armed wherever it runs
            # PostHog: the project the landings send to (Cloud US 614067, their
            # deploy/config.nix); the phc_ token is a public write-only ingest key
            env = {
              APP_ENV = "production";
              PANEL_DB_PATH = dbPath;
              POSTHOG_PROJECT_API_KEY = "phc_sBwWEgdgockVmfyucBRkTTo6iZ4Y2eApSGorD22WLzj3";
              POSTHOG_PROJECT_ID = "614067";
            };
            imageEnv = [ "PANEL_MFE_DIR=${frontend}" ];
          };
        };
        # `implement` knows no key for the above, and throws on one it does not know.
        # `provides`: endpoint -> path, what devops derives a consumer's URL and ingress from.
        provides.internal = "/api/internal";
        containers = lib.mapAttrs (_: c: c // { contract = c.contract // deploy // { inherit provides; }; }) containerStd.containers;

        contractEnv = pkgs.runCommand "${pname}-contract-env" { } ''
          ${bin}/bin/${pname} --print-required-vars production | sort > got
          printf '%s\n' ${lib.escapeShellArgs deploy.requiredEnv} | sort > want
          diff -u want got || { echo "flake.nix deploy.requiredEnv disagrees with the binary" >&2; exit 1; }
          touch "$out"
        '';

        contractProvides = pkgs.runCommand "${pname}-contract-provides" { } ''
          ${bin}/bin/${pname} --print-provided-paths | sort > got
          printf '%s\n' ${lib.escapeShellArgs (lib.attrValues provides)} | sort > want
          diff -u want got || { echo "flake.nix provides disagrees with the paths the router mounts" >&2; exit 1; }
          touch "$out"
        '';

        help = pkgs.writeShellApplication {
          name = "help";
          text = ''
            cat <<'EOF'
            nix develop                       toolchain, sqlite
            nix build                         the sites_panel binary
            nix build .#${pname}-container    OCI image, the front end's bundle in it (Linux only)
            nix build .#frontend              the front end's bundle
            nix run .#publish -- major|minor|patch [note]
                                              tag the next vX.Y.Z and push it: CI ships the image
            nix run .#gen                     the TS mirror of the Rust types: the front end's
                                              and @evinvest/sites-contract's (pre-commit runs it)
            git tag @evinvest/sites-contract-vX.Y.Z && git push origin <tag>
                                              publish ts/sites-contract (its package.json at X.Y.Z)
            nix run .#help                    this
            cargo test                        every test; the database ones on throwaway
                                              SQLite files, nothing to set up
            the CLI itself: sites_panel --help
            EOF
          '';
        };
        # The version lives in the tag, not in Cargo.toml: `release-container.yml` ships the image on it.
        publish = pkgs.writeShellApplication {
          name = "publish";
          runtimeInputs = with pkgs; [ git ];
          text = ''
            part="''${1:-}"
            case "$part" in
              major | minor | patch) ;;
              *) echo "usage: nix run .#publish -- major|minor|patch [release note]" >&2; exit 1 ;;
            esac
            [ -z "$(git status --porcelain)" ] || { echo "uncommitted changes — commit or stash first" >&2; exit 1; }
            shift
            note="$*"

            git fetch --tags --force origin >/dev/null # not silenced: a stale tag list reuses a version
            last="$(git tag -l 'v*' --sort=-v:refname | head -n1)"
            ver="''${last#v}"; [ -n "$ver" ] || ver="0.0.0"
            ma="''${ver%%.*}"; rest="''${ver#*.}"; mi="''${rest%%.*}"; pa="''${rest##*.}"
            case "$part" in
              major) ma=$((ma + 1)); mi=0; pa=0 ;;
              minor) mi=$((mi + 1)); pa=0 ;;
              patch) pa=$((pa + 1)) ;;
            esac
            next="v$ma.$mi.$pa"
            echo "''${last:-<no tag>} → $next"

            # annotated: a lightweight tag has nowhere to put a note, and `--follow-tags` skips it
            if [ -n "$note" ]; then
              printf '%s\n\n%s\n' "$next" "$note" | git tag -a "$next" -F -
            else
              git tag -a "$next" -m "$next"
            fi
            git push origin "$next"
          '';
        };
      in
      {
        apps = {
          help = { type = "app"; program = lib.getExe help; };
          publish = { type = "app"; program = lib.getExe publish; };
          gen = { type = "app"; program = lib.getExe gen; };
        };

        packages = {
          default = bin;
          inherit bin frontend;
        } // lib.optionalAttrs pkgs.stdenv.isLinux containerStd.packages;

        containers = lib.optionalAttrs pkgs.stdenv.isLinux containers;

        checks.contract-env = contractEnv;
        checks.contract-provides = contractProvides;

        devShells.default =
          with pkgs;
          mkShell {
            inherit stdenv;
            shellHook =
              pre-commit-check.shellHook
              + combined.shellHook
              + ''
                cp -f ${(v_flakes.files.treefmt) { inherit pkgs; }} ./.treefmt.toml
                cp -f ${(v_flakes.files.gitattributes) { inherit pkgs; lfs = false; }} ./.gitattributes
              '';

            packages = [
              mold
              pkg-config
              openssl # sentry's native-tls
              sqlite # inspecting the database
              rust
            ] ++ pre-commit-check.enabledPackages ++ combined.enabledPackages;

            env = {
              PANEL_BIND = "127.0.0.1:${toString SA_PORTS}";
              STUB_PORT = toString (SA_PORTS + 3); # frontend/'s `npm run dev`
              RUST_BACKTRACE = 1;
              RUST_LIB_BACKTRACE = 0;
            };
          };
      }
    );
}
