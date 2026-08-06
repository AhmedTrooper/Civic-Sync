# CivicSync API

The Rust/Axum backend that powers the CivicSync national emergency grid:
continuous incident ingestion, dynamic prioritization, AI-orchestrated
multi-center resource dispatch, and real-time dashboard synchronization —
with full explainability and human-in-the-loop controls.

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
        PostgreSQL (sqlx)   Redis (geo cache)
        + migrations        + Pub/Sub
```

## Module Map

| Module                | Responsibility                                                                 |
| --------------------- | ------------------------------------------------------------------------------ |
| `src/main.rs`         | Boot: config → pool → migrations → drivers → listener, graceful shutdown       |
| `src/config.rs`       | Env-driven config; all-or-nothing AI block validation; `AUTOPILOT` flag        |
| `src/app.rs`          | Router, CORS, metrics/trace layers, RBAC-guarded `/api/v1` nest                |
| `src/auth.rs`         | Header-based RBAC: mutations require `x-role: admin` or `dispatcher`           |
| `src/state.rs`        | Shared `AppState` (pool or in-memory maps, orchestrator, driver channels)      |
| `src/features/incidents.rs`   | Incident CRUD + PATCH deltas, priority scoring, Δ-trigger fan-out      |
| `src/features/resources.rs`   | Resource CRUD + status hook (`STUCK` fires re-route trigger)           |
| `src/features/centers.rs`     | The 8 divisional hubs (Dhaka core), seeded, delete-protected           |
| `src/features/dispatch.rs`    | Multi-center planning engine + explainable queue + transactional apply |
| `src/features/orchestrator.rs`| rig LLM enrichment (6 wired providers), §5B semaphore gate (3/30s)     |
| `src/features/triggers.rs`    | 30-second ticker + delta-trigger driver + Autopilot commit cycle       |
| `src/features/flush.rs`       | 60-second conditional flush of `server_synced_at` stamps               |
| `src/features/sync.rs`        | WebSocket relay of `FlushNotice` frames for the dashboard              |
| `src/features/simulation.rs`  | Pause/resume disaster generator + manual crisis injection              |
| `src/cache.rs`        | Redis-backed nearest-center cache (quantised coordinates, 60s TTL)     |
| `src/observability.rs`| Prometheus counters/histograms + OpenTelemetry stdout tracing          |
| `src/error.rs`        | Typed `ApiError` → stable JSON error bodies                            |

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

## AI Orchestration (rig, provider-agnostic)

Set `AI_PROVIDER` + `AI_MODEL` + `AI_API_KEY` (all three or none) to
enable LLM enrichment — any rig-core 0.39 provider string is accepted
(openai, anthropic, gemini, deepseek, cohere, ollama are wired; others
fall back gracefully). Without credentials the deterministic heuristic
runs the entire pipeline, so a bare `cargo run` is fully functional.

Concurrency is gated per spec: **max 3 LLM calls per 30-second window**
(`SemaphoreGate`); exhaustion falls back to the heuristic and is
counted in Prometheus (`civic_sync_semaphore_acquire_total`).

`POST /api/v1/dispatch/smoke` exposes the full diagnostic: heuristic
plan, whether the LLM was attempted, raw model patches, and any error —
so the AI path is verifiable end-to-end.

## Background Drivers

| Driver     | Cadence | Contract                                                                                  |
| ---------- | ------- | ----------------------------------------------------------------------------------------- |
| Triggers   | 30 s    | Re-plan (+ Autopilot apply). Instant cycle on Δ-casualty ≥ 10, severity→5, resource STUCK |
| Flush      | 60 s    | Stamps `server_synced_at` **only if data changed**; broadcasts `FlushNotice`              |
| Simulator  | 60 s    | Optional synthetic disasters; paused by default (`SIMULATION_AUTOSTART`)                  |

## Endpoints

All mutating endpoints require the header `x-role: admin` (or
`dispatcher`); viewers are read-only.

| Method | Path                                    | Notes                                     |
| ------ | --------------------------------------- | ----------------------------------------- |
| GET    | `/health/live`, `/health/ready`         | Probes (ready pings the DB when present)  |
| GET    | `/metrics`                              | Prometheus text format                    |
| GET    | `/api/v1/centers` (+`command-centers`)  | Filters: `name`, `is_core_center`, paging |
| GET    | `/api/v1/centers/{id}`                  | Also under `/command-centers/{id}`        |
| DELETE | `/api/v1/centers/{id}`                  | Seeded hubs are delete-protected          |
| GET    | `/api/v1/incidents`                     | Filters: `status`, `severity_level`, `min_casualties`, paging |
| POST   | `/api/v1/incidents`                     | Auto-assigns nearest primary hub          |
| GET    | `/api/v1/incidents/{id}`                |                                           |
| PATCH  | `/api/v1/incidents/{id}`                | Partial deltas fire §5B triggers          |
| DELETE | `/api/v1/incidents/{id}`                |                                           |
| GET    | `/api/v1/resources`                     | Filters: `status`, `resource_type`, `owner_center_id`, `assigned_incident_id`, paging |
| POST   | `/api/v1/resources`                     |                                           |
| GET    | `/api/v1/resources/{id}`                |                                           |
| PATCH  | `/api/v1/resources/{id}/status`         | `STUCK` fires the re-route trigger        |
| DELETE | `/api/v1/resources/{id}`                |                                           |
| POST   | `/api/v1/dispatch/recommendations`      | AI plan + explainable priority queue      |
| POST   | `/api/v1/dispatch/apply`                | Commit one operator-reviewed envelope     |
| POST   | `/api/v1/dispatch/smoke`                | LLM pipeline diagnostic                   |
| GET    | `/api/v1/admin/simulation`              | Simulator status                          |
| POST   | `/api/v1/admin/simulation/pause`        |                                           |
| POST   | `/api/v1/admin/simulation/resume`       |                                           |
| POST   | `/api/v1/admin/simulation/inject`       | Manual crisis injection                   |
| GET    | `/api/v1/sync/ws`                       | WebSocket: `hello` + 60s `flush` frames   |

## Running Locally

```bash
docker compose up -d          # Postgres (PostGIS) + Redis + MinIO
cp .env.example .env          # tweak if needed
cd api && cargo run           # migrations run automatically on boot
```

No database? The API boots in in-memory mode (same semantics, no
persistence) — perfect for a quick demo. No AI keys? The heuristic
engine runs everything.

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
```

## Deployment

`Dockerfile` is a two-stage build (rust:slim-bookworm → debian-slim)
producing the `civic-sync-api` binary; `docker-compose.yml` provisions
PostGIS, Redis, and MinIO (bucket auto-created). All state lives in the
managed stores — the API itself is stateless and horizontally scalable.
