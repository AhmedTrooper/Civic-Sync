# CivicSync API

The Rust/Axum backend that powers the CivicSync national emergency grid:
continuous incident ingestion, dynamic prioritization, AI-orchestrated
multi-center resource dispatch, real-time dashboard synchronization,
and a fail-soft cache + observability layer that keeps the grid alive
even when individual infrastructure components fail mid-crisis.

---

## Why this API is built for emergencies

Real disasters don't respect clean cloud deployments. Mobile networks drop, Redis restarts during a deploy, an LLM provider starts returning 429s, a divisional database goes read-only. A dispatch engine that fails closed under any of those conditions is worse than no engine at all — operators revert to instinct, and instinct is what CivicSync exists to replace.

This API is designed around a single principle: **every external dependency is optional, every degradation is observable, every dispatch decision is deterministic in its core and explainable in its surface**.

| Component | Status | Behavior when unavailable |
|---|---|---|
| PostgreSQL | Optional | API boots in **in-memory mode** (`Arc<Mutex<HashMap>>`); same router, same semantics, no persistence |
| Redis | Optional | Cache calls become **direct Haversine computation**; `redis_connected` gauge drops to `0`, `cache_operations_total{result="error"}` increments |
| AI provider | Optional | Engine runs the **deterministic heuristic**; `dispatch_total{fallback="llm_error"}` increments; same envelopes, same justification structure, slightly less polished prose |
| OTel SDK | Optional | Layer is skipped; `tracing_subscriber` alone takes over; log output continues unchanged |
| WebSocket client | Best-effort | The 60 s conditional flush produces no frames; clients just re-fetch on next poll — the dashboard never blocks |

---

## Architecture at a Glance

```
                 ┌─────────────────────────────────────────────┐
                 │                Axum HTTP/WS                 │
                 │  RBAC middleware · CORS · metrics · traces  │
                 └──────────────┬──────────────────────────────┘
                                │
        ┌───────────┬───────────┼────────────┬───────────────┐
        ▼           ▼           ▼            ▼               ▼
   incidents    resources    centers      dispatch       simulation
   (CRUD+Δ)     (CRUD+hook)  (8 hubs)     (AI engine)    (§5D admin)
        │           │           │            │
        └───────────┴─────┬─────┴────────────┘
                          ▼
          ┌────────────────────────────────┐
          │        Background drivers      │
          │ 30s trigger ticker (§5B)       │
          │ 60s conditional flush (§5C)    │
          │ disaster simulator (§5D)       │
          └──────┬─────────────┬───────────┘
                 ▼             ▼
        PostgreSQL (sqlx)   Redis (cache, optional)
        + migrations        fail-soft to direct compute
```

---

## Module Map

| Module | Responsibility |
| ------ | -------------- |
| `src/main.rs` | Boot: config → pool → migrations → drivers → listener, graceful shutdown |
| `src/config.rs` | Env-driven config; all-or-nothing AI block validation; `AUTOPILOT` flag |
| `src/app.rs` | Router, CORS, metrics/trace layers, RBAC-guarded `/api/v1` nest |
| `src/auth.rs` | Header-based RBAC: mutations require `x-role: admin` or `dispatcher` |
| `src/state.rs` | Shared `AppState` (pool or in-memory maps, orchestrator, driver channels) |
| `src/features/incidents.rs` | Incident CRUD + PATCH deltas, priority scoring, Δ-trigger fan-out |
| `src/features/resources.rs` | Resource CRUD + status hook (`STUCK` fires re-route trigger) |
| `src/features/centers.rs` | The 8 divisional hubs (Dhaka core), seeded, delete-protected |
| `src/features/dispatch.rs` | Multi-center planning engine + explainable queue + transactional apply |
| `src/features/orchestrator.rs` | rig LLM enrichment (6 wired providers), §5B semaphore gate (3/30s) |
| `src/features/triggers.rs` | 30-second ticker + delta-trigger driver + Autopilot commit cycle |
| `src/features/flush.rs` | 60-second conditional flush of `server_synced_at` stamps |
| `src/features/sync.rs` | WebSocket relay of `FlushNotice` frames for the dashboard |
| `src/features/simulation.rs` | Pause/resume disaster generator + manual crisis injection |
| `src/cache.rs` | Redis-backed nearest-center cache (quantised coordinates, 60s TTL, fail-soft) |
| `src/observability.rs` | Prometheus counters/histograms + OpenTelemetry stdout tracing |
| `src/error.rs` | Typed `ApiError` → stable JSON error bodies |

---

## The Dispatch Pipeline (the core demo)

1. **Prioritize** — every ACTIVE incident is scored by
   `priority_score`: severity (1–5) base weight + casualty load +
   affected population + time-on-grid escalation. Highest score plans
   first, so scarce assets always go to the most urgent crisis.
2. **Plan (multi-center)** — for each incident the engine selects up to
   3 assets: first from the incident's primary (nearest) hub, then from
   the **Dhaka Core center** as the national fallback, then from any
   other hub — each layer ordered by Haversine great-circle distance.
