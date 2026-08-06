# CivicSync API

> **This README is a pointer.** The full architecture, engine pipeline,
> Redis cache deep-dive, observability catalogue, and endpoint table live
> in the **[root README](../README.md)**. This file only adds the
> module-level details a contributor needs to boot, configure, and
> extend the backend.

---

## Quick links

| Topic | See |
| ----- | --- |
| Why this API is built for emergencies (resilience table) | [root README → Dependency-by-dependency](../README.md#dependency-by-dependency-what-survives-when-it-dies) |
| Architecture diagram (mermaid) | [root README → Architecture](../README.md#architecture) |
| Redis cache — keys, TTL, fail-soft contract | [root README → Redis as a cache](../README.md#redis-as-a-cache--how-it-works-how-it-fails) |
| Observability — every Prometheus metric + OTel setup | [root README → Observability](../README.md#observability--what-judges-can-scrape-today) |
| Dispatch pipeline — 5 steps | [root README → The dispatch pipeline](../README.md#the-dispatch-pipeline-5-steps) |
| AI orchestration (rig, providers, semaphore gate) | [root README → AI orchestration](../README.md#ai-orchestration-rig-provider-agnostic) |
| Full endpoint table | [root README → API surface](../README.md#api-surface-all-routes-prefixed-api) |
| Module map | [root README → Module map](../README.md#module-map) |
| WebSocket / 60s flush behavior | [root README → Background drivers](../README.md#background-drivers) |
| Quickstart (`cargo run` in-memory) | [root README → Run it in 60 seconds](../README.md#run-it-in-60-seconds-in-memory-mode--no-docker-no-db-no-ai-key-no-redis) |

---

## What lives here that doesn't live in the root README

Just the contributor-facing bits: boot order, config, AI provider
wiring, and a couple of subtleties that only matter when you're
modifying the engine.

### Boot sequence (`src/main.rs`)

```
config::from_env()                → all-or-nothing AI validation, AUTOPILOT
   ↓
build pool (or skip → in-memory)  → DATABASE_URL unset ⇒ Arc<Mutex<HashMap>>
   ↓
sqlx::migrate!()                  → applies api/migrations/*.sql automatically
   ↓
observability::install()          → Prometheus recorder (idempotent)
observability::install_tracing()  → tracing + OTel stdout (best-effort)
   ↓
spawn background drivers          → triggers (30s) · flush (60s) · simulator
   ↓
axum listener on PORT (default 8080)
```

### Config (`src/config.rs`)

All env-driven. Notable flags:

| Var | Default | Effect |
| --- | ------- | ------ |
| `PORT` | `8080` | Bind port |
| `DATABASE_URL` | _unset_ | Unset ⇒ in-memory mode (no Postgres required) |
| `DATABASE_MAX_CONNECTIONS` | `10` | sqlx pool ceiling |
| `REDIS_URL` | _unset_ | Unset ⇒ `Option<None>` passed to every cache call ⇒ fail-soft |
| `S3_*` | _unset_ | MinIO config (object store, currently unused for the dispatch path) |
| `AI_PROVIDER` | _unset_ | `openai` \| `anthropic` \| `gemini` \| `deepseek` \| `cohere` \| `ollama` |
| `AI_MODEL` | _unset_ | Free-form provider-specific model string |
| `AI_API_KEY` | _unset_ | **All three AI vars required or all unset** (validated in `config.rs`) |
| `ALLOWED_ORIGINS` | _unset_ | CORS allow-list (unset ⇒ `AllowOrigin::any()`) |
| `AUTOPILOT` | `true` | When `false`, the 30 s driver **plans only**; commits require explicit `POST /dispatch/apply` (strict human-in-the-loop) |
| `SIMULATION_AUTOSTART` | _unset_ | If set, the simulator starts on boot; otherwise it stays paused |

`.env` is honoured via `dotenvy`.

### Provider fallback in the orchestrator (`src/features/orchestrator.rs`)

Any rig-core 0.39 provider string is accepted. The 7 wired above use
their native adapters; an unknown provider logs a `warn!` and the
heuristic carries the load — there is no fatal "unsupported provider"
branch.

### Concurrency gate (semaphore)

`SemaphoreGate` enforces **3 LLM calls per 30-second window** at the
orchestrator boundary. A caller blocked by the gate falls back to the
heuristic; the gate outcome is recorded as a labelled counter
(`acquired` / `throttled`) on `/metrics`.

### Smoke endpoint

`POST /api/v1/dispatch/smoke` runs the LLM pipeline end-to-end with a
synthetic 1-incident / 1-resource scenario and returns a structured
diagnostic — `{ heuristic_plan, llm_attempted, provider, model, patches,
error }`. Judges (and CI) use this to prove the AI path is alive
without depending on real incident data.

---

## Running locally

```bash
docker compose up -d          # Postgres (PostGIS) + Redis + MinIO
cp .env.example .env          # tweak if needed
cd api && cargo run           # migrations run automatically on boot
```

No database? The API boots in in-memory mode (same semantics, no
persistence). No AI keys? The heuristic engine runs everything. No
Redis? The cache degrades transparently; everything else keeps working.

Verify a running instance:

```bash
./scripts/smoke.sh            # 12-step end-to-end walk, exits non-zero on failure
```

```bash
cargo check && cargo clippy --all-targets && cargo fmt --check
./scripts/verify-all.sh       # 4-layer self-test (root README → Self-verification)
```