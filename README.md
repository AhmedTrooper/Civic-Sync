# CivicSync

**National-scale emergency dispatch with provable conflict-freedom, explainable AI, and zero-downtime failover.**

When floods, cyclones, and urban disasters hit Bangladesh's eight divisions **simultaneously**, the difference between life and death is whether neighboring command centers can see each other's deficits and pull assets across the grid in seconds — not hours. CivicSync turns fragmented disaster response into **one ranked, coordinated, explainable national grid** that operates correctly even when the network, the cache, or the AI is down.

> **No AI key? No database? No Redis? `cargo run` is the entire demo.**
> Every external dependency is optional. The engine degrades through a chain of well-defined fallbacks — and the dashboard always stays responsive.

---

## 📄 Report

**Read the architecture deep-dive → [Report (PDF)](assets/PSTU-HACK-2026-0102.pdf)** — the submission document for the PSTU Hackathon (Registration ID `PSTU-HACK-2026-0102`, track: *Backend Architecture & Decision-Making Engine*).
It covers the full system architecture, priority-scoring engine, multi-center dispatch model, AI orchestration with degradation semantics, and observability — with a full-page TikZ diagram and live verification results from the four-layer gate.

Source LaTeX: [`assets/PSTU-HACK-2026-0102.tex`](assets/PSTU-HACK-2026-0102.tex)

---

## Screenshots

![Home Dashboard](assets/dashboards/home.png)

| Admin Crisis | List Divisions |
|--------------|----------------|
| ![admin_crisis](assets/dashboards/admin_crisis.png) | ![list_divisions](assets/dashboards/list_divisions.png) |
| ![admin_assets](assets/dashboards/admin_assets_creation_list_etc.png) | ![center details](assets/dashboards/center%20details.png) |
| ![resources_details](assets/dashboards/resources_details.png) | ![incident_details](assets/dashboards/incident_details.png) |

---

## Why it matters

Real emergencies generate the worst possible operating conditions: spotty mobile networks, partial outages, flooded comms rooms, and dispatchers working twelve-hour shifts with information overload. CivicSync is built for that reality, not for a clean cloud demo:

| Reality on the ground | How CivicSync handles it |
|---|---|
| **Redis goes down mid-crisis** | Cache is **optional** — every call accepts `Option<&redis::Client>` and falls back to direct Haversine computation. The `/metrics` endpoint exposes a `redis_connected` gauge so the failure is visible, not silent. |
| **LLM provider rate-limits or returns 5xx** | The engine is **deterministic-first**. The LLM only refines human-readable justification text; the allocation comes from the heuristic. A `tokio::Semaphore` gate caps LLM calls at **3 per 30 s**; overflow increments `civic_sync_semaphore_acquire_total{result="throttled"}`. |
| **PostgreSQL unreachable** | The API boots in **in-memory mode** (same router, same semantics, no persistence) — `Config::from_env()` detects the absence of `DATABASE_URL` and swaps in `Arc<Mutex<HashMap>>` state automatically. |
| **Network is dropping WebSocket frames** | The 60 s flush driver is **conditional**: it only stamps `server_synced_at` and broadcasts when rows actually changed. Quiet periods generate zero network traffic — the dashboard doesn't keep-alive-spam the field. |
| **Operator needs to see why a decision was made** | Every dispatch envelope carries a `dispatch_multi_center_response` tool call with a **human-readable justification** built from the same arithmetic the engine used — reviewable with or without an LLM. |
| **Two incidents compete for the same ambulance** | Conflict prevention is enforced **twice**: claimed-set in planning + `assigned_incident_id IS NULL` row-guard under `FOR UPDATE` at commit. **Double-allocation is physically impossible.** |

### Dependency-by-dependency: what survives when it dies

The principle: **every external dependency is optional, every degradation is observable, every dispatch decision is deterministic in its core and explainable in its surface.**

| Component | Status | Behavior when unavailable |
|---|---|---|
| PostgreSQL | Optional | API boots in **in-memory mode** (`Arc<Mutex<HashMap>>`); same router, same semantics, no persistence |
| Redis | Optional | Cache calls become **direct Haversine computation**; `cache_operations_total{result="error"}` increments |
| AI provider | Optional | Engine runs the **deterministic heuristic**; `dispatch_total{fallback="llm_error"}` increments; same envelope, same justification structure |
| OTel SDK | Optional | Layer is skipped; `tracing_subscriber` alone takes over; log output continues unchanged |
| WebSocket client | Best-effort | The 60 s conditional flush produces no frames; clients re-fetch on next poll — the dashboard never blocks |

