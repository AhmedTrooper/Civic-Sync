# CivicSync Web Dashboard

The operator-facing command dashboard for the CivicSync national
emergency grid. Built with **TanStack Start**, **React**, **Tailwind 4**,
and **Leaflet** — designed for a dispatcher in a divisional EOC staring
at a wall-mounted monitor during a 12-hour shift, not a marketing demo.

---

## What an operator sees

The dashboard is a **single-pane national view** of every active crisis,
every available asset, every command center, and every AI dispatch
recommendation pending approval. Three core surfaces:

1. **Live map** — Leaflet + OpenStreetMap showing the 8 divisional hubs
   (Dhaka core flagged distinctly), active incidents colour-coded by
   severity (1–5), and resource markers showing live status
   (`EN_ROUTE`, `STUCK`, `REJECTED`, `COMPLETED`).
2. **Active incident table** — sortable, filterable, with the explainable
   `priority_score` column so the dispatcher can see *why* one crisis
   is ranked above another.
3. **Admin / Simulation panel** — pause/resume the disaster generator,
   inject custom crises, drill into resource status, review and approve
   AI dispatch envelopes.

All three surfaces stay live via **conditional WebSocket flush frames**
— the dashboard re-renders only when rows actually change, so a quiet
shift doesn't burn battery on a tablet or CPU on a wall monitor.

---

## Pages

| Route | Page | Description |
| ----- | ---- | ----------- |
| `/` | Dashboard | Live overview: incident map, active crisis counters, resource status panels, priority-ranked table |
| `/admin` | Admin Panel | Simulation controls (pause/resume/inject), resource management, AI orchestration |
| `/admin/simulation` | Simulation Tab | Manual crisis injection + active incident table (the crisis-injection workbench) |
| `/admin/centers` | Centers Tab | Command-center grid with delete-protection surfaced to the operator |
| `/admin/assets` | Assets Tab | Resource inventory and live status changes (`STUCK` fires re-route trigger) |
| `/incidents/:incidentId` | Incident Details | Severity, casualty data, affected population, assigned resources, dispatch history |
| `/resources/:resourceId` | Resource Details | Single asset view: type, owner center, assignment, distance passed / remaining |
| `/centers/:centerId` | Center Details | Hub profile: lat/lon, core flag, fleet composition |

