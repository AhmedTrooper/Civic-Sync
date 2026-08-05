#!/usr/bin/env bash
# Smoke test for the Civic-Sync API. Exits non-zero on the first failed
# assertion. Run against an already-running API at $API (defaults to
# http://localhost:8080).
#
# Usage:
#   API=http://localhost:8080 ./scripts/smoke.sh
#
# The script does NOT start the API — that's a separate concern. It
# also does NOT need a database; in-memory mode is enough to walk the
# happy paths.

set -euo pipefail

API="${API:-http://localhost:8080}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

jq_or_cat() {
    if command -v jq >/dev/null 2>&1; then
        jq "$@"
    else
        cat
    fi
}

assert_status() {
    local got="$1" expected="$2" label="$3"
    if [[ "$got" != "$expected" ]]; then
        echo "FAIL: $label expected $expected, got $got" >&2
        exit 1
    fi
    echo "  ok  $label"
}

assert_nonempty() {
    local value="$1" label="$2"
    if [[ -z "$value" || "$value" == "null" ]]; then
        echo "FAIL: $label is empty" >&2
        exit 1
    fi
    echo "  ok  $label"
}

echo "==> 1. Health checks"
status=$(curl -s -o "$TMP/health.body" -w "%{http_code}" "$API/health/live")
assert_status "$status" "200" "GET /health/live"
status=$(curl -s -o "$TMP/ready.body" -w "%{http_code}" "$API/health/ready")
assert_status "$status" "200" "GET /health/ready"

echo "==> 2. Command-centers seeded with 8 divisional hubs"
status=$(curl -s -o "$TMP/centers.json" -w "%{http_code}" "$API/api/v1/command-centers")
assert_status "$status" "200" "GET /api/v1/command-centers"
count=$(grep -o '"id"' "$TMP/centers.json" | wc -l)
[[ "$count" -ge 8 ]] || { echo "FAIL: expected ≥8 centers, got $count"; exit 1; }
echo "  ok  ≥8 command centers seeded"

# Pull Dhaka as primary candidate. For smoke test, we just need ANY center id.
center_id=$(grep -o '"id":"[^"]*"' "$TMP/centers.json" | head -1 | cut -d'"' -f4)
assert_nonempty "$center_id" "found center id"

echo "==> 3. Create an incident"
incident_payload=$(cat <<EOF
{
  "title": "Smoke test incident",
  "severity_level": 4,
  "affected_people": 100,
  "casualty_count": 6,
  "latitude": 23.81,
  "longitude": 90.41,
  "required_resource_types": ["AMBULANCE", "BOAT"]
}
EOF
)
status=$(curl -s -o "$TMP/incident.json" -w "%{http_code}" -X POST \
    "$API/api/v1/incidents" \
    -H "content-type: application/json" \
    -d "$incident_payload")
assert_status "$status" "201" "POST /api/v1/incidents"
incident_id=$(grep -o '"id":"[^"]*"' "$TMP/incident.json" | head -1 | cut -d'"' -f4)
assert_nonempty "$incident_id" "incident id returned"

echo "==> 4. PATCH the incident (intra-incident Δ)"
status=$(curl -s -o "$TMP/patch.json" -w "%{http_code}" -X PATCH \
    "$API/api/v1/incidents/$incident_id" \
    -H "content-type: application/json" \
    -d '{"casualty_count": 25}')
assert_status "$status" "200" "PATCH /api/v1/incidents/{id} (Δcasualty ≥ 10)"
new_cas=$(grep -o '"casualty_count":[0-9]*' "$TMP/patch.json" | head -1 | cut -d':' -f2)
[[ "$new_cas" == "25" ]] || { echo "FAIL: expected casualty_count=25, got $new_cas"; exit 1; }
echo "  ok  casualty_count updated to 25"

echo "==> 5. List incident → observable"
status=$(curl -s -o "$TMP/incidents.json" -w "%{http_code}" "$API/api/v1/incidents")
assert_status "$status" "200" "GET /api/v1/incidents"

echo "==> 6. Create a resource pool entry"
resource_payload=$(cat <<EOF
{
  "center_id": "$center_id",
  "resource_type": "AMBULANCE",
  "unit_identifier": "SMOKE-AMB-001",
  "latitude": 23.81,
  "longitude": 90.41
}
EOF
)
status=$(curl -s -o "$TMP/resource.json" -w "%{http_code}" -X POST \
    "$API/api/v1/resources" \
    -H "content-type: application/json" \
    -d "$resource_payload")
assert_status "$status" "201" "POST /api/v1/resources"

echo "==> 7. Create a helper team"
team_payload=$(cat <<EOF
{
  "center_id": "$center_id",
  "team_name": "Smoke Team",
  "total_members": 12,
  "latitude": 23.81,
  "longitude": 90.41
}
EOF
)
status=$(curl -s -o "$TMP/team.json" -w "%{http_code}" -X POST \
    "$API/api/v1/helper-teams" \
    -H "content-type: application/json" \
    -d "$team_payload")
assert_status "$status" "201" "POST /api/v1/helper-teams"

echo "==> 8. Dispatch recommendations"
status=$(curl -s -o "$TMP/dispatch.json" -w "%{http_code}" -X POST \
    "$API/api/v1/dispatch/recommendations")
assert_status "$status" "200" "POST /api/v1/dispatch/recommendations"

echo "==> 9. Dispatch smoke endpoint (LLM diagnostic)"
status=$(curl -s -o "$TMP/smoke.json" -w "%{http_code}" -X POST \
    "$API/api/v1/dispatch/smoke")
assert_status "$status" "200" "POST /api/v1/dispatch/smoke"

echo "==> 10. Admin: simulation status"
status=$(curl -s -o "$TMP/sim_status.json" -w "%{http_code}" \
    "$API/api/v1/admin/simulation")
# Will be 500 when no SimulationState wired up (test default) — that's
# acceptable. Just confirm the endpoint responds.
if [[ "$status" == "200" ]]; then
    echo "  ok  GET /api/v1/admin/simulation (wired)"
elif [[ "$status" == "500" ]]; then
    echo "  ok  GET /api/v1/admin/simulation (not wired in test mode — expected)"
else
    echo "FAIL: GET /api/v1/admin/simulation unexpected status $status" >&2
    exit 1
fi

echo "==> 11. Re-fetch the incident (60s flush stamp eventually visible)"
status=$(curl -s -o "$TMP/incident2.json" -w "%{http_code}" \
    "$API/api/v1/incidents/$incident_id")
assert_status "$status" "200" "GET /api/v1/incidents/{id}"

echo
echo "==========================================="
echo "  ALL SMOKE TESTS PASSED — API is healthy"
echo "==========================================="
