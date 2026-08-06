import { z } from "zod";
import { create } from "zustand";

import { ApiError, apiFetch } from "#/lib/apiClient.ts";

// Strict Zod schemas that mirror the Rust response models in api/src/features/*.
// Any field drift between web and api is caught here: if a field is renamed
// on the backend, the page logs "validation failed" instead of silently
// showing "undefined" in the UI. Keep these in sync with the corresponding
// `pub struct` in api/src/features/{centers,incidents,resources,simulation}.rs.

export const IncidentStatusEnum = z.enum(["ACTIVE", "DISPATCHED", "RESOLVED"]);
export const ResourceStatusEnum = z.enum([
	"EN_ROUTE",
	"STUCK",
	"REJECTED",
	"COMPLETED",
]);
export const ResourceTypeEnum = z.enum([
	"AMBULANCE",
	"BOAT",
	"HELICOPTER",
	"RELIEF_TRUCK",
	"FOOD_PACK",
	"WATER_SUPPLY",
	"SHELTER_KIT",
	"MEDICAL_RATION",
]);

export const CenterSchema = z
	.object({
		id: z.string().uuid(),
		name: z.string(),
		is_core_center: z.boolean(),
		latitude: z.number(),
		longitude: z.number(),
		created_at: z.string(),
		updated_at: z.string(),
		server_synced_at: z.string().nullable(),
	})
	.strict();

export const IncidentSchema = z
	.object({
		id: z.string().uuid(),
		title: z.string(),
		primary_center_id: z.string().uuid(),
		severity_level: z.number().int().min(1).max(5),
		affected_people: z.number().int().nonnegative(),
		casualty_count: z.number().int().nonnegative(),
		latitude: z.number().min(-90).max(90),
		longitude: z.number().min(-180).max(180),
		status: IncidentStatusEnum,
		created_at: z.string(),
		updated_at: z.string(),
		server_synced_at: z.string().nullable(),
	})
	.strict();

export const ResourceSchema = z
	.object({
		id: z.string().uuid(),
		owner_center_id: z.string().uuid(),
		assigned_incident_id: z.string().uuid().nullable(),
		resource_type: ResourceTypeEnum,
		unit_identifier: z.string(),
		status: ResourceStatusEnum,
		distance_passed_km: z.number(),
		distance_remaining_km: z.number(),
		latitude: z.number(),
		longitude: z.number(),
		total_capacity: z.number().int(),
		current_capacity: z.number().int(),
		created_at: z.string(),
		updated_at: z.string(),
		server_synced_at: z.string().nullable(),
	})
	.strict();

export const SimulationStatusSchema = z
	.object({
		paused: z.boolean(),
		generated: z.number().int().nonnegative(),
		tick_interval_seconds: z.number().int().positive(),
	})
	.strict();

export type Center = z.infer<typeof CenterSchema>;
export type Incident = z.infer<typeof IncidentSchema>;
export type Resource = z.infer<typeof ResourceSchema>;
export type SimulationStatus = z.infer<typeof SimulationStatusSchema>;

export const PAGE_SIZE = 25;

export interface IncidentFilters {
	search: string;
	severity: number | "ALL";
	status: Incident["status"] | "ALL";
	page: number;
}

export interface ResourceFilters {
	search: string;
	type: Resource["resource_type"] | "ALL";
	status: Resource["status"] | "ALL";
	ownerCenterId: string | "ALL";
	page: number;
}

export interface CenterFilters {
	search: string;
	isCore: boolean | "ALL";
	page: number;
}

export const emptyIncidentFilters: IncidentFilters = {
	search: "",
	severity: "ALL",
	status: "ALL",
	page: 1,
};

export const emptyResourceFilters: ResourceFilters = {
	search: "",
	type: "ALL",
	status: "ALL",
	ownerCenterId: "ALL",
	page: 1,
};

export const emptyCenterFilters: CenterFilters = {
	search: "",
	isCore: "ALL",
	page: 1,
};

export interface AssignResourceInput {
	status: Resource["status"];
	assigned_incident_id: string | null;
	distance_passed_km?: number;
	distance_remaining_km?: number;
	current_capacity?: number;
}

export interface UpdateIncidentInput {
	severity_level?: number;
	affected_people?: number;
	casualty_count?: number;
	status?: Incident["status"];
}

export interface UpdateResourceInput {
	status?: Resource["status"];
	current_capacity?: number;
}

export interface InjectIncidentInput {
	title: string;
	severity_level: number;
	affected_people: number;
	casualty_count: number;
	latitude: number;
	longitude: number;
}

export interface CreateResourceInput {
	owner_center_id: string;
	resource_type: Resource["resource_type"];
	unit_identifier: string;
	latitude: number;
	longitude: number;
	total_capacity: number;
}

interface AdminState {
	centers: Center[];
	incidents: Incident[];
	resources: Resource[];
	simStatus: SimulationStatus | null;
	isLoading: boolean;
	wsConnected: boolean;

