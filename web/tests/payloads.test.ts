// Live-API payload contract tests.
//
// These tests fetch from the running backend (defaults to
// http://localhost:8080) and validate the JSON shape against the Zod
// schemas in src/store/adminStore.ts. The whole point: if the frontend
// ever drifts from the backend (rename a field, delete a field,
// deserialize a typo'd enum), one of these tests will fail instead of
// silently rendering "undefined" in the UI.
//
// Run:
//   1. DATABASE_URL="" PORT=8080 ./target/debug/civic-sync-api   (in another shell)
//   2. cd web && bun test tests/payloads.test.ts
//
// Override the API host with API env var, e.g.
//   API=http://localhost:8080 bun test tests/payloads.test.ts

import { describe, it, expect } from 'vitest';
import {
	CenterSchema,
	IncidentSchema,
	ResourceSchema,
	SimulationStatusSchema,
} from '../src/store/adminStore';

const API = (process.env.API as string | undefined) ?? 'http://localhost:8080';
const headers = { 'x-role': 'admin' };

async function getJson(path: string) {
	const res = await fetch(`${API}${path}`, { headers });
	if (!res.ok) {
		throw new Error(
			`GET ${path} failed: ${res.status} ${await res.text()}`,
		);
	}
	return res.json();
}

describe('API response contracts', () => {
	it('GET /api/v1/centers returns Center[] matching schema', async () => {
		const data = await getJson('/api/v1/centers');
		expect(Array.isArray(data)).toBe(true);
		expect(data.length).toBeGreaterThanOrEqual(8);
		const parsed = data.map((c: unknown) => CenterSchema.parse(c));
		expect(parsed.length).toBe(data.length);
		// Every center has an id, name, lat/lng, and the timestamps.
		for (const center of parsed) {
			expect(center.id).toMatch(/^[0-9a-f-]{36}$/);
			expect(typeof center.name).toBe('string');
			expect(typeof center.is_core_center).toBe('boolean');
		}
	});

	it('GET /api/v1/incidents returns Incident[] matching schema', async () => {
		const data = await getJson('/api/v1/incidents');
		expect(Array.isArray(data)).toBe(true);
		const parsed = data.map((i: unknown) => IncidentSchema.parse(i));
		for (const incident of parsed) {
			expect(incident.severity_level).toBeGreaterThanOrEqual(1);
			expect(incident.severity_level).toBeLessThanOrEqual(5);
			expect(['ACTIVE', 'DISPATCHED', 'RESOLVED']).toContain(
				incident.status,
			);
		}
	});

	it('GET /api/v1/resources returns Resource[] matching schema', async () => {
		const data = await getJson('/api/v1/resources');
		expect(Array.isArray(data)).toBe(true);
		const parsed = data.map((r: unknown) => ResourceSchema.parse(r));
		for (const resource of parsed) {
			expect(['EN_ROUTE', 'STUCK', 'REJECTED', 'COMPLETED']).toContain(
				resource.status,
			);
			expect([
				'AMBULANCE',
				'BOAT',
				'HELICOPTER',
				'RELIEF_TRUCK',
				'FOOD_PACK',
				'WATER_SUPPLY',
				'SHELTER_KIT',
				'MEDICAL_RATION',
			]).toContain(resource.resource_type);
		}
	});

	it('GET /api/v1/admin/simulation returns SimulationStatus matching schema', async () => {
		const data = await getJson('/api/v1/admin/simulation');
		const parsed = SimulationStatusSchema.parse(data);
		expect(typeof parsed.paused).toBe('boolean');
		expect(parsed.generated).toBeGreaterThanOrEqual(0);
	});

	it('Filter query params used by frontend are accepted', async () => {
		const centers = await getJson('/api/v1/centers');
		const first = centers[0];
		expect(first).toBeDefined();
		const filtered = await getJson(
			`/api/v1/resources?owner_center_id=${first.id}`,
		);
		expect(Array.isArray(filtered)).toBe(true);
		for (const r of filtered) {
			ResourceSchema.parse(r);
		}
	});

	it('Backend rejects unknown IncidentStatus enum values', async () => {
		const res = await fetch(`${API}/api/v1/incidents?status=BOGUS`, {
			headers,
		});
		expect(res.status).toBe(400);
	});

	it('Backend rejects unknown ResourceStatus enum values', async () => {
		const res = await fetch(`${API}/api/v1/resources?status=BOGUS`, {
			headers,
		});
		expect(res.status).toBe(400);
	});

	it('RBAC: POST without role is forbidden', async () => {
		const res = await fetch(`${API}/api/v1/incidents`, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({
				title: 'rbac',
				severity_level: 1,
				affected_people: 1,
				casualty_count: 0,
				latitude: 23.8,
				longitude: 90.4,
			}),
		});
		expect(res.status).toBe(403);
	});
});
