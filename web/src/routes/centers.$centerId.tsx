import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useEffect, useState, lazy, Suspense } from "react";
import { API_BASE } from "#/lib/apiClient.ts";
import {
	ArrowLeft,
	MapPin,
	Shield,
	Truck,
	AlertTriangle,
	Loader2,
	Radio,
	Activity,
	Users,
	AlertCircle,
	Trash2,
} from "lucide-react";

const LeafletMap = lazy(() => import("#/components/CenterMap"));

interface Center {
	id: string;
	name: string;
	latitude: number;
	longitude: number;
	is_core_center: boolean;
}

interface Resource {
	id: string;
	resource_type: string;
	unit_identifier: string;
	status: string;
	latitude: number;
	longitude: number;
	distance_passed_km: number;
	distance_remaining_km: number;
	current_capacity: number;
	total_capacity: number;
	owner_center_id: string;
}

interface Incident {
	id: string;
	title: string;
	status: string;
	severity_level: number;
	affected_people: number;
	casualty_count: number;
	latitude: number;
	longitude: number;
	primary_center_id: string;
	created_at: string;
	updated_at: string;
}

interface LoaderData {
	center: Center | null;
	resources: Resource[];
	incidents: Incident[];
}

export const Route = createFileRoute("/centers/$centerId")({
	loader: async ({ params }): Promise<LoaderData> => {
		try {
			const [centerRes, resourcesRes, incidentsRes] = await Promise.all([
				fetch(
					`${API_BASE}/api/v1/command-centers/${params.centerId}`,
				).catch(() => null),
				fetch(
					`${API_BASE}/api/v1/resources?owner_center_id=${params.centerId}`,
				).catch(() => null),
				fetch(`${API_BASE}/api/v1/incidents`).catch(() => null),
			]);

			const center = centerRes?.ok ? await centerRes.json() : null;
			const resources = resourcesRes?.ok ? await resourcesRes.json() : [];
			const allIncidents = incidentsRes?.ok ? await incidentsRes.json() : [];

			const safeIncidents = Array.isArray(allIncidents) ? allIncidents : [];
			const routedIncidents = center
				? safeIncidents.filter(
						(i: Incident) => i.primary_center_id === params.centerId,
					)
				: [];

			return {
				center,
				resources: Array.isArray(resources) ? resources : [],
				incidents: routedIncidents,
			};
		} catch (_e) {
			return { center: null, resources: [], incidents: [] };
		}
	},
	component: CenterDetails,
});

const resourceStatusColors: Record<string, string> = {
	EN_ROUTE: "bg-sky-500/10 text-sky-600 dark:text-sky-400 border-sky-500/30",
	STUCK:
		"bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/30",
	REJECTED:
		"bg-rose-500/10 text-rose-600 dark:text-rose-400 border-rose-500/30",
	COMPLETED:
		"bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/30",
};

const severityColors: Record<number, string> = {
	1: "bg-emerald-500",
	2: "bg-sky-500",
	3: "bg-amber-500",
	4: "bg-orange-500",
	5: "bg-rose-500",
};

const statusColors: Record<string, string> = {
	ACTIVE: "bg-rose-500/10 text-rose-500 border-rose-500/30",
	DISPATCHED: "bg-amber-500/10 text-amber-500 border-amber-500/30",
	RESOLVED: "bg-emerald-500/10 text-emerald-500 border-emerald-500/30",
};

