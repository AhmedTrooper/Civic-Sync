# CivicSync Web Dashboard

The frontend application for the CivicSync emergency response platform, built with **TanStack Start**, **React**, and **TailwindCSS**.

---

## Overview

This is the admin command dashboard that provides real-time visibility into nationwide emergency operations. Operators can monitor active incidents across 8 divisional hubs, review AI dispatch recommendations, manage resources, and control the simulation engine — all through an interactive map-driven interface.

---

## Pages

| Route                      | Page              | Description                                                       |
| -------------------------- | ----------------- | ----------------------------------------------------------------- |
| `/`                        | Dashboard         | Live overview with incident map, active crisis counters, and resource status panels |
| `/admin`                   | Admin Panel       | Simulation controls (pause/resume/inject), resource management, and AI orchestration settings |
| `/incidents/:incidentId`   | Incident Details  | Deep-dive into a specific incident with severity, casualty data, assigned resources, and dispatch history |

---

## Features

- **Real-Time Polling** — Automatic data refresh with configurable intervals to keep dashboards synchronized with the backend state.
- **OpenStreetMap Integration** — Interactive Leaflet maps displaying incident locations, command center positions, and resource movements across Bangladesh.
- **Dark Mode** — Full dark theme support using Tailwind CSS dark variant with slate color palette.
- **Responsive Design** — Optimized layouts for desktop monitoring stations and tablet field devices.
- **TanStack Router** — File-based routing with type-safe navigation and SSR support via TanStack Start.
- **Lucide Icons** — Consistent icon system across all UI components.

---

## Tech Stack

| Technology         | Purpose                            |
| ------------------ | ---------------------------------- |
| TanStack Start     | SSR framework & server functions   |
| TanStack Router    | File-based type-safe routing       |
| React              | UI component library               |
| TailwindCSS        | Utility-first styling              |
| Leaflet            | Interactive map rendering          |
| OpenStreetMap      | Map tile provider                  |
| Lucide React       | Icon library                       |

---

## Setup

### Prerequisites

- [Bun](https://bun.sh/) (v1.0+)
- Backend API running on `http://localhost:8080` (see [root README](../README.md))

### Install Dependencies

```bash
bun install
```

### Development Server

```bash
bun run dev
```

The dashboard will be available at **http://localhost:3000**.

---

## Build

```bash
bun run build
```

Generates an optimized production build with SSR support.

---

## Test

```bash
bun test
```

---

## Project Structure

```
web/
├── src/
│   ├── components/      # Reusable UI components
│   ├── data/            # Static data and constants
│   ├── hooks/           # Custom React hooks
│   ├── lib/             # Utility functions and API clients
│   ├── routes/          # TanStack Router file-based routes
│   │   ├── __root.tsx   # Root layout with navigation
│   │   ├── index.tsx    # Dashboard page (/)
│   │   ├── admin.tsx    # Admin panel (/admin)
│   │   └── incidents.$incidentId.tsx  # Incident details
│   ├── store/           # State management
│   ├── router.tsx       # Router configuration
│   ├── routeTree.gen.ts # Auto-generated route tree
│   └── styles.css       # Global styles and Tailwind directives
└── package.json
```

---

## Related

- [**Root README**](../README.md) — Full project overview, architecture, and local setup
- [**API README**](../api/README.md) — Backend API documentation
