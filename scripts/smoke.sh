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
# happy paths, including the full dispatch pipeline
# (recommend -> apply -> DISPATCHED).

set -euo pipefail

API="${API:-http://localhost:8080}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

ROLE_HEADER="x-role: admin"

assert_status() {
    local got="$1" expected="$2" label="$3"
    if [[ "$got" != "$expected" ]]; then
        echo "FAIL: $label expected $expected, got $got" >&2
        cat "$TMP/last.body" 2>/dev/null >&2 || true
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

get() { # get <path> <body-file> -> echoes status
    curl -s -o "$2" -w "%{http_code}" "$API$1"
}

mutate() { # mutate <method> <path> [json-body] -> echoes status, body in $TMP/last.body
    local method="$1" path="$2" body="${3:-}"
    if [[ -n "$body" ]]; then
        curl -s -o "$TMP/last.body" -w "%{http_code}" -X "$method" \
            -H "content-type: application/json" -H "$ROLE_HEADER" \
            -d "$body" "$API$path"
    else
        curl -s -o "$TMP/last.body" -w "%{http_code}" -X "$method" \
            -H "$ROLE_HEADER" "$API$path"
    fi
}

echo "==> 1. Health checks"
status=$(get "/health/live" "$TMP/last.body")
assert_status "$status" "200" "GET /health/live"
status=$(get "/health/ready" "$TMP/last.body")
assert_status "$status" "200" "GET /health/ready"

echo "==> 2. Command centers seeded with 8 divisional hubs"
status=$(get "/api/v1/centers" "$TMP/centers.json")
assert_status "$status" "200" "GET /api/v1/centers"
count=$(grep -o '"id"' "$TMP/centers.json" | wc -l)
[[ "$count" -ge 8 ]] || { echo "FAIL: expected >=8 centers, got $count"; exit 1; }
echo "  ok  >=8 command centers seeded"
status=$(get "/api/v1/command-centers" "$TMP/alias.json")
assert_status "$status" "200" "GET /api/v1/command-centers (frontend alias)"
center_id=$(grep -o '"id":"[^"]*"' "$TMP/centers.json" | head -1 | cut -d'"' -f4)
assert_nonempty "$center_id" "found center id"

echo "==> 3. RBAC: mutations require a role"
status=$(curl -s -o "$TMP/last.body" -w "%{http_code}" -X POST \
    -H "content-type: application/json" \
    -d '{"title":"rbac probe","severity_level":1,"affected_people":1,"casualty_count":0,"latitude":23.8,"longitude":90.4}' \
    "$API/api/v1/incidents")
assert_status "$status" "403" "POST /api/v1/incidents without x-role is forbidden"

echo "==> 4. Create an incident (admin role)"
status=$(mutate POST "/api/v1/incidents" '{
  "title": "Smoke test flash flood",
  "severity_level": 4,
  "affected_people": 100,
  "casualty_count": 6,
  "latitude": 23.81,
  "longitude": 90.41
}')
assert_status "$status" "201" "POST /api/v1/incidents"
incident_id=$(grep -o '"id":"[^"]*"' "$TMP/last.body" | head -1 | cut -d'"' -f4)
assert_nonempty "$incident_id" "incident id returned"

echo "==> 5. PATCH the incident (intra-incident delta)"
status=$(mutate PATCH "/api/v1/incidents/$incident_id" '{"casualty_count": 25}')
assert_status "$status" "200" "PATCH /api/v1/incidents/{id} (delta-casualty >= 10)"
new_cas=$(grep -o '"casualty_count":[0-9]*' "$TMP/last.body" | head -1 | cut -d':' -f2)
[[ "$new_cas" == "25" ]] || { echo "FAIL: expected casualty_count=25, got $new_cas"; exit 1; }
echo "  ok  casualty_count updated to 25"

echo "==> 6. List incidents -> observable"
status=$(get "/api/v1/incidents" "$TMP/last.body")
assert_status "$status" "200" "GET /api/v1/incidents"

echo "==> 7. Create a resource pool entry"
status=$(mutate POST "/api/v1/resources" "{
  \"owner_center_id\": \"$center_id\",
  \"resource_type\": \"AMBULANCE\",
  \"unit_identifier\": \"SMOKE-AMB-001\",
  \"latitude\": 23.81,
  \"longitude\": 90.41,
  \"total_capacity\": 4
}")
assert_status "$status" "201" "POST /api/v1/resources"
resource_id=$(grep -o '"id":"[^"]*"' "$TMP/last.body" | head -1 | cut -d'"' -f4)
assert_nonempty "$resource_id" "resource id returned"

echo "==> 8. Dispatch recommendations (AI tool-call envelopes)"
status=$(mutate POST "/api/v1/dispatch/recommendations")
assert_status "$status" "200" "POST /api/v1/dispatch/recommendations"
grep -q 'dispatch_multi_center_response' "$TMP/last.body" \
    || { echo "FAIL: no dispatch_multi_center_response envelope in recommendations"; exit 1; }
echo "  ok  structured tool-call envelope present"
grep -q 'priority_score' "$TMP/last.body" \
    || { echo "FAIL: no explainable priority queue in recommendations"; exit 1; }
echo "  ok  explainable priority queue present"

echo "==> 9. Dispatch smoke endpoint (LLM diagnostic)"
status=$(mutate POST "/api/v1/dispatch/smoke")
assert_status "$status" "200" "POST /api/v1/dispatch/smoke"

echo "==> 10. Apply the first recommendation (human-in-the-loop)"
if command -v jq >/dev/null 2>&1; then
    status=$(mutate POST "/api/v1/dispatch/recommendations")
    assert_status "$status" "200" "POST /api/v1/dispatch/recommendations (re-plan)"
    jq '.recommendations[0]' "$TMP/last.body" >"$TMP/envelope.json"
    status=$(mutate POST "/api/v1/dispatch/apply" "$(cat "$TMP/envelope.json")")
    assert_status "$status" "200" "POST /api/v1/dispatch/apply"
    attached=$(grep -o '"resources_attached":[0-9]*' "$TMP/last.body" | head -1 | cut -d':' -f2)
    [[ "${attached:-0}" -ge 1 ]] || { echo "FAIL: apply attached no resources"; exit 1; }
    echo "  ok  $attached resource(s) attached"
    status=$(get "/api/v1/incidents/$incident_id" "$TMP/last.body")
    assert_status "$status" "200" "GET /api/v1/incidents/{id}"
    grep -q 'DISPATCHED' "$TMP/last.body" \
        || { echo "FAIL: incident did not flip to DISPATCHED"; exit 1; }
    echo "  ok  incident flipped to DISPATCHED"
else
    echo "  skip  apply step requires jq (not installed)"
fi

echo "==> 11. Admin: simulation controls"
status=$(get "/api/v1/admin/simulation" "$TMP/last.body")
assert_status "$status" "200" "GET /api/v1/admin/simulation"
status=$(mutate POST "/api/v1/admin/simulation/pause")
assert_status "$status" "200" "POST /api/v1/admin/simulation/pause"
status=$(mutate POST "/api/v1/admin/simulation/resume")
assert_status "$status" "200" "POST /api/v1/admin/simulation/resume"
status=$(mutate POST "/api/v1/admin/simulation/inject" '{
  "title": "Injected crisis",
  "severity_level": 5,
  "affected_people": 500,
  "casualty_count": 15,
  "latitude": 24.8949,
  "longitude": 91.8687
}')
assert_status "$status" "201" "POST /api/v1/admin/simulation/inject"

echo "==> 12. Metrics endpoint"
status=$(get "/metrics" "$TMP/last.body")
assert_status "$status" "200" "GET /metrics"

echo
echo "==========================================="
echo "  ALL SMOKE TESTS PASSED — API is healthy"
echo "==========================================="