	incidentFilters: IncidentFilters;
	resourceFilters: ResourceFilters;
	centerFilters: CenterFilters;

	fetchData: () => Promise<void>;
	deleteIncident: (id: string) => Promise<boolean>;
	deleteResource: (id: string) => Promise<boolean>;
	deleteCenter: (id: string) => Promise<boolean>;

	updateIncident: (
		id: string,
		patch: UpdateIncidentInput,
	) => Promise<Incident | null>;
	updateResource: (
		id: string,
		patch: UpdateResourceInput,
	) => Promise<Resource | null>;
	assignResource: (
		id: string,
		input: AssignResourceInput,
	) => Promise<Resource | null>;
	createResource: (input: CreateResourceInput) => Promise<Resource | null>;
	injectIncident: (input: InjectIncidentInput) => Promise<Incident | null>;
	toggleSimulation: () => Promise<boolean>;

	setIncidentFilters: (patch: Partial<IncidentFilters>) => void;
	setResourceFilters: (patch: Partial<ResourceFilters>) => void;
	setCenterFilters: (patch: Partial<CenterFilters>) => void;

	setWsConnected: (connected: boolean) => void;
	upsertCenter: (center: Center) => void;
	upsertIncident: (incident: Incident) => void;
	upsertResource: (resource: Resource) => void;
	removeIncident: (id: string) => void;
	removeResource: (id: string) => void;
	removeCenter: (id: string) => void;
}

function errorMessage(err: unknown): string {
	if (err instanceof ApiError) return err.message;
	if (err instanceof Error) return err.message;
	return String(err);
}

async function safeApi<T>(
	path: string,
	init?: Parameters<typeof apiFetch>[1],
): Promise<T | null> {
	try {
		return await apiFetch<T>(path, init);
	} catch (err) {
		console.error(`[apiClient] ${path} failed:`, errorMessage(err));
		throw err;
	}
}

