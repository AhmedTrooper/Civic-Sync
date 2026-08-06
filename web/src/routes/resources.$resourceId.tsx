import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useEffect, useState, lazy, Suspense } from "react";
import { API_BASE } from "#/lib/apiClient.ts";
import {
	ArrowLeft,
	MapPin,
	Truck,
	AlertTriangle,
	Loader2,
	Package,
	Navigation,
	Building2,
	AlertCircle,
	CheckCircle2,
	RefreshCw,
	Trash2,
} from "lucide-react";

const ResourceMap = lazy(() => import("#/components/ResourceMap"));

interface Resource {
	id: string;
	owner_center_id: string;
	assigned_incident_id: string | null;
	resource_type: string;
	unit_identifier: string;
	status: string;
	distance_passed_km: number;
	distance_remaining_km: number;
	latitude: number;
	longitude: number;
	total_capacity: number;
	current_capacity: number;
	created_at: string;
	updated_at: string;
}

interface Center {
	id: string;
	name: string;
	latitude: number;
	longitude: number;
}

interface Incident {
	id: string;
	title: string;
	severity_level: number;
	latitude: number;
	longitude: number;
}

interface LoaderData {
	resource: Resource | null;
	center: Center | null;
	incident: Incident | null;
}

export const Route = createFileRoute("/resources/$resourceId")({
	loader: async ({ params }): Promise<LoaderData> => {
		try {
			const res = await fetch(
				`${API_BASE}/api/v1/resources/${params.resourceId}`,
			);
			if (!res.ok) return { resource: null, center: null, incident: null };
			const resource: Resource = await res.json();

			const [centerRes, incidentRes] = await Promise.all([
				fetch(
					`${API_BASE}/api/v1/command-centers/${resource.owner_center_id}`,
				).catch(() => null),
				resource.assigned_incident_id
					? fetch(
							`${API_BASE}/api/v1/incidents/${resource.assigned_incident_id}`,
						).catch(() => null)
					: Promise.resolve(null),
			]);

			const center = centerRes?.ok ? await centerRes.json() : null;
			const incident = incidentRes?.ok ? await incidentRes.json() : null;

			return { resource, center, incident };
		} catch (_e) {
			return { resource: null, center: null, incident: null };
		}
	},
	component: ResourceDetails,
});

const statusColors: Record<string, string> = {
	EN_ROUTE: "bg-sky-500/10 text-sky-500 border-sky-500/30",
	STUCK: "bg-amber-500/10 text-amber-500 border-amber-500/30",
	REJECTED: "bg-rose-500/10 text-rose-500 border-rose-500/30",
	COMPLETED: "bg-emerald-500/10 text-emerald-500 border-emerald-500/30",
};

