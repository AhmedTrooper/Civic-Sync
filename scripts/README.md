# Scripts

Utility scripts for operating the CivicSync stack.

## `smoke.sh`

End-to-end smoke test for a **running** API. It walks 12 steps:
health probes, hub seeding (both `/centers` and the `/command-centers`
frontend alias), RBAC enforcement, incident create + Δ-casualty PATCH,
resource provisioning, the full dispatch pipeline
(`recommendations` → `apply` → incident flips to `DISPATCHED`),
simulation controls (status/pause/resume/inject), and the Prometheus
`/metrics` endpoint.

```bash
# API defaults to http://localhost:8080
./scripts/smoke.sh

# or point at another instance
API=http://staging.example.com ./scripts/smoke.sh
```

Exits non-zero on the first failed assertion. `jq` is optional but
unlocks the apply-and-verify step. The API can run in in-memory mode —
no database required.
