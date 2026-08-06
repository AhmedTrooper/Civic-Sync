# CivicSync

**National-scale emergency dispatch with provable conflict-freedom and explainable AI.**

When floods, cyclones, and urban disasters hit Bangladesh's eight divisions simultaneously, neighboring command centers cannot see each other's deficits — and ambulances, helicopters, boats, and relief supplies get dispatched by instinct. CivicSync turns fragmented disaster response into **one ranked, coordinated, explainable national grid**.

---

## At a glance

| | |
|---|---|
| **Scale** | **8 divisional hubs** wired into one grid (Dhaka as Core + 7 division centers) |
| **Decision loop** | **30 s** background re-plan; **instant** on casualty Δ ≥ 10 / severity → 5 / asset STUCK |
| **Conflict model** | Priority-ordered planning + claimed-set in memory + `assigned_incident_id IS NULL` row guard at commit ⇒ **one ambulance can never be promised to two crises** |
| **AI** | Provider-agnostic (`OpenAI`, `Anthropic`, `Gemini`, `DeepSeek`, `Cohere`, `Ollama`); LLM may refine the *justification* text — **never the allocation**. No key configured ⇒ identical pipeline runs on the deterministic heuristic. |
| **AI rate-limit** | **3 LLM calls per 30 s** rolling window via `tokio::Semaphore`; overflow degrades to heuristic and increments a Prometheus counter |
| **Sync** | **60 s** conditional flush — dashboard updates only when rows actually changed |
| **Modes** | Full Postgres + Redis + MinIO stack — **and** an in-memory mode that boots identically with zero external dependencies (see quickstart below) |
| **Self-tests** | **4 layers, ~1 min, all green**: cargo check / clippy / fmt + live smoke + frontend body contract + Zod schema contract |

> **No AI key? No database? `cargo run` is the entire demo.**

---

## Architecture

```mermaid
graph TD
    A["Admin Command Dashboard<br/>(TanStack Start + React + Leaflet)"] -->|REST + WebSocket| B["Rust / Axum API"]
    B -->|30s plan · Δ triggers| C["Rig AI Orchestrator<br/>(structured tool calls)"]
    C -->|envelope + justification| B
    B -->|read / write| D[(PostgreSQL)]
    B -->|nearest-center cache| E[(Redis)]
    B -->|prometheus / otel| F["Prometheus + OpenTelemetry"]

    style A fill:#3b82f6,stroke:#1e40af,color:#fff
    style B fill:#f97316,stroke:#c2410c,color:#fff
    style C fill:#8b5cf6,stroke:#6d28d9,color:#fff
    style D fill:#22c55e,stroke:#15803d,color:#fff
    style E fill:#ef4444,stroke:#b91c1c,color:#fff
    style F fill:#64748b,stroke:#334155,color:#fff
```

---

## Run it in 60 seconds (in-memory mode — no Docker, no DB, no AI key)

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
    participant DB

    Op->>API: POST /incidents  (or simulator tick, or Δ-trigger)
    API->>DB: INSERT + fire §5B triggers if severity=5 or casualties≥10
    Note over Engine: every 30s (or instant on Δ-trigger)
    Engine->>DB: load all ACTIVE incidents + all deployable assets
    Engine->>Engine: sort by priority_score = severity·10 + casualties·0.5 + affected·0.02 + time
    loop each incident, max 3 assets
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
| **Cache** | Redis (optional) | 60 s nearest-center quantised-key cache |
| **Frontend** | TanStack Start + React + Vite + Tailwind 4 | Loader-based data fetch, type-safe routes |
| **Maps** | Leaflet + OpenStreetMap | No API key, works offline-friendly |
| **Observability** | `prometheus` + `opentelemetry` | HTTP/dispatch/semaphore/flush/simulation counters + traces |
| **AuthZ** | Custom `axum` middleware | `x-role: admin\|dispatcher` header on every mutation |

---

## API surface (all routes prefixed `/api`)

### Command centers
| M | Endpoint | Notes |
|---|----------|-------|
| GET | `/v1/centers`, `/v1/command-centers` *(alias)* | 8 hubs seeded; only `/centers/:id` has DELETE (seeded hubs are protected) |

### Incidents
| M | Endpoint | Notes |
|---|----------|-------|
| GET POST | `/v1/incidents` | Filters: `status`, `severity_level`, `min_casualties`, `limit`, `offset` |
| GET PATCH DELETE | `/v1/incidents/:id` | PATCH is partial; Δ triggers fire on `casualty_count`+`severity` |

### Resources
| M | Endpoint | Notes |
|---|----------|-------|
| GET POST | `/v1/resources` | Filters: `status`, `resource_type`, `owner_center_id`, `assigned_incident_id` |
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

### Real-time
| M | Endpoint | Notes |
|---|----------|-------|
| GET | `/v1/sync/ws` | WebSocket — `{type:"hello"}` then `{type:"flush"}` frames only when rows change |

> All mutations require `x-role: admin` *(or `dispatcher`)*. RBAC is enforced by middleware on every POST/PATCH/DELETE.

---

## Project layout

```
Civic-Sync/
├── api/                 # Rust/Axum backend (8 feature modules)
│   ├── src/features/    # centers · incidents · resources · dispatch · orchestrator
│   │                    # simulation · triggers · flush · sync · health
│   └── migrations/      # 8 SQL files, applied automatically on boot
├── web/                 # TanStack Start + React + Tailwind 4
│   ├── src/routes/      # / · /admin · /incidents/:id · /resources/:id · /centers/:id
│   ├── src/store/       # Zustand admin store + Zod schemas mirroring Rust structs
│   └── tests/           # vitest — zod contract tests
├── scripts/             # verify-all.sh · smoke.sh · verify-payloads.sh
├── docker-compose.yml   # Postgres + Redis + MinIO
├── Makefile             # api-run · frontend · docker-up · docker-down · docker-logs
└── .env.example         # PORT · DATABASE_URL · REDIS_URL · S3_* · AI_{PROVIDER,MODEL,API_KEY}
                                              └─ all-or-nothing: if any one is set, all three required
```

See **[`api/README.md`](api/README.md)** for the engine internals and **[`web/README.md`](web/README.md)** for the dashboard pages.

---

## License

See [LICENSE](LICENSE) for details.