function ResourceDetails() {
	const initialData = Route.useLoaderData();
	const { resourceId } = Route.useParams();
	const [data, setData] = useState<LoaderData>(initialData);
	const [isClient, setIsClient] = useState(false);
	const [updating, setUpdating] = useState(false);
	const [selectedStatus, setSelectedStatus] = useState(
		initialData.resource?.status || "EN_ROUTE",
	);
	const [capacityInput, setCapacityInput] = useState(
		initialData.resource?.current_capacity ?? 0,
	);

	useEffect(() => {
		setIsClient(true);
	}, []);

	useEffect(() => {
		const interval = setInterval(async () => {
			try {
				const res = await fetch(
					`${API_BASE}/api/v1/resources/${resourceId}`,
				);
				if (res.ok) {
					const resource: Resource = await res.json();
					const [centerRes, incidentRes] = await Promise.all([
						fetch(
							`${API_BASE}/api/v1/command-centers/${resource.owner_center_id}`,
						).catch(() => null),
						resource.assigned_incident_id
							? fetch(
									`${API_BASE}/api/v1/incidents/${resource.assigned_incident_id}`,
								).catch(() => null)
							: Promise.resolve(null),
					]);

					const center = centerRes?.ok ? await centerRes.json() : null;
					const incident = incidentRes?.ok ? await incidentRes.json() : null;

					setData({ resource, center, incident });
				}
			} catch (_e) {
				// ignore polling errors
			}
		}, 3000);
		return () => clearInterval(interval);
	}, [resourceId]);

	if (!data.resource) {
		return (
			<div className="min-h-screen bg-slate-50 dark:bg-slate-950 p-10 flex flex-col items-center justify-center gap-4">
				<div className="w-20 h-20 rounded-full bg-rose-500/10 flex items-center justify-center">
					<AlertTriangle className="w-10 h-10 text-rose-500" />
				</div>
				<h1 className="text-2xl font-bold text-slate-800 dark:text-slate-200">
					Resource Not Found
				</h1>
				<Link
					to="/"
					className="mt-4 px-6 py-2.5 bg-indigo-500 text-white rounded-xl font-bold text-sm hover:bg-indigo-600 transition-colors"
				>
					Return to Dashboard
				</Link>
			</div>
		);
	}

	const { resource, center, incident } = data;
	const totalDist =
		resource.distance_passed_km + resource.distance_remaining_km;
	const progressPercent =
		totalDist > 0 ? (resource.distance_passed_km / totalDist) * 100 : 0;
	const capacityPercent =
		resource.total_capacity > 0
			? (resource.current_capacity / resource.total_capacity) * 100
			: 0;

	const handleStatusUpdate = async (e: React.FormEvent) => {
		e.preventDefault();
		setUpdating(true);
		try {
			const res = await fetch(
				`${API_BASE}/api/v1/resources/${resource.id}/status`,
				{
					method: "PATCH",
					headers: {
						"Content-Type": "application/json",
						"x-role": "admin",
					},
					body: JSON.stringify({
						status: selectedStatus,
						current_capacity: capacityInput,
					}),
				},
			);
			if (res.ok) {
				const updated: Resource = await res.json();
				setData((prev) => ({ ...prev, resource: updated }));
			}
		} catch (_e) {
			// ignore
		} finally {
			setUpdating(false);
		}
	};

	const navigate = useNavigate();
	const [deleting, setDeleting] = useState(false);

	const handleDeleteResource = async () => {
		if (!confirm("Are you sure you want to delete this resource?")) return;
		setDeleting(true);
		try {
			const res = await fetch(
				`${API_BASE}/api/v1/resources/${resource.id}`,
				{
					method: "DELETE",
					headers: { "x-role": "admin" },
				},
			);
			if (res.ok || res.status === 204) {
				navigate({ to: "/" });
			}
		} catch (_e) {
			// ignore
		} finally {
			setDeleting(false);
		}
	};

	return (
		<div className="min-h-screen bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-50 p-4 md:p-8 transition-colors duration-300">
			<div className="max-w-7xl mx-auto space-y-6">
				{/* Header */}
				<header className="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-6 border-b border-slate-200 dark:border-slate-800">
					<div className="flex items-center gap-4">
						<Link
							to="/"
							className="p-2.5 rounded-xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors shadow-sm"
						>
							<ArrowLeft className="w-5 h-5 text-slate-600 dark:text-slate-400" />
						</Link>
						<div>
							<div className="flex items-center gap-3 mb-1">
								<Truck className="w-6 h-6 text-emerald-500" />
								<h1 className="text-2xl md:text-3xl font-bold tracking-tight text-slate-900 dark:text-white">
									{resource.unit_identifier}
								</h1>
							</div>
							<p className="text-xs font-mono text-slate-500 uppercase tracking-wider">
								{resource.resource_type.replace(/_/g, " ")} · ID:{" "}
								{resource.id.slice(0, 8)}…
							</p>
						</div>
					</div>
					<div className="flex items-center gap-3">
						<span
							className={`px-4 py-2 rounded-full font-bold text-xs uppercase tracking-wider border ${statusColors[resource.status] || "bg-slate-100 text-slate-500 border-slate-300"}`}
						>
							{resource.status.replace(/_/g, " ")}
						</span>
						<button
							type="button"
							onClick={handleDeleteResource}
							disabled={deleting}
							className="p-2 bg-rose-500/10 hover:bg-rose-500/20 text-rose-600 dark:text-rose-400 border border-rose-500/30 rounded-xl transition-all disabled:opacity-50"
							title="Delete Resource"
						>
							{deleting ? (
								<Loader2 className="w-4 h-4 animate-spin" />
							) : (
								<Trash2 className="w-4 h-4" />
							)}
						</button>
					</div>
				</header>

				{/* Main Layout */}
				<div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
					{/* Left Column */}
					<div className="space-y-6">
						{/* Status & Capacity */}
						<div className="bg-white dark:bg-slate-900/60 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 shadow-sm space-y-4">
							<h2 className="text-sm font-bold uppercase tracking-wider text-slate-400">
								Capacity & Status
							</h2>

							<div>
								<div className="flex justify-between items-center text-sm font-medium mb-1">
									<span className="text-slate-500 flex items-center gap-2">
										<Package className="w-4 h-4 text-indigo-500" /> Current
										Capacity
									</span>
									<span className="font-bold">
										{resource.current_capacity} / {resource.total_capacity}{" "}
										Units
									</span>
								</div>
								<div className="w-full h-2.5 bg-slate-100 dark:bg-slate-800 rounded-full overflow-hidden">
									<div
										className="h-full bg-gradient-to-r from-indigo-500 to-emerald-500 transition-all duration-500"
										style={{ width: `${capacityPercent}%` }}
									/>
								</div>
							</div>

							<div className="pt-2 border-t border-slate-100 dark:border-slate-800/50 space-y-2">
								<div className="flex justify-between items-center text-xs">
									<span className="text-slate-500 flex items-center gap-2">
										<Building2 className="w-3.5 h-3.5 text-slate-400" /> Home
										Base
									</span>
									<span className="font-bold text-indigo-600 dark:text-indigo-400">
										{center?.name || "Unknown Hub"}
									</span>
								</div>

								<div className="flex justify-between items-center text-xs">
									<span className="text-slate-500 flex items-center gap-2">
										<AlertCircle className="w-3.5 h-3.5 text-rose-500" />{" "}
										Assigned Incident
									</span>
									{incident ? (
										<Link
											to="/incidents/$incidentId"
											params={{ incidentId: incident.id }}
											className="font-bold text-rose-600 dark:text-rose-400 hover:underline"
										>
											{incident.title}
										</Link>
									) : (
										<span className="text-slate-400">Unassigned</span>
									)}
								</div>

								<div className="flex justify-between items-center text-xs">
									<span className="text-slate-500 flex items-center gap-2">
										<MapPin className="w-3.5 h-3.5 text-slate-400" />{" "}
										Coordinates
									</span>
									<span className="font-mono text-slate-600 dark:text-slate-400">
										{resource.latitude.toFixed(4)},{" "}
										{resource.longitude.toFixed(4)}
									</span>
								</div>
							</div>
						</div>

						{/* Distance & Telemetry */}
						<div className="bg-white dark:bg-slate-900/60 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 shadow-sm space-y-4">
							<h2 className="text-sm font-bold uppercase tracking-wider text-slate-400 flex items-center gap-2">
								<Navigation className="w-4 h-4 text-sky-500" /> Distance
								Telemetry
							</h2>

							<div className="grid grid-cols-2 gap-3 text-center">
								<div className="p-3 bg-slate-50 dark:bg-slate-950 rounded-xl border border-slate-200 dark:border-slate-800">
									<div className="text-xs text-slate-500 font-medium mb-1">
										Passed
									</div>
									<div className="text-lg font-black text-slate-800 dark:text-slate-200">
										{resource.distance_passed_km.toFixed(1)} km
									</div>
								</div>
								<div className="p-3 bg-slate-50 dark:bg-slate-950 rounded-xl border border-slate-200 dark:border-slate-800">
									<div className="text-xs text-slate-500 font-medium mb-1">
										Remaining
									</div>
									<div className="text-lg font-black text-emerald-600 dark:text-emerald-400">
										{resource.distance_remaining_km.toFixed(1)} km
									</div>
								</div>
							</div>

							{totalDist > 0 && (
								<div>
									<div className="flex justify-between text-xs text-slate-500 mb-1">
										<span>Route Progress</span>
										<span>{progressPercent.toFixed(0)}%</span>
									</div>
									<div className="w-full h-2 bg-slate-100 dark:bg-slate-800 rounded-full overflow-hidden">
										<div
											className="h-full bg-sky-500 transition-all duration-500"
											style={{ width: `${progressPercent}%` }}
										/>
									</div>
								</div>
							)}
						</div>

						{/* Quick Update Form */}
						<div className="bg-white dark:bg-slate-900/60 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 shadow-sm space-y-4">
							<h2 className="text-sm font-bold uppercase tracking-wider text-slate-400 flex items-center gap-2">
								<RefreshCw className="w-4 h-4 text-indigo-500" /> Update Status
								& Capacity
							</h2>

							<form onSubmit={handleStatusUpdate} className="space-y-3">
								<div>
									<label className="block text-xs font-semibold text-slate-600 dark:text-slate-400 mb-1">
										Status
									</label>
									<select
										value={selectedStatus}
										onChange={(e) => setSelectedStatus(e.target.value)}
										className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-xl px-3 py-2 text-sm font-medium focus:outline-none focus:ring-2 focus:ring-indigo-500/50"
									>
										<option value="EN_ROUTE">EN ROUTE</option>
										<option value="STUCK">STUCK</option>
										<option value="REJECTED">REJECTED</option>
										<option value="COMPLETED">COMPLETED</option>
									</select>
								</div>

								<div>
									<label className="block text-xs font-semibold text-slate-600 dark:text-slate-400 mb-1">
										Current Capacity
									</label>
									<input
										type="number"
										min={0}
										max={resource.total_capacity}
										value={capacityInput}
										onChange={(e) => setCapacityInput(Number(e.target.value))}
										className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-xl px-3 py-2 text-sm font-medium focus:outline-none focus:ring-2 focus:ring-indigo-500/50"
									/>
								</div>

								<button
									type="submit"
									disabled={updating}
									className="w-full py-2.5 bg-indigo-600 hover:bg-indigo-700 text-white font-bold text-xs uppercase tracking-wider rounded-xl transition-all shadow-sm flex items-center justify-center gap-2 disabled:opacity-50"
								>
									{updating ? (
										<Loader2 className="w-4 h-4 animate-spin" />
									) : (
										<CheckCircle2 className="w-4 h-4" />
									)}
									Apply Changes
								</button>
							</form>
						</div>
					</div>

					{/* Right Column: Map */}
					<div className="lg:col-span-2">
						<div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl overflow-hidden shadow-sm h-[600px] relative">
							{isClient ? (
								<Suspense
									fallback={
										<div className="w-full h-full flex items-center justify-center bg-slate-100 dark:bg-slate-900">
											<Loader2 className="w-8 h-8 animate-spin text-indigo-500" />
										</div>
									}
								>
									<ResourceMap
										resource={resource}
										center={center}
										incident={incident}
									/>
								</Suspense>
							) : (
								<div className="w-full h-full flex items-center justify-center bg-slate-100 dark:bg-slate-900">
									<Loader2 className="w-8 h-8 animate-spin text-indigo-500" />
								</div>
							)}

							<div className="absolute top-4 right-4 z-[400] bg-white/90 dark:bg-slate-900/90 backdrop-blur-md px-4 py-2 rounded-xl shadow-lg border border-slate-200 dark:border-slate-700 text-xs font-bold text-slate-700 dark:text-slate-300">
								Asset Live Location
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}