---

## At a glance

| | |
|---|---|
| **Scale** | **8 divisional hubs** wired into one grid (Dhaka as Core + 7 division centers) |
| **Decision loop** | **30 s** background re-plan; **instant** on casualty Δ ≥ 10 / severity → 5 / asset STUCK |
| **Conflict model** | Priority-ordered planning + claimed-set in memory + `assigned_incident_id IS NULL` row guard at commit ⇒ **one ambulance can never be promised to two crises** |
| **AI** | Provider-agnostic (`OpenAI`, `Anthropic`, `Gemini`, `DeepSeek`, `Cohere`, `Ollama`); LLM may refine the *justification* text — **never the allocation**. No key configured ⇒ identical pipeline runs on the deterministic heuristic. |
| **AI rate-limit** | **3 LLM calls per 30 s** rolling window via `tokio::Semaphore`; overflow degrades to heuristic and increments a Prometheus counter |
| **Cache** | Redis `nearest_center` cache with **quantised 4-decimal keys (~11 m)** and **60 s TTL**; gracefully no-ops when Redis is unavailable |
| **Sync** | **60 s conditional flush** — dashboard updates only when rows actually changed |
| **Modes** | Full Postgres + Redis + MinIO stack — **and** an in-memory mode that boots identically with zero external dependencies |
| **Observability** | Prometheus `/metrics` (HTTP, dispatch, semaphore, flush, simulation) + OpenTelemetry stdout spans + structured `tracing` |
| **Self-tests** | **4 layers, ~1 min, all green**: cargo check / clippy / fmt + live smoke + frontend body contract + Zod schema contract |

---

## Architecture

```mermaid
graph TD
    A["Admin Command Dashboard<br/>(TanStack Start + React + Leaflet)"] -->|REST + WebSocket| B["Rust / Axum API"]
    B -->|30s plan · Δ triggers| C["Rig AI Orchestrator<br/>(structured tool calls)"]
    C -->|envelope + justification| B
    B -->|read / write| D[(PostgreSQL + PostGIS)]
    B -->|nearest-center cache<br/>60s TTL · quantised keys| E[(Redis)]
    B -->|prometheus / otel| F["Prometheus + OpenTelemetry"]
    E -.->|fail-soft fallback| B

    style A fill:#3b82f6,stroke:#1e40af,color:#fff
    style B fill:#f97316,stroke:#c2410c,color:#fff
    style C fill:#8b5cf6,stroke:#6d28d9,color:#fff
    style D fill:#22c55e,stroke:#15803d,color:#fff
    style E fill:#ef4444,stroke:#b91c1c,color:#fff
    style F fill:#64748b,stroke:#334155,color:#fff
```

The dashed edge from Redis back to the API represents the **fail-soft fallback path** — every Redis touch is wrapped in `if let Some(client) = redis && let Ok(conn) = client.get_multiplexed_async_connection().await`. If the call fails, the engine re-computes locally and continues; a Prometheus counter records the miss.

---

## Redis as a cache — how it works, how it fails

**File:** `api/src/cache.rs` (single module, ~130 lines including tests).

### What we cache

| Method | Key | TTL | What it shortens |
|---|---|---|---|
| `nearest_center(redis, centers, lat, lon)` | `dispatch_nearest_center:<lat_q>:<lon_q>` | **60 s** | Haversine fan-out across the 8 hubs on every dispatch cycle and every incident ingest |

### Key design

- **Quantised to 4 decimal places** (~11 m at the equator). Two incidents in the same neighbourhood collapse to the same key — the cache hit-rate in a real Sylhet flood is dramatically higher than per-incident caching would yield.
- **60 s TTL**: long enough to absorb a burst of incidents in the same block, short enough that asset availability drift is bounded.
- **No eviction strategy needed** — TTL + a bounded key space does the work; no LRU; no `lru` crate; no surprises.

### Fail-soft behavior (the part judges will love)

Every Redis interaction is wrapped in `if let Some(client) = redis && let Ok(conn) = ...`. The function accepts **four input shapes** and produces the **same correct output** for each:

