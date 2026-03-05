# CashCache

## Directory Structure

```
~/projects/CashCache/
+-- README.md                              # This file
+-- specs/
|   +-- TALON_ARCHITECTURE_v3_2_0.md       # System architecture (canonical)
|   +-- CASHCACHE_MODULE_SPEC_v0_4_0.md    # Vault module spec
+-- skills/
|   +-- rust-systems-spec/
|   |   +-- SKILL.md
|   |   +-- references/
|   |       +-- async-patterns.md
|   |       +-- integration-ports.md
|   |       +-- spec-structure.md
|   |       +-- tui-philosophy.md
|   +-- trading-system-spec/
|       +-- SKILL.md
|       +-- references/
|           +-- broker-landscape.md
|           +-- options-engineering.md
|           +-- settlement-and-risk.md
|           +-- supervision-models.md
+-- TALON/
|   +-- config/
|   |   +-- system.toml                    # Tiers, regulatory, paths, modes
|   |   +-- risk.toml                      # Risk mesh, stress, flameout, forced cover
|   |   +-- graduation.toml               # Gates, track record, demotion
|   |   +-- brokers/
|   |   |   +-- ibkr.toml                 # P0 -- all tiers + Vault
|   |   |   +-- alpaca.toml               # P1 -- Hatch alt
|   |   |   +-- cobra.toml                # P2 -- Turbo primary
|   |   |   +-- webull.toml               # P1-ALT -- Hatch alt
|   |   +-- modules/
|   |       +-- firebird.toml             # Hatch -- oversold reversal
|   |       +-- thunderbird.toml          # Hatch -- overextension fade
|   |       +-- taxi.toml                 # Hatch -- equity swing
|   |       +-- cashcache.toml            # Hatch -- profit preservation vault
|   |       +-- snapback.toml             # Takeoff -- mean-reversion 0DTE
|   |       +-- climb.toml               # Takeoff -- intraday momentum
|   |       +-- sage.toml                # Turbo -- gamma exposure scalp
|   |       +-- parashort.toml           # Turbo -- parabolic fade
|   |       +-- siphon.toml             # Turbo -- theta decay farming
|   |       +-- yoyo.toml               # Turbo -- binary event 0DTE
|   |       +-- payload.toml            # Turbo -- dynamic regime 0DTE
|   +-- data/                             # Runtime (gitignored)
|   |   +-- events.db                     # Event-sourced log (trust, stress, trades, vault)
|   |   +-- market.db                     # Market data archive
|   |   +-- sovereign.db.enc              # Encrypted sovereign data
|   +-- backups/                          # Runtime (gitignored)
|   |   +-- daily/                        # Daily SQLite backups
|   +-- scripts/
|       +-- pre_deliver_lint.sh           # ASCII + TOML lint for specs/configs
|       +-- talon-panic.sh               # Emergency flatten (external to TALON)
+-- PIREP/
|   +-- README.md                          # MarketPIREP placeholder
+-- Vault/
    +-- README.md                          # CashCache Vault documentation home
```

## File Counts

- Specs: 2
- Skills: 2 (10 files total with references)
- Config: 18 (.toml files)
- Scripts: 2
- READMEs: 3 (root + PIREP + Vault)
- Runtime data: 3 (.db/.enc, created at first run)

Total files to store: 35

## What Goes Where

| File Type | Location | Git Tracked? |
|---|---|---|
| Architecture specs | specs/ | Yes |
| AI skills (.skill zips) | skills/ | Yes |
| System config | TALON/config/ | Yes (secrets in .env or vault, not TOML) |
| Runtime databases | TALON/data/ | No -- gitignored |
| Daily backups | TALON/backups/ | No -- gitignored |
| Scripts | TALON/scripts/ | Yes |
| Future PIREP code | PIREP/ | Yes (when written) |
| Future Vault tooling | Vault/ | Yes (when written) |