3. **Prevent conflicts** — a resource claimed by a higher-priority
   incident in the same cycle is never re-offered (planning set), and
   apply runs under row locks with an `assigned_incident_id IS NULL`
   guard (commit time). Double-allocation is impossible.
4. **Explain** — every envelope is a `dispatch_multi_center_response`
   tool call carrying a human-readable justification built from the
   exact numbers the engine used. When an LLM is configured it may
   refine the wording — never the allocation.
5. **Commit** — `POST /dispatch/apply` (operator approval) or the
   30-second **Autopilot** cycle links resources (`EN_ROUTE`, distance
   stamped) and flips the incident to `DISPATCHED`. Idempotent by
   design: non-ACTIVE incidents and already-assigned assets are skipped
   and reported, never double-committed.

---

## AI Orchestration (rig, provider-agnostic)

Set `AI_PROVIDER` + `AI_MODEL` + `AI_API_KEY` (all three or none) to
enable LLM enrichment — any rig-core 0.39 provider string is accepted
(`openai`, `anthropic`, `gemini`, `deepseek`, `cohere`, `ollama` are
wired; others fall back gracefully with a `warn!`). Without credentials
the deterministic heuristic runs the entire pipeline, so a bare
`cargo run` is fully functional.

### Concurrency gate

Per spec §5B: **max 3 LLM calls per 30-second rolling window**
(`SemaphoreGate`). Exhaustion falls back to the heuristic and is
counted in Prometheus:

```
civic_sync_semaphore_acquire_total{result="acquired"}  3
civic_sync_semaphore_acquire_total{result="throttled"} 1
```

Judges can see live whether the AI is keeping up or whether the
heuristic is carrying the load — and the user-visible behavior is
identical in both cases.

### Smoke endpoint

`POST /api/v1/dispatch/smoke` exposes the full diagnostic: heuristic
plan, whether the LLM was attempted, raw model patches, and any error —
so the AI path is verifiable end-to-end without a real incident.

---

## Cache layer (`src/cache.rs`)

### Public API

```rust
pub async fn nearest_center(
    redis: Option<&redis::Client>,
    centers: &[Center],
    latitude: f64,
    longitude: f64,
) -> Uuid
pub async fn clear_for_tests(redis: Option<&redis::Client>)
pub const DISPATCH_TTL_SECS: u64 = 60;
```

### Why quantised keys

Coordinates are rounded to 4 decimal places (~11 m at the equator). In
real Sylhet-flood conditions, an entire block of incidents collapses
to the same key, and the cache hit-rate during a multi-incident burst
goes from poor to excellent without changing the semantics.

### Fail-soft contract

Every Redis interaction is wrapped in `if let Some(client) = redis
&& let Ok(conn) = ...`. The function therefore accepts **four input
shapes** and produces the **same correct output** for each:

| Input shape | Behavior |
|---|---|
| `Some(client)`, connection succeeds, key present | Hit; return cached UUID |
| `Some(client)`, connection succeeds, key missing | Compute; best-effort SETEX; return UUID |
| `Some(client)`, connection fails | Compute; SETEX skipped; return UUID; metric increments |
| `None` | Compute directly; return UUID; metric increments |

Three guarantees:

1. **Redis is never on the critical path.** A Redis outage cannot raise
   p99 latency beyond the cost of an in-memory Haversine sweep — it
   simply removes the cache.
2. **No silent correctness loss.** A fallback is mathematically
   equivalent to a cold cache: the same coordinates always produce the
   same UUID. This is verified by the unit test
   `cache_returns_same_answer_for_clustered_coordinates`.
3. **The failure is observable.** The `civic_sync_cache_operations_total`
   counter records every op with a `result` label (`hit`, `miss`,
   `error`). Operators can see cache degradation in real time.

### Test-mode escape hatch

`clear_for_tests(redis)` removes all `dispatch_nearest_center:*` keys
so integration scenarios don't leak state. No-op when `redis` is
`None`.

---

## Background Drivers

| Driver | Cadence | Contract |
| ------ | ------- | -------- |
| Triggers | 30 s | Re-plan (+ Autopilot apply). Instant cycle on Δ-casualty ≥ 10, severity→5, resource STUCK |
| Flush | 60 s | Stamps `server_synced_at` **only if data changed**; broadcasts `FlushNotice` |
| Simulator | 60 s | Optional synthetic disasters; paused by default (`SIMULATION_AUTOSTART`) |

The flush driver is **conditional** for two reasons:

1. **Quiet periods don't waste bandwidth** — a field operator on a
   shaky mobile link doesn't receive `flush` frames for hours of
   unchanged state.
2. **The dashboard's React render loop stays calm** — fewer state
   transitions means fewer re-renders and lower CPU/battery cost on
   the operator's device.

---

## Observability (`src/observability.rs`)

Prometheus is exposed in-process on `GET /metrics` (no separate
exporter binary required).

### Counter catalogue