---

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│ TanStack Start (SSR + file-based router)                 │
│                                                          │
│   routes/  ──►  loaders (typed)  ──►  components          │
│       │                                                   │
│       ├── admin.tsx          ─► AdminLayout + tabs        │
│       ├── index.tsx          ─► Dashboard                 │
│       └── incidents.$id.tsx  ─► Incident Details          │
│                                                          │
│   store/adminStore.ts        ─► Zustand + Zod             │
│   components/admin/*         ─► Tables, Forms, Controls   │
│   hooks/*                    ─► Polling, auto-fill        │
└──────────────────┬───────────────────────────────────────┘
                   │ REST + WebSocket
                   ▼
             Rust / Axum API (see api/README.md)
```

### Data flow on the simulation tab

```
admin.simulation.tsx
  └─ useAdminStore((s) => s.isLoading)   ← pure selector, no side effects
  └─ <IncidentsTable loading={isLoading} />
       └─ useAdminStore()                ← per-slice selectors
       └─ useReactTable({ ... })         ← TanStack table
       └─ useTableSelection({ ... })     ← internal memoised callbacks
```

The store enforces **pure, single-subscription selectors** at the
boundary — every `useAdminStore` call returns one slice, never the
whole state — so re-renders are surgical and the dashboard stays
responsive even when the simulation is generating incidents every
second.

---

## Real-time strategy

### Why polling, not just WebSocket

The backend pushes `flush` frames only when data actually changed (see
the `flush.rs` module — the conditional 60 s flush). When a frame
arrives, the client refetches the affected slice. We poll on a short
interval as a **belt-and-braces fallback** for:

- Frame drops on shaky mobile networks
- Browser tab being backgrounded (WebSockets throttle)
- Network reconnects after a transient failure

The polling interval is configurable per-page; the WebSocket is the
optimistic path, the poll is the safety net.

### Why we don't render-loop

The dashboard uses **Zustand v5** on top of `useSyncExternalStore`.
Every store subscription uses a **pure, single-argument selector** —
never an inline arrow with side effects, never a whole-state
`useStore()`. This means React's commit phase never wedges on a
getSnapshot that itself triggers an update, the page hydrates
deterministically, and `useEffect`s fire on first mount as expected.

This is enforced by code review and verified by the **contract test
suite** — `web/tests/payloads.test.ts` runs the Zod schemas against
live API responses and fails on any drift.

---

## Features

- **Real-Time Polling** — Configurable per-page intervals keep
  dashboards synchronized even when the WebSocket is throttled.
- **OpenStreetMap Integration** — Leaflet maps render incident
  locations, command centers, and resource movements across Bangladesh
  with no API key and offline-friendly tile caching.
- **Dark Mode** — Full dark theme via Tailwind 4 dark variant; the
  default theme is dark because control rooms are dark.
- **Responsive Layouts** — Desktop monitoring stations and tablet
  field devices both render correctly.
- **TanStack Router** — File-based routing with type-safe navigation
  and SSR support via TanStack Start.
- **Zod Schema Mirroring** — Every response struct from the Rust API
  has a matching Zod schema in `web/src/store/adminStore.ts`. Any
  drift fails the test suite, not the UI.

---

## Contract Tests — the part judges should pay attention to

Two layers of contract testing lock the API ↔ UI boundary:

### 1. `web/tests/payloads.test.ts` — *Backend → Frontend*

Runs the Zod schemas in `adminStore.ts` against **live API responses**.
If the Rust struct gains a field, drops a field, or changes a type,
this test fails before the UI can break. 8 checks, all green.

### 2. `scripts/verify-payloads.sh` — *Frontend → Backend*

Replays every `fetch()` body the React components would build, hits
the live API, asserts acceptance. If the frontend sends a malformed
payload — a typo in a select option, a wrong status enum — this test
fails at the API boundary. 18 checks, all green.

Together these catch **any future rename in either direction** before
it reaches a user. This is the difference between "the demo worked
when we recorded it" and "the demo keeps working as we evolve".

---

## Tech Stack

| Technology | Purpose |
| ---------- | ------- |
| TanStack Start | SSR framework & server functions |
| TanStack Router | File-based type-safe routing |
| TanStack Table | Headless table primitives (admin tables) |
| React | UI component library |
| Zustand | Minimal store with `useSyncExternalStore` semantics |
| Zod | Runtime schema validation mirroring Rust response structs |
| TailwindCSS | Utility-first styling (dark-mode default) |
| Leaflet | Interactive map rendering |
| OpenStreetMap | Map tile provider (no API key) |
| Lucide React | Icon system |
| Bun | Package manager + dev server |

---

## Setup

### Prerequisites

- [Bun](https://bun.sh/) (v1.0+)
- Backend API running on `http://localhost:8080` (see
  [root README](../README.md) and [api/README.md](../api/README.md))

### Install Dependencies

```bash
bun install
```

### Development Server

```bash
bun run dev
```

The dashboard will be available at **http://localhost:3000**.

### Run Contract Tests

```bash
bun test                   # runs payloads.test.ts against a live API
```

Or use the repo-level gate (recommended):

```bash
./scripts/verify-all.sh    # stages 1–4, fail-fast
```

---

## Build

```bash
bun run build
```

Generates an optimized production build with SSR support.

---

## Project Structure

```
web/
├── src/
│   ├── components/
│   │   └── admin/         # Tables, forms, simulation controls
│   │       ├── forms/
│   │       │   └── InjectIncidentForm.tsx
│   │       ├── IncidentsTable.tsx
│   │       └── AdminLiveControls.tsx
│   ├── hooks/             # Custom React hooks (polling, auto-fill, table selection)
│   ├── lib/               # Utility functions and API clients
│   ├── routes/
│   │   ├── __root.tsx
│   │   ├── index.tsx                    # Dashboard (/)
│   │   ├── admin.tsx                    # Admin layout
│   │   ├── admin.simulation.tsx         # Simulation tab
│   │   ├── admin.centers.tsx
│   │   ├── admin.assets.tsx
│   │   ├── incidents.$incidentId.tsx
│   │   ├── resources.$resourceId.tsx
│   │   └── centers.$centerId.tsx
│   ├── store/
│   │   └── adminStore.ts  # Zustand + Zod (mirrors Rust response structs)
│   ├── router.tsx
│   ├── routeTree.gen.ts
│   └── styles.css
├── tests/
│   └── payloads.test.ts   # Zod ↔ live API contract
└── package.json
```

---

## Related

- [**Root README**](../README.md) — Full project overview, architecture,
  and local setup
- [**API README**](../api/README.md) — Backend engine, cache layer,
  observability