#!/bin/bash
# talon-panic.sh -- Emergency flatten script
# Authenticates directly to broker(s), submits market close-all, sends SMS.
# Ref: TALON_ARCHITECTURE_v3.2.0 S12.3
#
# This script is EXTERNAL to TALON. It runs independently.
# Triggered by: systemd watchdog timeout, manual execution, 0DTE crash after 3:30 PM.
# Tested monthly on paper account.
#
# Usage: bash talon-panic.sh [--live]
#        Without --live, runs against paper account.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$SCRIPT_DIR/../config"
LOG_FILE="$SCRIPT_DIR/../data/panic.log"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

MODE="paper"
if [[ "${1:-}" == "--live" ]]; then
    MODE="live"
fi

log() {
    echo "[$TIMESTAMP] PANIC ($MODE): $1" | tee -a "$LOG_FILE"
}

log "=== TALON PANIC SCRIPT INITIATED ==="
log "Mode: $MODE"

# --- Step 1: Flatten IBKR positions ---
log "Attempting IBKR flatten..."
# TODO: Implement IBKR Client Portal REST close-all
# POST /v1/api/iserver/account/{accountId}/orders
# Use market orders for everything.
log "IBKR flatten: NOT YET IMPLEMENTED"

# --- Step 2: Flatten Cobra positions (if configured) ---
if [ -f "$CONFIG_DIR/brokers/cobra.toml" ]; then
    log "Attempting Cobra flatten..."
    # TODO: Implement DAS TAPI close-all or web API fallback
    log "Cobra flatten: NOT YET IMPLEMENTED"
fi

# --- Step 3: Send SMS notification ---
log "Sending SMS notification..."
# TODO: Implement SMS via Twilio or similar
# Include: timestamp, mode, positions flattened, errors
log "SMS: NOT YET IMPLEMENTED"

log "=== TALON PANIC SCRIPT COMPLETE ==="
log "MANUAL VERIFICATION REQUIRED -- confirm all positions closed at broker."
log "IBKR: https://www.interactivebrokers.com/webtrader"
log "Cobra: https://das.cobratrading.com"

exit 0