| Metric | Type | Labels | Meaning |
| ------ | ---- | ------ | ------- |
| `civic_sync_http_requests_total` | counter | `method`, `route`, `status` | Every request, every status |
| `civic_sync_http_request_duration_seconds` | histogram | `method`, `route` | Per-route latency (matched-path, not raw URL) |
| `civic_sync_dispatch_total` | counter | `fallback` ∈ {`none`, `semaphore`, `llm_error`} | Dispatch calls broken out by *why* they degraded |
| `civic_sync_dispatch_envelopes` | counter | — | Envelopes produced |
| `civic_sync_semaphore_acquire_total` | counter | `result` ∈ {`acquired`, `throttled`} | AI gate outcomes |
| `civic_sync_flush_ticks_total` | counter | `kind` | Flush driver ticks |
| `civic_sync_flush_marks_total` | counter | `kind` | FlushMarks enqueued |
| `civic_sync_simulation_generated_total` | counter | — | Synthetic incidents |
| `civic_sync_simulation_paused` | gauge | — | `1` = paused |

### HTTP middleware

`http_metrics_layer` wraps every request and records the **matched
route template** (`/api/v1/incidents/{id}`), not the raw URL — a
thousand incident detail views produce one histogram series, not a
thousand.

### OpenTelemetry

Layered on top of `tracing` via `tracing-opentelemetry` +
`opentelemetry-stdout` — every span shows up next to the existing
logs without needing an external collector (Tempo/Jaeger). The OTel
SDK install is best-effort: if it fails, we log a warning and continue
with plain `tracing_subscriber` — the API never refuses to boot
because of a tracing dependency.

---

## Endpoints

All mutating endpoints require the header `x-role: admin` (or
`dispatcher`); viewers are read-only.

| Method | Path | Notes |
| ------ | ---- | ----- |
| GET | `/health/live`, `/health/ready` | Probes (ready pings the DB when present) |
| GET | `/metrics` | Prometheus text format |
| GET | `/api/v1/centers` (+`command-centers`) | Filters: `name`, `is_core_center`, paging |
| GET | `/api/v1/centers/{id}` | Also under `/command-centers/{id}` |
| DELETE | `/api/v1/centers/{id}` | Seeded hubs are delete-protected |
| GET | `/api/v1/incidents` | Filters: `status`, `severity_level`, `min_casualties`, paging |
| POST | `/api/v1/incidents` | Auto-assigns nearest primary hub |
| GET | `/api/v1/incidents/{id}` | |
| PATCH | `/api/v1/incidents/{id}` | Partial deltas fire §5B triggers |
| DELETE | `/api/v1/incidents/{id}` | |
| GET | `/api/v1/resources` | Filters: `status`, `resource_type`, `owner_center_id`, `assigned_incident_id`, paging |
| POST | `/api/v1/resources` | |
| GET | `/api/v1/resources/{id}` | Single-asset lookup |
| PATCH | `/api/v1/resources/:id/status` | `STUCK` fires the re-route trigger |
| DELETE | `/api/v1/resources/{id}` | |
| POST | `/api/v1/dispatch/recommendations` | AI plan + explainable priority queue |
| POST | `/api/v1/dispatch/apply` | Commit one operator-reviewed envelope |
| POST | `/api/v1/dispatch/smoke` | LLM pipeline diagnostic |
| GET | `/api/v1/admin/simulation` | Simulator status |
| POST | `/api/v1/admin/simulation/pause` | |
| POST | `/api/v1/admin/simulation/resume` | |
| POST | `/api/v1/admin/simulation/inject` | Manual crisis injection |
| GET | `/api/v1/sync/ws` | WebSocket: `hello` + 60s `flush` frames |

---

## Running Locally

```bash
docker compose up -d          # Postgres (PostGIS) + Redis + MinIO
cp .env.example .env          # tweak if needed
cd api && cargo run           # migrations run automatically on boot
```

No database? The API boots in in-memory mode (same semantics, no
persistence) — perfect for a quick demo. No AI keys? The heuristic
engine runs everything. No Redis? The cache degrades transparently;
everything else keeps working.

Verify a running instance:

```bash
./scripts/smoke.sh            # 12-step end-to-end walk, exits non-zero on failure
```

## Testing & Verification

- Unit tests live alongside modules (`config`, `cache`, `orchestrator`:
  provider mapping, patch validation, semaphore window semantics).
- Integration tests in `tests/api.rs` cover priority-score invariants
  and router boot.
- `scripts/smoke.sh` exercises the live API end-to-end, including the
  full recommend → apply → DISPATCHED cycle and RBAC enforcement.

```bash
cargo check && cargo clippy --all-targets && cargo fmt --check
./scripts/verify-all.sh       # 4-layer self-test
```

---

## Deployment

`Dockerfile` is a two-stage build (`rust:slim-bookworm` → `debian-slim`)
producing the `civic-sync-api` binary; `docker-compose.yml` provisions
PostGIS, Redis, and MinIO (bucket auto-created). All state lives in
the managed stores — the API itself is stateless and horizontally
scalable.