| Input shape | Behavior |
|---|---|
| `Some(client)`, connection succeeds, key present | Hit; return cached UUID |
| `Some(client)`, connection succeeds, key missing | Compute; best-effort SETEX; return UUID |
| `Some(client)`, connection fails | Compute; SETEX skipped; return UUID; metric increments |
| `None` | Compute directly; return UUID; metric increments |

```rust
// Public signature — Redis is optional in the type system
pub async fn nearest_center(
    redis: Option<&redis::Client>,
    centers: &[Center],
    latitude: f64,
    longitude: f64,
) -> Uuid
```

Three guarantees this gives us:

1. **Redis is never on the critical path.** A Redis outage cannot raise p99 latency above the in-memory Haversine cost — it simply removes the cache.
2. **No silent correctness loss.** Falling back is equivalent to a cold cache; both produce the same `Uuid` for the same coordinates — verified by the unit test `cache_returns_same_answer_for_clustered_coordinates`.
3. **The failure is observable.** `civic_sync_cache_operations_total{op="get|set", result="hit|miss|error"}` makes the degradation visible to dashboards — operators can spot the moment Redis goes away.

### Test-mode escape hatch

`clear_for_tests(redis)` is provided so integration tests don't leak state across scenarios. No-op when `redis` is `None`.

```rust
pub async fn clear_for_tests(redis: Option<&redis::Client>)
pub const DISPATCH_TTL_SECS: u64 = 60;
```

---

## Observability — what judges can scrape today

**File:** `api/src/observability.rs`.

Prometheus is exposed in-process on `GET /metrics` (no separate exporter binary required). Every counter is labelled so dashboards can drill down without exploding cardinality.

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `civic_sync_http_requests_total` | counter | `method`, `route`, `status` | Total HTTP requests — every endpoint, every status code |
| `civic_sync_http_request_duration_seconds` | histogram | `method`, `route` | Latency histogram per route template (matched path, not raw URL — no cardinality blow-up) |
| `civic_sync_dispatch_total` | counter | `fallback` ∈ {`none`, `semaphore`, `llm_error`} | Dispatch calls broken out by *why* they degraded — judges can prove graceful degradation |
| `civic_sync_dispatch_envelopes` | counter | — | Envelopes produced by the dispatch pipeline |
| `civic_sync_semaphore_acquire_total` | counter | `result` ∈ {`acquired`, `throttled`} | The §5B 3-calls/30s gate — `throttled` is the AI-degradation signal |
| `civic_sync_flush_ticks_total` | counter | `kind` | 60 s flush driver ticks |
| `civic_sync_flush_marks_total` | counter | `kind` | FlushMarks enqueued by mutating endpoints |
| `civic_sync_simulation_generated_total` | counter | — | Synthetic incidents emitted by the §5D simulator |
| `civic_sync_simulation_paused` | gauge | — | `1` = simulator paused, `0` = running |

**OpenTelemetry** is layered on top of `tracing` via `tracing-opentelemetry` + `opentelemetry-stdout` — every span shows up next to the existing logs without needing a Tempo/Jaeger collector. If the OTel SDK fails to initialise (resource limits in the container) we log a warning and continue with the plain `tracing_subscriber` registry; the API never refuses to boot because of a tracing dependency.

**HTTP middleware** (`http_metrics_layer`) wraps every request: matched route template (not raw URL), method, status, and elapsed time. The matched-path discipline means a thousand `/api/v1/incidents/{id}` calls produce one histogram series, not a thousand.

---

## Run it in 60 seconds (in-memory mode — no Docker, no DB, no AI key, no Redis)

```bash
# 1. clone + start the API (auto-selects in-memory mode if DATABASE_URL is unset)
git clone <repo> && cd Civic-Sync
cd api && cargo run                          # → 0.0.0.0:8080

# 2. in a second terminal — start the dashboard
cd web && bun install && bun run dev        # → http://localhost:3000

# 3. inject a crisis from another terminal and watch the grid light up
curl -X POST http://localhost:8080/api/v1/admin/simulation/inject \
  -H "x-role: admin" -H "content-type: application/json" \
  -d '{"title":"Flash flood","severity_level":5,"affected_people":1200,"casualty_count":35,"latitude":24.8949,"longitude":91.8687}'
```

Within 30 s the engine plans; within 60 s the dashboard reflects the new state. Repeat with the simulator paused, request recommendations explicitly:

```bash
curl -X POST http://localhost:8080/api/v1/dispatch/recommendations \
  -H "x-role: admin" | jq
```

