import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useEffect, useState, lazy, Suspense } from "react";
import {
	ArrowLeft,
	AlertCircle,
	MapPin,
	Users,
	Activity,
	Truck,
	AlertTriangle,
	Clock,
	Loader2,
	Trash2,
} from "lucide-react";

const LeafletMap = lazy(() => import("#/components/IncidentMap"));

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
}

interface LoaderData {
	incident: Incident | null;
	center: Center | null;
	resources: Resource[];
	allCenters: Center[];
}

export const Route = createFileRoute("/incidents/$incidentId")({
	loader: async ({ params }): Promise<LoaderData> => {
		try {
			const [incidentRes, centersRes, resourcesRes] = await Promise.all([
				fetch(
					`http://localhost:8080/api/v1/incidents/${params.incidentId}`,
				).catch(() => null),
				fetch("http://localhost:8080/api/v1/command-centers").catch(() => null),
				fetch(
					`http://localhost:8080/api/v1/resources?assigned_incident_id=${params.incidentId}`,
				).catch(() => null),
			]);

			const incident = incidentRes?.ok ? await incidentRes.json() : null;
			const centers = centersRes?.ok ? await centersRes.json() : [];
			const resources = resourcesRes?.ok ? await resourcesRes.json() : [];

			const allCenters = Array.isArray(centers) ? centers : [];
			const center =
				allCenters.length > 0 && incident
					? allCenters.find(
							(c: Center) => c.id === incident.primary_center_id,
						) || null
					: null;

			return {
				incident,
				center,
				resources: Array.isArray(resources) ? resources : [],
				allCenters,
			};
		} catch (_e) {
			return {
				incident: null,
				center: null,
				resources: [],
				allCenters: [],
			};
		}
	},
	component: IncidentDetails,
});

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

const resourceStatusColors: Record<string, string> = {
	EN_ROUTE: "bg-sky-500/10 text-sky-600 dark:text-sky-400 border-sky-500/30",
	STUCK:
		"bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/30",
	REJECTED:
		"bg-rose-500/10 text-rose-600 dark:text-rose-400 border-rose-500/30",
	COMPLETED:
		"bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/30",
};

