#!/usr/bin/env bash
# Verify that every payload the web frontend sends is accepted by the API.
# This re-issues each admin.tsx fetch with the exact JSON body the React
# component would build, then asserts the status. Run against a live API
# at $API (defaults to http://localhost:8080) — start with:
#   DATABASE_URL="" PORT=8080 ./target/debug/civic-sync-api

set -euo pipefail

API="${API:-http://localhost:8080}"
ADMIN="x-role: admin"
CT="content-type: application/json"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

ok()    { echo "  ok    $*"; }
fail()  { echo "  FAIL  $*" >&2; cat "$TMP/last.body" >&2 || true; exit 1; }

mutate() { # mutate <method> <path> [body]
    local m="$1" p="$2" b="${3:-}"
    if [[ -n "$b" ]]; then
        curl -s -o "$TMP/last.body" -w "%{http_code}" -X "$m" \
            -H "$CT" -H "$ADMIN" -d "$b" "$API$p"
    else
        curl -s -o "$TMP/last.body" -w "%{http_code}" -X "$m" \
            -H "$ADMIN" "$API$p"
    fi
}

get() { # get <path>
    curl -s -o "$TMP/last.body" -w "%{http_code}" "$API$1"
}

extract_id() {
    # Picks the first 'id' from either a list response or single object.
    python3 -c "import json; d=json.load(open('$TMP/last.body')); (d if isinstance(d, list) else [d]) ; items = d if isinstance(d, list) else [d]; print((items[0].get('id') if items else '') or '')"
}

echo "=== Health ==="
st=$(get "/health/live");      [[ "$st" == "200" ]] && ok "GET /health/live"      || fail "/health/live $st"
st=$(get "/health/ready");     [[ "$st" == "200" ]] && ok "GET /health/ready"     || fail "/health/ready $st"

echo "=== Seeded centers ==="
st=$(get "/api/v1/centers");   [[ "$st" == "200" ]] && ok "GET /api/v1/centers"   || fail "/centers $st"
CID=$(extract_id); [[ -n "$CID" ]] && ok "got CID=$CID" || fail "no centers"
st=$(get "/api/v1/command-centers"); [[ "$st" == "200" ]] && ok "GET /api/v1/command-centers (alias)" || fail "alias $st"

echo "=== admin.tsx Inject custom crisis ==="
# This is the body sent by handleSubmitIncident in web/src/routes/admin.tsx
st=$(mutate POST "/api/v1/admin/simulation/inject" '{
  "title":"Flash flood in Sylhet",
  "severity_level":5,
  "affected_people":1500,
  "casualty_count":50,
  "latitude":24.8949,
  "longitude":91.8687
}')
[[ "$st" == "201" ]] && ok "POST /admin/simulation/inject (admin.tsx body)" || fail "inject $st"

echo "=== admin.tsx Provision asset ==="
# Exact body sent by handleSubmitResource in admin.tsx
st=$(mutate POST "/api/v1/resources" "{
  \"unit_identifier\":\"MED-01\",
  \"resource_type\":\"AMBULANCE\",
  \"latitude\":23.8103,
  \"longitude\":90.4125,
  \"total_capacity\":1,
  \"owner_center_id\":\"$CID\"
}")
[[ "$st" == "201" ]] && ok "POST /api/v1/resources (admin.tsx body)" || fail "resource create $st"
RID=$(extract_id); ok "RID=$RID"

echo "=== admin.tsx saveIncidentUpdate (PATCH incident) ==="
# Get an existing incident to patch
curl -s -o "$TMP/last.body" "$API/api/v1/incidents"
IID=$(extract_id)
[[ -n "$IID" ]] || fail "no incidents to patch"
st=$(mutate PATCH "/api/v1/incidents/$IID" '{
  "severity_level":4,
  "affected_people":120,
  "casualty_count":12,
  "status":"DISPATCHED"
}')
[[ "$st" == "200" ]] && ok "PATCH /api/v1/incidents/{id} (admin.tsx body)" || fail "incident patch $st"

echo "=== admin.tsx saveResourceUpdate (PATCH resource /status) ==="
st=$(mutate PATCH "/api/v1/resources/$RID/status" '{
  "status":"STUCK",
  "current_capacity":1
}')
[[ "$st" == "200" ]] && ok "PATCH /api/v1/resources/{id}/status (admin.tsx body)" || fail "resource patch $st"

echo "=== admin.tsx toggleSimulation ==="
st=$(mutate POST "/api/v1/admin/simulation/pause")
[[ "$st" == "200" ]] && ok "POST /admin/simulation/pause" || fail "pause $st"
st=$(mutate POST "/api/v1/admin/simulation/resume")
[[ "$st" == "200" ]] && ok "POST /admin/simulation/resume" || fail "resume $st"

echo "=== resources.\$resourceId.tsx handleStatusUpdate body ==="
st=$(mutate PATCH "/api/v1/resources/$RID/status" '{
  "status":"EN_ROUTE",
  "current_capacity":4
}')
[[ "$st" == "200" ]] && ok "PATCH /resources/{id}/status (resource page body)" || fail "resource PATCH $st"

echo "=== Filter query strings used by frontend ==="
st=$(get "/api/v1/resources?owner_center_id=$CID")
[[ "$st" == "200" ]] && ok "GET /resources?owner_center_id=X (center page)" || fail "filter $st"

st=$(get "/api/v1/resources?assigned_incident_id=$IID")
[[ "$st" == "200" ]] && ok "GET /resources?assigned_incident_id=X (incident page)" || fail "filter $st"

st=$(get "/api/v1/centers?is_core_center=true")
[[ "$st" == "200" ]] && ok "GET /centers?is_core_center=true" || fail "center filter $st"

echo "=== Reject-the-broken-stuff checks ==="
# The OLD admin.tsx used the typo value="STUCK font-bold" — that path should now
# never trigger in the UI, but the backend must still reject it.
st=$(mutate PATCH "/api/v1/resources/$RID/status" '{"status":"STUCK font-bold"}')
[[ "$st" == "422" ]] && ok "backend rejects bogus status enum (proves typo was wrong)" || fail "expected 422 got $st"

# Available status should also fail since enum does not have it
st=$(get "/api/v1/resources?status=AVAILABLE")
[[ "$st" == "400" ]] && ok "backend rejects AVAILABLE (proves AVAILABLE removed from frontend colors)" || fail "expected 400 got $st"

echo
echo "============================================="
echo "  ALL FRONTEND PAYLOADS VERIFIED OK"
echo "============================================="