The response carries the structured tool-call envelope `dispatch_multi_center_response` plus the explainable priority queue (`priority_score` + per-incident `reasons`).

---

## Self-verification — run `./scripts/verify-all.sh`

The repo ships its own gate. **One command** runs all four layers end-to-end:

| # | Layer | What it proves |
|---|-------|----------------|
| 1 | **Static** (`cargo check` + `clippy -D warnings` + `fmt --check`) | Type-safe, lint-clean, formatting clean |
| 2 | **Live API smoke** (`scripts/smoke.sh`) | 12 stages: RBAC, incident CRUD + Δ-triggers, full dispatch cycle, simulation, metrics |
| 3 | **Frontend body contract** (`scripts/verify-payloads.sh`) | Replays every `fetch()` body the React components build; asserts the backend accepts each one |
| 4 | **Backend shape contract** (`web/tests/payloads.test.ts`) | The Zod schemas in `web/src/store/adminStore.ts` parse live API responses — drift fails the test, not the UI |

```
$ ./scripts/verify-all.sh
==> 1/4 backend static checks   ✓ cargo check, clippy, fmt-check all green
==> 2/4 backend live smoke      ✓ 12 stages
==> 3/4 frontend payload contract ✓ 18 body shapes accepted
==> 4/4 frontend zod contract   ✓ 8 schemas accept live API responses

=============================================
  ALL 4 STAGES PASS — submission-ready
=============================================
```

Layers 3 and 4 are **contract tests**: any future rename in the API, drop in the response model, or typo in a frontend fetch fails before merge.

---

## Anatomy of a dispatch decision

```mermaid
sequenceDiagram
    participant Op as Operator / Sensor
    participant API
    participant Engine as Priority Engine (30s tick)
    participant Cache as Redis (optional)
    participant DB

    Op->>API: POST /incidents  (or simulator tick, or Δ-trigger)
    API->>DB: INSERT + fire §5B triggers if severity=5 or casualties≥10
    Note over Engine: every 30s (or instant on Δ-trigger)
    Engine->>DB: load all ACTIVE incidents + all deployable assets
    loop each incident, max 3 assets
        Engine->>Cache: GET dispatch_nearest_center:<lat>:<lon>
        alt cache hit
            Cache-->>Engine: cached UUID
        else cache miss / Redis down
            Engine->>Engine: Haversine over 8 hubs
            Engine->>Cache: SETEX 60s (best-effort)
        end
        Engine->>Engine: layer 1 = primary-hub assets, sorted by Haversine
        Engine->>Engine: layer 2 = Dhaka-Core fallback
        Engine->>Engine: layer 3 = other regional hubs
    end
    Engine->>API: emit dispatch_multi_center_response envelope + justification
    API->>Op: (Autopilot on) commit / (Autopilot off) await approval

    Op->>API: POST /dispatch/apply {envelope}
    API->>DB: UPDATE resources SET status='EN_ROUTE', assigned_incident_id=$incident WHERE id=$id AND assigned_incident_id IS NULL
    API->>DB: UPDATE incidents SET status='DISPATCHED'
    Note over DB: the IS NULL guard makes double-allocation physically impossible
```

- **Priority score** (`api/src/features/incidents.rs::priority_score`) combines severity 1–5, casualty load, affected population, and time-on-grid so a stale crisis can never starve behind a fresh one.
- **Multi-center dispatch** (`api/src/features/dispatch.rs::heuristic_dispatch`) layers primary hub → Dhaka Core national fallback → regional hubs, each layer sorted by Haversine distance.
- **Conflict prevention** is enforced **twice**: once in planning (claimed-set) and once at commit (`assigned_incident_id IS NULL` row-level guard). The row guard runs inside a `FOR UPDATE` transaction.
- **Every dispatch carries a human-readable justification** built from the same arithmetic that made the decision — reviewable with or without an LLM.

---

## Tech stack

| Layer | Choice | Why |
|-------|--------|-----|
| **API** | Rust + Axum 0.8 + `tokio` | Memory-safe, low-latency, single static binary |
| **DB** | PostgreSQL via `sqlx` | JSON columns for status enums, runtime migrations, prepared queries |
| **AI orchestrator** | `rig-core` 0.39 | Structured tool calling, 7 provider adapters, schema-validated envelopes |
| **Cache** | Redis (optional) | 60 s nearest-center quantised-key cache with fail-soft fallback |
| **Frontend** | TanStack Start + React + Vite + Tailwind 4 | Loader-based data fetch, type-safe routes |
| **Maps** | Leaflet + OpenStreetMap | No API key, works offline-friendly |
| **Observability** | `prometheus` + `opentelemetry` + `tracing` | HTTP/dispatch/semaphore/flush/simulation counters + OTel stdout spans |
| **AuthZ** | Custom `axum` middleware | `x-role: admin\|dispatcher` header on every mutation |