function CenterDetails() {
	const initialData = Route.useLoaderData();
	const { centerId } = Route.useParams();
	const [data, setData] = useState<LoaderData>(initialData);
	const [isClient, setIsClient] = useState(false);

	useEffect(() => {
		setIsClient(true);
	}, []);

	useEffect(() => {
		const interval = setInterval(async () => {
			try {
				const [centerRes, resourcesRes, incidentsRes] = await Promise.all([
					fetch(`${API_BASE}/api/v1/command-centers/${centerId}`),
					fetch(
						`${API_BASE}/api/v1/resources?owner_center_id=${centerId}`,
					),
					fetch(`${API_BASE}/api/v1/incidents`),
				]);
				if (centerRes.ok) {
					const center = await centerRes.json();
					const resources = resourcesRes.ok ? await resourcesRes.json() : [];
					const allIncidents = incidentsRes.ok ? await incidentsRes.json() : [];

					const safeIncidents = Array.isArray(allIncidents) ? allIncidents : [];
					const routedIncidents = safeIncidents.filter(
						(i: Incident) => i.primary_center_id === centerId,
					);

					setData({
						center,
						resources: Array.isArray(resources) ? resources : [],
						incidents: routedIncidents,
					});
				}
			} catch (_e) {
				// ignore network errors on poll
			}
		}, 5000);
		return () => clearInterval(interval);
	}, [centerId]);

	if (!data.center) {
		return (
			<div className="min-h-screen bg-slate-50 dark:bg-slate-950 p-10 flex flex-col items-center justify-center gap-4">
				<div className="w-20 h-20 rounded-full bg-rose-500/10 flex items-center justify-center">
					<AlertTriangle className="w-10 h-10 text-rose-500" />
				</div>
				<h1 className="text-2xl font-bold text-slate-800 dark:text-slate-200">
					Center Not Found
				</h1>
				<p className="text-slate-500 text-sm">
					The command center may have been removed or is unavailable.
				</p>
				<Link
					to="/"
					className="mt-4 px-6 py-2.5 bg-indigo-500 text-white rounded-xl font-bold text-sm hover:bg-indigo-600 transition-colors"
				>
					Return to Dashboard
				</Link>
			</div>
		);
	}

	const { center, resources, incidents } = data;

	const navigate = useNavigate();
	const [deleting, setDeleting] = useState(false);

	const handleDeleteCenter = async () => {
		if (!confirm("Are you sure you want to delete this command center?"))
			return;
		setDeleting(true);
		try {
			const res = await fetch(
				`${API_BASE}/api/v1/centers/${centerId}`,
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
								<div className="w-3 h-3 rounded-full bg-indigo-500 animate-pulse" />
								<h1 className="text-2xl md:text-3xl font-bold tracking-tight text-slate-900 dark:text-white">
									{center.name}
								</h1>
								{center.is_core_center && (
									<span className="text-[10px] uppercase font-bold text-indigo-600 dark:text-indigo-400 bg-indigo-100 dark:bg-indigo-500/10 px-2.5 py-1 rounded-full border border-indigo-200 dark:border-indigo-500/30">
										Core
									</span>
								)}
							</div>
							<div className="flex items-center gap-3 text-xs text-slate-500">
								<span className="font-mono">{center.id.slice(0, 8)}…</span>
								<span className="flex items-center gap-1">
									<MapPin className="w-3 h-3" />
									{center.latitude.toFixed(4)}, {center.longitude.toFixed(4)}
								</span>
							</div>
						</div>
					</div>
					<div className="flex items-center gap-3">
						<span className="px-4 py-2 rounded-full font-bold text-xs uppercase tracking-wider bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/30">
							<Radio className="w-3 h-3 inline-block mr-1.5 -mt-0.5" />
							Online
						</span>
						<button
							type="button"
							onClick={handleDeleteCenter}
							disabled={deleting}
							className="p-2 bg-rose-500/10 hover:bg-rose-500/20 text-rose-600 dark:text-rose-400 border border-rose-500/30 rounded-xl transition-all disabled:opacity-50"
							title="Delete Center"
						>
							{deleting ? (
								<Loader2 className="w-4 h-4 animate-spin" />
							) : (
								<Trash2 className="w-4 h-4" />
							)}
						</button>
						<span className="px-4 py-2 rounded-full font-bold text-xs uppercase tracking-wider bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300">
							{center.is_core_center ? "Core Hub" : "Divisional"}
						</span>
					</div>
				</header>

				{/* Grid Layout */}
				<div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
					{/* Left Column */}
					<div className="space-y-6">
						{/* Center Info */}
						<div className="bg-white dark:bg-slate-900/60 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 shadow-sm space-y-1">
							<h2 className="text-sm font-bold uppercase tracking-wider text-slate-400 mb-4">
								Center Information
							</h2>
							<MetricRow
								icon={<Shield className="w-4 h-4 text-indigo-500" />}
								label="Name"
								value={center.name}
								valueClass="text-indigo-600 dark:text-indigo-400"
							/>
							<MetricRow
								icon={<MapPin className="w-4 h-4 text-slate-400" />}
								label="Coordinates"
								value={`${center.latitude.toFixed(4)}, ${center.longitude.toFixed(4)}`}
								valueClass="font-mono text-xs"
							/>
							<MetricRow
								icon={<Radio className="w-4 h-4 text-emerald-500" />}
								label="Type"
								value={center.is_core_center ? "Core Operations" : "Divisional"}
							/>
							<MetricRow
								icon={<Truck className="w-4 h-4 text-sky-500" />}
								label="Resources"
								value={resources.length.toString()}
							/>
							<MetricRow
								icon={<AlertCircle className="w-4 h-4 text-rose-500" />}
								label="Active Incidents"
								value={incidents
									.filter((i) => i.status === "ACTIVE")
									.length.toString()}
								valueClass="text-rose-600 dark:text-rose-400"
							/>
						</div>

						{/* Resources List */}
						<div className="bg-white dark:bg-slate-900/60 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 shadow-sm">
							<h2 className="text-sm font-bold uppercase tracking-wider text-slate-400 mb-4 flex items-center gap-2">
								<Truck className="w-4 h-4 text-emerald-500" />
								Owned Resources ({resources.length})
							</h2>

							{resources.length === 0 ? (
								<div className="text-sm text-slate-500 text-center py-8 bg-slate-50 dark:bg-slate-950/50 rounded-xl border border-dashed border-slate-200 dark:border-slate-800">
									No resources assigned to this center.
								</div>
							) : (
								<div className="space-y-2 max-h-[300px] overflow-y-auto pr-1">
									{resources.map((resource) => {
										const rsColor =
											resourceStatusColors[resource.status] ||
											"bg-slate-100 text-slate-500 border-slate-300";

										return (
											<Link
												to="/resources/$resourceId"
												params={{ resourceId: resource.id }}
												key={resource.id}
												className="block p-3 bg-slate-50 dark:bg-slate-950 rounded-xl border border-slate-200 dark:border-slate-800 hover:border-indigo-500/40 transition-colors cursor-pointer"
											>
												<div className="flex justify-between items-center">
													<div>
														<div className="font-bold text-sm text-slate-800 dark:text-slate-200">
															{resource.unit_identifier}
														</div>
														<div className="text-[10px] text-slate-500 uppercase tracking-wider">
															{resource.resource_type.replace(/_/g, " ")}
														</div>
													</div>
													<span
														className={`text-[10px] font-bold px-2 py-1 rounded-md border ${rsColor}`}
													>
														{resource.status.replace(/_/g, " ")}
													</span>
												</div>
											</Link>
										);
									})}
								</div>
							)}
						</div>

						{/* Routed Incidents */}
						<div className="bg-white dark:bg-slate-900/60 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 shadow-sm">
							<h2 className="text-sm font-bold uppercase tracking-wider text-slate-400 mb-4 flex items-center gap-2">
								<Activity className="w-4 h-4 text-rose-500" />
								Routed Incidents ({incidents.length})
							</h2>

							{incidents.length === 0 ? (
								<div className="text-sm text-slate-500 text-center py-8 bg-slate-50 dark:bg-slate-950/50 rounded-xl border border-dashed border-slate-200 dark:border-slate-800">
									No incidents routed to this center.
								</div>
							) : (
								<div className="space-y-2 max-h-[300px] overflow-y-auto pr-1">
									{incidents.map((inc) => {
										const sevColor =
											severityColors[inc.severity_level] || "bg-slate-500";
										const statColor =
											statusColors[inc.status] ||
											"bg-slate-100 text-slate-500 border-slate-300";

										return (
											<Link
												key={inc.id}
												to="/incidents/$incidentId"
												params={{
													incidentId: inc.id,
												}}
												className="block p-3 bg-slate-50 dark:bg-slate-950 rounded-xl border border-slate-200 dark:border-slate-800 hover:border-indigo-500/40 transition-colors"
											>
												<div className="flex justify-between items-start mb-1.5">
													<div className="flex items-center gap-2">
														<div
															className={`w-2 h-2 rounded-full ${sevColor} ${inc.status === "ACTIVE" ? "animate-pulse" : ""}`}
														/>
														<span className="font-bold text-sm text-slate-800 dark:text-slate-200">
															{inc.title}
														</span>
													</div>
													<span
														className={`text-[10px] font-bold px-2 py-0.5 rounded-md border ${statColor}`}
													>
														{inc.status}
													</span>
												</div>
												<div className="flex items-center gap-3 text-[10px] text-slate-500 ml-4">
													<span className="flex items-center gap-1">
														<Users className="w-3 h-3" />
														{inc.affected_people}
													</span>
													<span>L{inc.severity_level}</span>
												</div>
											</Link>
										);
									})}
								</div>
							)}
						</div>
					</div>

					{/* Right Column: Map */}
					<div className="lg:col-span-2">
						<div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl overflow-hidden shadow-sm h-[550px] relative">
							{isClient ? (
								<Suspense
									fallback={
										<div className="w-full h-full flex items-center justify-center bg-slate-100 dark:bg-slate-900">
											<Loader2 className="w-8 h-8 animate-spin text-indigo-500" />
										</div>
									}
								>
									<LeafletMap
										center={center}
										resources={resources}
										incidents={incidents}
									/>
								</Suspense>
							) : (
								<div className="w-full h-full flex items-center justify-center bg-slate-100 dark:bg-slate-900">
									<Loader2 className="w-8 h-8 animate-spin text-indigo-500" />
								</div>
							)}

							<div className="absolute top-4 right-4 z-[400] bg-white/90 dark:bg-slate-900/90 backdrop-blur-md px-4 py-2 rounded-xl shadow-lg border border-slate-200 dark:border-slate-700">
								<div className="flex items-center gap-2">
									<span className="w-2 h-2 rounded-full bg-indigo-500 animate-pulse" />
									<span className="text-xs font-bold text-slate-700 dark:text-slate-300">
										Center Overview
									</span>
								</div>
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}

function MetricRow({
	icon,
	label,
	value,
	valueClass = "",
}: {
	icon: React.ReactNode;
	label: string;
	value: string;
	valueClass?: string;
}) {
	return (
		<div className="flex justify-between items-center py-3 border-b border-slate-100 dark:border-slate-800/50 last:border-0">
			<span className="text-sm text-slate-500 flex items-center gap-2">
				{icon} {label}
			</span>
			<span className={`font-bold text-sm ${valueClass}`}>{value}</span>
		</div>
	);
}
