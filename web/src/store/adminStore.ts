import { create } from "zustand";
import { z } from "zod";

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

interface AdminState {
	centers: Center[];
	incidents: Incident[];
	resources: Resource[];
	simStatus: SimulationStatus | null;
	isLoading: boolean;
	fetchData: () => Promise<void>;
	deleteIncident: (id: string) => Promise<boolean>;
	deleteResource: (id: string) => Promise<boolean>;
	deleteCenter: (id: string) => Promise<boolean>;
}

export const useAdminStore = create<AdminState>((set, get) => ({
	centers: [],
	incidents: [],
	resources: [],
	simStatus: null,
	isLoading: false,

	deleteIncident: async (id: string) => {
		try {
			const res = await fetch(`http://localhost:8080/api/v1/incidents/${id}`, {
				method: "DELETE",
				headers: { "x-role": "admin" },
			});
			if (res.ok || res.status === 204) {
				set({ incidents: get().incidents.filter((i) => i.id !== id) });
				return true;
			}
		} catch (_e) {
			// ignore
		}
		return false;
	},

	deleteResource: async (id: string) => {
		try {
			const res = await fetch(`http://localhost:8080/api/v1/resources/${id}`, {
				method: "DELETE",
				headers: { "x-role": "admin" },
			});
			if (res.ok || res.status === 204) {
				set({ resources: get().resources.filter((r) => r.id !== id) });
				return true;
			}
		} catch (_e) {
			// ignore
		}
		return false;
	},

	deleteCenter: async (id: string) => {
		try {
			const res = await fetch(`http://localhost:8080/api/v1/centers/${id}`, {
				method: "DELETE",
				headers: { "x-role": "admin" },
			});
			if (res.ok || res.status === 204) {
				set({ centers: get().centers.filter((c) => c.id !== id) });
				return true;
			}
		} catch (_e) {
			// ignore
		}
		return false;
	},

	fetchData: async () => {
		set({ isLoading: true });
		try {
			const headers = { "x-role": "admin" };
			const [centersRes, incidentsRes, resourcesRes, simRes] =
				await Promise.all([
					fetch("http://localhost:8080/api/v1/centers", { headers }),
					fetch("http://localhost:8080/api/v1/incidents", { headers }),
					fetch("http://localhost:8080/api/v1/resources", { headers }),
					fetch("http://localhost:8080/api/v1/admin/simulation", { headers }),
				]);

			let centers = [];
			let incidents = [];
			let resources = [];
			let simStatus = null;

			if (centersRes.ok) {
				const data = await centersRes.json();
				const parsed = z.array(CenterSchema).safeParse(data);
				if (parsed.success) centers = parsed.data;
				else console.error("Centers validation failed:", parsed.error);
			}

			if (incidentsRes.ok) {
				const data = await incidentsRes.json();
				const parsed = z.array(IncidentSchema).safeParse(data);
				if (parsed.success) incidents = parsed.data;
				else console.error("Incidents validation failed:", parsed.error);
			}

			if (resourcesRes.ok) {
				const data = await resourcesRes.json();
				const parsed = z.array(ResourceSchema).safeParse(data);
				if (parsed.success) resources = parsed.data;
				else console.error("Resources validation failed:", parsed.error);
			}

			if (simRes.ok) {
				const data = await simRes.json();
				const parsed = SimulationStatusSchema.safeParse(data);
				if (parsed.success) simStatus = parsed.data;
			}

			set({ centers, incidents, resources, simStatus, isLoading: false });
		} catch (error) {
			console.error("Failed to fetch admin data:", error);
			set({ isLoading: false });
		}
	},
}));