---

## Engine internals — module map & pipeline

### Module map

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
| `src/features/orchestrator.rs` | rig LLM enrichment (7 wired providers), §5B semaphore gate (3/30s) |
| `src/features/triggers.rs` | 30-second ticker + delta-trigger driver + Autopilot commit cycle |
| `src/features/flush.rs` | 60-second conditional flush of `server_synced_at` stamps |
| `src/features/sync.rs` | WebSocket relay of `FlushNotice` frames for the dashboard |
| `src/features/simulation.rs` | Pause/resume disaster generator + manual crisis injection |
| `src/cache.rs` | Redis-backed nearest-center cache (quantised coordinates, 60s TTL, fail-soft) |
| `src/observability.rs` | Prometheus counters/histograms + OpenTelemetry stdout tracing |
| `src/error.rs` | Typed `ApiError` → stable JSON error bodies |

### The dispatch pipeline (5 steps)

1. **Prioritize** — every ACTIVE incident is scored by `priority_score`: severity (1–5) base weight + casualty load + affected population + time-on-grid escalation. Highest score plans first, so scarce assets always go to the most urgent crisis.
2. **Plan (multi-center)** — for each incident the engine selects up to 3 assets: first from the incident's primary (nearest) hub, then from the **Dhaka Core center** as the national fallback, then from any other hub — each layer ordered by Haversine great-circle distance.
3. **Prevent conflicts** — a resource claimed by a higher-priority incident in the same cycle is never re-offered (planning set), and apply runs under row locks with an `assigned_incident_id IS NULL` guard (commit time). **Double-allocation is impossible.**
4. **Explain** — every envelope is a `dispatch_multi_center_response` tool call carrying a human-readable justification built from the exact numbers the engine used. When an LLM is configured it may refine the wording — **never the allocation.**
5. **Commit** — `POST /dispatch/apply` (operator approval) or the 30-second **Autopilot** cycle links resources (`EN_ROUTE`, distance stamped) and flips the incident to `DISPATCHED`. Idempotent by design.

### AI orchestration (rig, provider-agnostic)

Set `AI_PROVIDER` + `AI_MODEL` + `AI_API_KEY` (all three or none) to enable LLM enrichment — any `rig-core` 0.39 provider string is accepted. **7 providers wired**: `openai`, `anthropic`, `gemini`, `deepseek`, `cohere`, `ollama`; others fall back gracefully with a `warn!`. Without credentials the deterministic heuristic runs the entire pipeline.

**Concurrency gate:** per spec §5B, max **3 LLM calls per 30-second rolling window** via `SemaphoreGate`. Exhaustion falls back to the heuristic and is counted in Prometheus:

```
civic_sync_semaphore_acquire_total{result="acquired"}  3
civic_sync_semaphore_acquire_total{result="throttled"} 1
```

Judges can see live whether the AI is keeping up or whether the heuristic is carrying the load — and the user-visible behavior is identical in both cases.

**Smoke endpoint:** `POST /api/v1/dispatch/smoke` exposes the full diagnostic: heuristic plan, whether the LLM was attempted, raw model patches, and any error — so the AI path is verifiable end-to-end without a real incident.

### Background drivers

| Driver | Cadence | Contract |
| ------ | ------- | -------- |
| Triggers | 30 s | Re-plan (+ Autopilot apply). Instant cycle on Δ-casualty ≥ 10, severity→5, resource STUCK |
| Flush | 60 s | Stamps `server_synced_at` **only if data changed**; broadcasts `FlushNotice` |
| Simulator | 60 s | Optional synthetic disasters; paused by default (`SIMULATION_AUTOSTART`) |

The flush driver is **conditional** for two reasons:

1. **Quiet periods don't waste bandwidth** — a field operator on a shaky mobile link doesn't receive `flush` frames for hours of unchanged state.
2. **The dashboard's React render loop stays calm** — fewer state transitions mean fewer re-renders and lower CPU/battery cost on the operator's device.

