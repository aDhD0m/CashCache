# CashCache Vault

Profit preservation module. Lives inside TALON as a module but has its own
spec due to complexity. This directory is the conceptual home for Vault-specific
tooling, scripts, and documentation that sits outside the TALON binary.

## Canonical Spec

specs/CASHCACHE_MODULE_SPEC_v0_4_0.md

## Runtime Config

TALON/config/modules/cashcache.toml

## Runtime Data

Vault events are stored in TALON/data/events.db alongside all other TALON
events (event-sourced, append-only, immutable).

## Broker Residency

Always IBKR. Separate sub-account. See CASHCACHE_MODULE_SPEC S8.
