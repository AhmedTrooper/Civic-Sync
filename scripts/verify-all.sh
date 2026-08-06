#!/usr/bin/env bash
# Single-shot verification for the whole stack.
#
# Runs:
#   1. Backend static checks:    cargo check  / clippy / fmt-check
#   2. Backend live smoke:       scripts/smoke.sh        (12 endpoint stages)
#   3. Frontend payload contract: scripts/verify-payloads.sh (18 body shape checks)
#   4. Frontend zod contract:    web/tests/payloads.test.ts  (8 schema checks)
#
# The first 3 stages need the API running. Stage 1 is offline; we run it
# first to fail fast before starting the server. The script then starts the
# API in the background for stages 2–4 and tears it down on exit.
#
# Override: SKIP_API=1 to skip the server boot (assume one is already up);
# PASS=... to relax; CI=1 to assume bun + cargo + jq are pre-installed.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BIN="$ROOT/api/target/debug/civic-sync-api"
API_LOG="$(mktemp -t civic-sync-api.XXXXXX.log)"
TMP="$(mktemp -d)"
SERVER_PID=""

cleanup() {
    if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -rf "$TMP"
}
trap cleanup EXIT

green() { printf "\033[32m%s\033[0m\n" "$*"; }
red()   { printf "\033[31m%s\033[0m\n" "$*"; }
gray()  { printf "\033[90m%s\033[0m\n" "$*"; }

stage() { printf "\n\033[1m===> %s\033[0m\n" "$1"; }

require() {
    if ! command -v "$1" >/dev/null 2>&1; then
        red "missing: $1"
        exit 1
    fi
}

require cargo
require jq

# ---- STAGE 1: backend static checks (offline) ----------------------------
stage "1/4 backend static checks"
(
    cd "$ROOT/api"
    DATABASE_URL="" cargo check --quiet
    DATABASE_URL="" cargo clippy --all-targets --quiet -- -D warnings
    cargo fmt --check --quiet
)
green "cargo check, clippy, fmt-check all green"

# ---- start API for stages 2–4 (unless caller already has one up) --------
start_api() {
    if [[ "${SKIP_API:-0}" == "1" ]]; then
        gray "SKIP_API=1, assuming server is already up at $API"
        return
    fi
    if [[ ! -x "$API_BIN" ]]; then
        red "binary not found: $API_BIN (run: cd api && cargo build)"
        exit 1
    fi
    gray "starting $API_BIN (log: $API_LOG)"
    DATABASE_URL="" PORT="${PORT:-8080}" RUST_LOG="${RUST_LOG:-info}" \
        "$API_BIN" >"$API_LOG" 2>&1 &
    SERVER_PID=$!
    # wait for /health/live
    local i
    for i in {1..30}; do
        if curl -s -m 1 -o /dev/null -w "%{http_code}" \
                "http://localhost:${PORT:-8080}/health/live" | grep -q 200; then
            green "API listening (pid=$SERVER_PID)"
            return
        fi
        sleep 0.5
    done
    red "API never came up; last log:"
    tail -40 "$API_LOG" >&2 || true
    exit 1
}

start_api

API="http://localhost:${PORT:-8080}"
export API

# ---- STAGE 2: backend live smoke ----------------------------------------
stage "2/4 backend live smoke (scripts/smoke.sh)"
API="$API" bash "$ROOT/scripts/smoke.sh"
green "smoke.sh — all stages pass"

# ---- STAGE 3: frontend payload contract ---------------------------------
stage "3/4 frontend payload contract (scripts/verify-payloads.sh)"
API="$API" bash "$ROOT/scripts/verify-payloads.sh"
green "verify-payloads.sh — every web fetch body accepted by the backend"

# ---- STAGE 4: frontend zod contract --------------------------------------
stage "4/4 frontend zod contract (web/tests/payloads.test.ts)"
cd "$ROOT/web"
API="$API" ./node_modules/.bin/vitest run tests/payloads.test.ts
green "zod schemas accept every API response"

printf "\n\033[1;32m=============================================\n"
printf "  ALL 4 STAGES PASS — submission-ready\n"
printf "=============================================\033[0m\n"