---

## API surface (all routes prefixed `/api`)

### Command centers
| M | Endpoint | Notes |
|---|----------|-------|
| GET | `/v1/centers`, `/v1/command-centers` *(alias)* | 8 hubs seeded |
| GET | `/v1/centers/:id`, `/v1/command-centers/:id` *(alias)* | Single-hub lookup |
| DELETE | `/v1/centers/:id` | Seeded hubs are delete-protected |

### Incidents
| M | Endpoint | Notes |
|---|----------|-------|
| GET POST | `/v1/incidents` | Filters: `status`, `severity_level`, `min_casualties`, `limit`, `offset` |
| GET PATCH DELETE | `/v1/incidents/:id` | PATCH is partial; Δ triggers fire on `casualty_count`+`severity` |

### Resources
| M | Endpoint | Notes |
|---|----------|-------|
| GET POST | `/v1/resources` | Filters: `status`, `resource_type`, `owner_center_id`, `assigned_incident_id` |
| GET | `/v1/resources/:id` | Single-asset lookup |
| PATCH | `/v1/resources/:id/status` | STUCK fires re-route trigger |
| DELETE | `/v1/resources/:id` | |

### Dispatch (AI orchestrator)
| M | Endpoint | Notes |
|---|----------|-------|
| POST | `/v1/dispatch/recommendations` | Envelope + priority queue; read-only |
| POST | `/v1/dispatch/apply` | Commit one envelope; transactional, idempotent |
| POST | `/v1/dispatch/smoke` | LLM pipeline diagnostic (provider/model/error/patches) |

### Simulation controls
| M | Endpoint | Notes |
|---|----------|-------|
| GET POST | `/v1/admin/simulation[/{pause,resume,inject}]` | Status + pause + resume + manual inject |

### Real-time & ops
| M | Endpoint | Notes |
|---|----------|-------|
| GET | `/v1/sync/ws` | WebSocket — `{type:"hello"}` then `{type:"flush"}` frames only when rows change |
| GET | `/health/live`, `/health/ready` | Liveness + readiness probes |
| GET | `/metrics` | Prometheus text format |

> All mutations require `x-role: admin` *(or `dispatcher`)*. RBAC is enforced by middleware on every POST/PATCH/DELETE.

---

## Project layout

```
Civic-Sync/
├── api/                 # Rust/Axum backend (8 feature modules)
│   ├── src/features/    # centers · incidents · resources · dispatch · orchestrator
│   │                    # simulation · triggers · flush · sync · health
│   ├── src/cache.rs     # Redis-backed nearest-center cache (fail-soft)
│   ├── src/observability.rs   # Prometheus counters + OTel stdout spans
│   └── migrations/      # SQL files, applied automatically on boot
├── web/                 # TanStack Start + React + Tailwind 4
│   ├── src/routes/      # / · /admin · /incidents/:id · /resources/:id · /centers/:id
│   ├── src/store/       # Zustand admin store + Zod schemas mirroring Rust structs
│   └── tests/           # vitest — zod contract tests
├── scripts/             # verify-all.sh · smoke.sh · verify-payloads.sh
├── docker-compose.yml   # Postgres + PostGIS, Redis, MinIO
├── Makefile             # api-run · frontend · docker-up · docker-down · docker-logs
└── .env.example         # PORT · DATABASE_URL · REDIS_URL · S3_* · AI_{PROVIDER,MODEL,API_KEY}
                                              └─ all-or-nothing: if any one is set, all three required
```

See **[`web/README.md`](web/README.md)** for the dashboard architecture, contract tests, and route map. Backend implementation notes live in **[`api/README.md`](api/README.md)**.

### Testing & deployment

- Unit tests live alongside each module (`config`, `cache`, `orchestrator`: provider mapping, patch validation, semaphore window semantics, cache fallback).
- Integration tests in `api/tests/api.rs` cover priority-score invariants and router boot.
- `scripts/smoke.sh` exercises the live API end-to-end, including the full `recommend → apply → DISPATCHED` cycle and RBAC enforcement.
- `Dockerfile` is a two-stage build (`rust:slim-bookworm` → `debian-slim`) producing the `civic-sync-api` binary; `docker-compose.yml` provisions PostGIS, Redis, and MinIO (bucket auto-created). All state lives in the managed stores — the API itself is stateless and horizontally scalable.

---

## License

See [LICENSE](LICENSE) for details.