export const useAdminStore = create<AdminState>((set, get) => ({
	centers: [],
	incidents: [],
	resources: [],
	simStatus: null,
	isLoading: false,
	wsConnected: false,

	incidentFilters: emptyIncidentFilters,
	resourceFilters: emptyResourceFilters,
	centerFilters: emptyCenterFilters,

	setWsConnected: (connected) => set({ wsConnected: connected }),

	upsertCenter: (center) =>
		set((s) => {
			const idx = s.centers.findIndex((c) => c.id === center.id);
			if (idx === -1) return { centers: [...s.centers, center] };
			const next = s.centers.slice();
			next[idx] = center;
			return { centers: next };
		}),
	upsertIncident: (incident) =>
		set((s) => {
			const idx = s.incidents.findIndex((i) => i.id === incident.id);
			if (idx === -1) return { incidents: [...s.incidents, incident] };
			const next = s.incidents.slice();
			next[idx] = incident;
			return { incidents: next };
		}),
	upsertResource: (resource) =>
		set((s) => {
			const idx = s.resources.findIndex((r) => r.id === resource.id);
			if (idx === -1) return { resources: [...s.resources, resource] };
			const next = s.resources.slice();
			next[idx] = resource;
			return { resources: next };
		}),
	removeIncident: (id) =>
		set((s) => ({ incidents: s.incidents.filter((i) => i.id !== id) })),
	removeResource: (id) =>
		set((s) => ({ resources: s.resources.filter((r) => r.id !== id) })),
	removeCenter: (id) =>
		set((s) => ({ centers: s.centers.filter((c) => c.id !== id) })),

	deleteIncident: async (id) => {
		try {
			await safeApi(`/api/v1/incidents/${id}`, { method: "DELETE" });
			get().removeIncident(id);
			return true;
		} catch (err) {
			console.error("[admin] deleteIncident failed", errorMessage(err));
			return false;
		}
	},

	deleteResource: async (id) => {
		try {
			await safeApi(`/api/v1/resources/${id}`, { method: "DELETE" });
			get().removeResource(id);
			return true;
		} catch (err) {
			console.error("[admin] deleteResource failed", errorMessage(err));
			return false;
		}
	},

	deleteCenter: async (id) => {
		try {
			await safeApi(`/api/v1/centers/${id}`, { method: "DELETE" });
			get().removeCenter(id);
			return true;
		} catch (err) {
			console.error("[admin] deleteCenter failed", errorMessage(err));
			return false;
		}
	},

	updateIncident: async (id, patch) => {
		const updated = await safeApi<Incident>(`/api/v1/incidents/${id}`, {
			method: "PATCH",
			body: JSON.stringify(patch),
		});
		if (updated) {
			const parsed = IncidentSchema.safeParse(updated);
			if (parsed.success) get().upsertIncident(parsed.data);
			return parsed.success ? parsed.data : null;
		}
		return null;
	},

	updateResource: async (id, patch) => {
		const updated = await safeApi<Resource>(`/api/v1/resources/${id}/status`, {
			method: "PATCH",
			body: JSON.stringify(patch),
		});
		if (updated) {
			const parsed = ResourceSchema.safeParse(updated);
			if (parsed.success) get().upsertResource(parsed.data);
			return parsed.success ? parsed.data : null;
		}
		return null;
	},

	assignResource: async (id, input) => {
		const updated = await safeApi<Resource>(`/api/v1/resources/${id}/status`, {
			method: "PATCH",
			body: JSON.stringify(input),
		});
		if (updated) {
			const parsed = ResourceSchema.safeParse(updated);
			if (parsed.success) get().upsertResource(parsed.data);
			return parsed.success ? parsed.data : null;
		}
		return null;
	},

	createResource: async (input) => {
		const created = await safeApi<Resource>("/api/v1/resources", {
			method: "POST",
			body: JSON.stringify(input),
		});
		if (created) {
			const parsed = ResourceSchema.safeParse(created);
			if (parsed.success) get().upsertResource(parsed.data);
			return parsed.success ? parsed.data : null;
		}
		return null;
	},

	injectIncident: async (input) => {
		const created = await safeApi<Incident>("/api/v1/admin/simulation/inject", {
			method: "POST",
			body: JSON.stringify(input),
		});
		if (created) {
			const parsed = IncidentSchema.safeParse(created);
			if (parsed.success) get().upsertIncident(parsed.data);
			return parsed.success ? parsed.data : null;
		}
		return null;
	},

	toggleSimulation: async () => {
		const { simStatus } = get();
		if (!simStatus) return false;
		const endpoint = simStatus.paused
			? "/api/v1/admin/simulation/resume"
			: "/api/v1/admin/simulation/pause";
		try {
			await safeApi(endpoint, { method: "POST" });
			set({
				simStatus: { ...simStatus, paused: !simStatus.paused },
			});
			return true;
		} catch (err) {
			console.error("[admin] toggleSimulation failed", errorMessage(err));
			return false;
		}
	},

	setIncidentFilters: (patch) =>
		set((s) => ({
			incidentFilters: {
				...s.incidentFilters,
				...patch,
				page:
					patch.page ??
					(patch.search !== undefined ||
					patch.severity !== undefined ||
					patch.status !== undefined
						? 1
						: s.incidentFilters.page),
			},
		})),
	setResourceFilters: (patch) =>
		set((s) => ({
			resourceFilters: {
				...s.resourceFilters,
				...patch,
				page:
					patch.page ??
					(patch.search !== undefined ||
					patch.type !== undefined ||
					patch.status !== undefined ||
					patch.ownerCenterId !== undefined
						? 1
						: s.resourceFilters.page),
			},
		})),
	setCenterFilters: (patch) =>
		set((s) => ({
			centerFilters: {
				...s.centerFilters,
				...patch,
				page:
					patch.page ??
					(patch.search !== undefined || patch.isCore !== undefined
						? 1
						: s.centerFilters.page),
			},
		})),

	fetchData: async () => {
		console.log("[adminStore] fetchData() triggered");
		set({ isLoading: true });
		try {
			console.log(
				"[adminStore] Fetching centers, incidents, resources, simulation...",
			);
			const [centersRes, incidentsRes, resourcesRes, simRes] =
				await Promise.all([
					safeApi<unknown[]>("/api/v1/centers"),
					safeApi<unknown[]>("/api/v1/incidents"),
					safeApi<unknown[]>("/api/v1/resources"),
					safeApi<unknown>("/api/v1/admin/simulation"),
				]);

			let centers: Center[] = [];
			let incidents: Incident[] = [];
			let resources: Resource[] = [];
			let simStatus: SimulationStatus | null = null;

			if (Array.isArray(centersRes)) {
				const parsed = z.array(CenterSchema).safeParse(centersRes);
				if (parsed.success) centers = parsed.data;
				else console.error("Centers validation failed:", parsed.error);
			}
			if (Array.isArray(incidentsRes)) {
				const parsed = z.array(IncidentSchema).safeParse(incidentsRes);
				if (parsed.success) incidents = parsed.data;
				else console.error("Incidents validation failed:", parsed.error);
			}
			if (Array.isArray(resourcesRes)) {
				const parsed = z.array(ResourceSchema).safeParse(resourcesRes);
				if (parsed.success) resources = parsed.data;
				else console.error("Resources validation failed:", parsed.error);
			}
			if (simRes) {
				const parsed = SimulationStatusSchema.safeParse(simRes);
				if (parsed.success) simStatus = parsed.data;
			}

			console.log("[adminStore] fetchData() success, setting state", {
				centers: centers.length,
				incidents: incidents.length,
				resources: resources.length,
				simStatus,
			});
			set({ centers, incidents, resources, simStatus, isLoading: false });
		} catch (error) {
			console.error(
				"[adminStore] Failed to fetch admin data:",
				errorMessage(error),
				error,
			);
			set({ isLoading: false });
		}
	},
}));