function IncidentDetails() {
	const initialData = Route.useLoaderData();
	const { incidentId } = Route.useParams();
	const [data, setData] = useState<LoaderData>(initialData);
	const [isClient, setIsClient] = useState(false);

	useEffect(() => {
		setIsClient(true);
	}, []);

	useEffect(() => {
		const interval = setInterval(async () => {
			try {
				const [incidentRes, centersRes, resourcesRes] = await Promise.all([
					fetch(`http://localhost:8080/api/v1/incidents/${incidentId}`),
					fetch("http://localhost:8080/api/v1/command-centers"),
					fetch(
						`http://localhost:8080/api/v1/resources?assigned_incident_id=${incidentId}`,
					),
				]);
				if (incidentRes.ok) {
					const incident = await incidentRes.json();
					const centers = centersRes.ok ? await centersRes.json() : [];
					const resources = resourcesRes.ok ? await resourcesRes.json() : [];

					const allCenters = Array.isArray(centers) ? centers : [];
					const center =
						allCenters.length > 0
							? allCenters.find(
									(c: Center) => c.id === incident.primary_center_id,
								) || null
							: null;

					setData({
						incident,
						center,
						resources: Array.isArray(resources) ? resources : [],
						allCenters,
					});
				}
			} catch (_e) {
				// ignore network errors on poll
			}
		}, 5000);
		return () => clearInterval(interval);
	}, [incidentId]);

	if (!data.incident) {
		return (
			<div className="min-h-screen bg-slate-50 dark:bg-slate-950 p-10 flex flex-col items-center justify-center gap-4">
				<div className="w-20 h-20 rounded-full bg-rose-500/10 flex items-center justify-center">
					<AlertTriangle className="w-10 h-10 text-rose-500" />
				</div>
				<h1 className="text-2xl font-bold text-slate-800 dark:text-slate-200">
					Incident Not Found
				</h1>
				<p className="text-slate-500 text-sm">
					The incident may have been resolved or removed.
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

	const { incident, center, resources } = data;
	const timeAgo = getTimeAgo(incident.created_at);
	const sevColor = severityColors[incident.severity_level] || "bg-slate-500";
	const statColor =
		statusColors[incident.status] ||
		"bg-slate-100 text-slate-500 border-slate-300";

	const navigate = useNavigate();
	const [deleting, setDeleting] = useState(false);

	const handleDelete = async () => {
		if (!confirm("Are you sure you want to delete this incident?")) return;
		setDeleting(true);
		try {
			const res = await fetch(
				`http://localhost:8080/api/v1/incidents/${incidentId}`,
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
								<div
									className={`w-3 h-3 rounded-full ${sevColor} ${incident.status === "ACTIVE" ? "animate-pulse" : ""}`}
								/>
								<h1 className="text-2xl md:text-3xl font-bold tracking-tight text-slate-900 dark:text-white">
									{incident.title}
								</h1>
							</div>
							<div className="flex items-center gap-3 text-xs text-slate-500">
								<span className="font-mono">{incident.id.slice(0, 8)}…</span>
								<span className="flex items-center gap-1">
									<Clock className="w-3 h-3" /> {timeAgo}
								</span>
							</div>
						</div>
					</div>
					<div className="flex items-center gap-3">
						<span
							className={`px-4 py-2 rounded-full font-bold text-xs uppercase tracking-wider border ${statColor}`}
						>
							{incident.status}
						</span>
						<span className="px-4 py-2 rounded-full font-bold text-xs uppercase tracking-wider bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300">
							Level {incident.severity_level}
						</span>
						<button
							type="button"
							onClick={handleDelete}
							disabled={deleting}
							className="p-2 bg-rose-500/10 hover:bg-rose-500/20 text-rose-600 dark:text-rose-400 border border-rose-500/30 rounded-xl transition-all disabled:opacity-50"
							title="Delete Incident"
						>
							{deleting ? (
								<Loader2 className="w-4 h-4 animate-spin" />
							) : (
								<Trash2 className="w-4 h-4" />
							)}
						</button>
					</div>
				</header>

				{/* Grid Layout */}
				<div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
					{/* Left Column */}
					<div className="space-y-6">
						{/* Key Metrics */}
						<div className="bg-white dark:bg-slate-900/60 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 shadow-sm space-y-1">
							<h2 className="text-sm font-bold uppercase tracking-wider text-slate-400 mb-4">
								Impact Metrics
							</h2>
							<MetricRow
								icon={<Users className="w-4 h-4 text-orange-500" />}
								label="Affected People"
								value={incident.affected_people.toLocaleString()}
							/>
							<MetricRow
								icon={<AlertCircle className="w-4 h-4 text-rose-500" />}
								label="Casualties"
								value={incident.casualty_count.toLocaleString()}
								valueClass="text-rose-600 dark:text-rose-400"
							/>
							<MetricRow
								icon={<MapPin className="w-4 h-4 text-indigo-500" />}
								label="Primary Center"
								value={center?.name || "Unassigned"}
								valueClass="text-indigo-600 dark:text-indigo-400"
							/>
							<MetricRow
								icon={<MapPin className="w-4 h-4 text-slate-400" />}
								label="Coordinates"
								value={`${incident.latitude.toFixed(4)}, ${incident.longitude.toFixed(4)}`}
								valueClass="font-mono text-xs"
							/>
						</div>

						{/* Assigned Resources */}
						<div className="bg-white dark:bg-slate-900/60 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 shadow-sm">
							<h2 className="text-sm font-bold uppercase tracking-wider text-slate-400 mb-4 flex items-center gap-2">
								<Truck className="w-4 h-4 text-emerald-500" />
								Dispatched Resources ({resources.length})
							</h2>

							{resources.length === 0 ? (
								<div className="text-sm text-slate-500 text-center py-8 bg-slate-50 dark:bg-slate-950/50 rounded-xl border border-dashed border-slate-200 dark:border-slate-800">
									No resources allocated yet.
								</div>
							) : (
								<div className="space-y-2">
									{resources.map((resource) => {
										const totalDist =
											resource.distance_passed_km +
											resource.distance_remaining_km;
										const progress =
											totalDist > 0
												? (resource.distance_passed_km / totalDist) * 100
												: 0;
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
												<div className="flex justify-between items-center mb-2">
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
												{totalDist > 0 && (
													<div>
														<div className="flex justify-between text-[10px] text-slate-500 mb-1">
															<span>
																{resource.distance_passed_km.toFixed(1)}
																km passed
															</span>
															<span>
																{resource.distance_remaining_km.toFixed(1)}
																km left
															</span>
														</div>
														<div className="w-full h-1.5 bg-slate-200 dark:bg-slate-800 rounded-full overflow-hidden">
															<div
																className="h-full bg-gradient-to-r from-indigo-500 to-emerald-500 rounded-full transition-all duration-700"
																style={{ width: `${progress}%` }}
															/>
														</div>
													</div>
												)}
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
										incident={incident}
										center={center}
										resources={resources}
										allCenters={data.allCenters}
									/>
								</Suspense>
							) : (
								<div className="w-full h-full flex items-center justify-center bg-slate-100 dark:bg-slate-900">
									<Loader2 className="w-8 h-8 animate-spin text-indigo-500" />
								</div>
							)}

							<div className="absolute top-4 right-4 z-[400] bg-white/90 dark:bg-slate-900/90 backdrop-blur-md px-4 py-2 rounded-xl shadow-lg border border-slate-200 dark:border-slate-700">
								<div className="flex items-center gap-2">
									<span className="w-2 h-2 rounded-full bg-rose-500 animate-pulse" />
									<span className="text-xs font-bold text-slate-700 dark:text-slate-300">
										Live Telemetry
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

function getTimeAgo(dateStr: string): string {
	const now = Date.now();
	const then = new Date(dateStr).getTime();
	const diffMs = now - then;
	const mins = Math.floor(diffMs / 60000);
	if (mins < 1) return "Just now";
	if (mins < 60) return `${mins}m ago`;
	const hrs = Math.floor(mins / 60);
	if (hrs < 24) return `${hrs}h ago`;
	const days = Math.floor(hrs / 24);
	return `${days}d ago`;
}
