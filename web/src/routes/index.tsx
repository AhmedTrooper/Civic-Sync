import { createFileRoute, Link } from "@tanstack/react-router";
import {
	Activity,
	AlertCircle,
	Loader2,
	MapPin,
	Radio,
	ShieldAlert,
	Truck,
	Users,
} from "lucide-react";
import { lazy, Suspense, useEffect, useState } from "react";

const DashboardMap = lazy(() => import("#/components/DashboardMap"));

interface Incident {
	id: string;
	title: string;
	status: string;
	severity_level: number;
	affected_people: number;
	casualty_count: number;
	latitude: number;
	longitude: number;
}

interface Center {
	id: string;
	name: string;
	is_core_center: boolean;
	latitude: number;
	longitude: number;
}

interface Resource {
	id: string;
	status: string;
}

interface LoaderData {
	incidents: Incident[];
	centers: Center[];
	resources: Resource[];
}

export const Route = createFileRoute("/")({
	loader: async (): Promise<LoaderData> => {
		try {
			const [incidentsRes, centersRes, resourcesRes] = await Promise.all([
				fetch("http://localhost:8080/api/v1/incidents").catch(() => null),
				fetch("http://localhost:8080/api/v1/command-centers").catch(() => null),
				fetch("http://localhost:8080/api/v1/resources").catch(() => null),
			]);
			const incidents = incidentsRes?.ok ? await incidentsRes.json() : [];
			const centers = centersRes?.ok ? await centersRes.json() : [];
			const resources = resourcesRes?.ok ? await resourcesRes.json() : [];
			return {
				incidents: Array.isArray(incidents) ? incidents : [],
				centers: Array.isArray(centers) ? centers : [],
				resources: Array.isArray(resources) ? resources : [],
			};
		} catch (_e) {
			return { incidents: [], centers: [], resources: [] };
		}
	},
	component: Dashboard,
});

function Dashboard() {
	const initialData = Route.useLoaderData();
	const [data, setData] = useState<LoaderData>(initialData);
	const [isClient, setIsClient] = useState(false);

	useEffect(() => {
		setIsClient(true);
	}, []);

	useEffect(() => {
		const interval = setInterval(async () => {
			try {
				const [incidentsRes, centersRes, resourcesRes] = await Promise.all([
					fetch("http://localhost:8080/api/v1/incidents"),
					fetch("http://localhost:8080/api/v1/command-centers"),
					fetch("http://localhost:8080/api/v1/resources"),
				]);
				const incidents = incidentsRes.ok ? await incidentsRes.json() : [];
				const centers = centersRes.ok ? await centersRes.json() : [];
				const resources = resourcesRes.ok ? await resourcesRes.json() : [];
				setData({
					incidents: Array.isArray(incidents) ? incidents : [],
					centers: Array.isArray(centers) ? centers : [],
					resources: Array.isArray(resources) ? resources : [],
				});
			} catch (_e) {
				// ignore network errors on poll
			}
		}, 5000);
		return () => clearInterval(interval);
	}, []);

	const activeIncidents = data.incidents.filter(
		(i) => i.status === "ACTIVE",
	).length;
	const criticalIncidents = data.incidents.filter(
		(i) => i.severity_level >= 4,
	).length;
	const totalResources = data.resources.length;

	return (
		<div className="min-h-screen bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-50 p-6 md:p-10 selection:bg-rose-500/30 transition-colors duration-300">
			<div className="max-w-7xl mx-auto space-y-8">
				<header className="flex items-center justify-between">
					<div className="flex items-center gap-4">
						<div className="relative flex items-center justify-center w-14 h-14 rounded-2xl bg-gradient-to-br from-rose-500 to-orange-500 shadow-[0_0_40px_-10px_rgba(244,63,94,0.7)] group">
							<ShieldAlert className="w-7 h-7 text-white relative z-10 group-hover:scale-110 transition-transform duration-500" />
							<div className="absolute inset-0 rounded-2xl bg-rose-500 animate-ping opacity-20" />
						</div>
						<div>
							<h1 className="text-3xl font-extrabold tracking-tight bg-gradient-to-r from-rose-600 via-fuchsia-600 to-indigo-600 dark:from-rose-400 dark:via-fuchsia-400 dark:to-indigo-400 bg-clip-text text-transparent transition-all">
								CIVIC-SYNC
							</h1>
							<p className="text-xs font-medium text-rose-600 dark:text-rose-500/80 tracking-widest uppercase flex items-center gap-2 transition-colors">
								<span className="w-2 h-2 rounded-full bg-rose-500 animate-pulse" />
								Live Grid
							</p>
						</div>
					</div>
					<div className="flex items-center gap-3">
						<Link
							to="/admin"
							className="px-4 py-2 text-sm font-semibold text-rose-600 dark:text-rose-400 hover:text-rose-700 dark:hover:text-rose-300 transition-colors bg-white/50 dark:bg-slate-900/50 rounded-xl border border-slate-200 dark:border-slate-800"
						>
							<Radio className="w-4 h-4 inline-block mr-2" />
							Admin Panel
						</Link>
					</div>
				</header>

				{/* Stat Cards */}
				<div className="grid grid-cols-1 md:grid-cols-3 gap-6">
					<StatCard
						title="ACTIVE INCIDENTS"
						value={activeIncidents}
						icon={<AlertCircle className="w-5 h-5" />}
						color="rose"
						pulse
					/>
					<StatCard
						title="CRITICAL (L4-L5)"
						value={criticalIncidents}
						icon={<Activity className="w-5 h-5" />}
						color="orange"
					/>
					<StatCard
						title="DISPATCHED ASSETS"
						value={totalResources}
						icon={<Truck className="w-5 h-5" />}
						color="indigo"
					/>
				</div>

				{/* Global Map */}
				<div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl overflow-hidden shadow-sm h-[400px] relative">
					{isClient ? (
						<Suspense
							fallback={
								<div className="w-full h-full flex items-center justify-center bg-slate-100 dark:bg-slate-900">
									<Loader2 className="w-8 h-8 animate-spin text-indigo-500" />
								</div>
							}
						>
							<DashboardMap centers={data.centers} incidents={data.incidents} />
						</Suspense>
					) : (
						<div className="w-full h-full flex items-center justify-center bg-slate-100 dark:bg-slate-900">
							<Loader2 className="w-8 h-8 animate-spin text-indigo-500" />
						</div>
					)}
					<div className="absolute top-4 left-4 z-[400] bg-white/90 dark:bg-slate-900/90 backdrop-blur-md px-4 py-2 rounded-xl shadow-lg border border-slate-200 dark:border-slate-700">
						<div className="flex items-center gap-2">
							<MapPin className="w-4 h-4 text-indigo-500" />
							<span className="text-xs font-bold text-slate-700 dark:text-slate-300">
								National Emergency Grid
							</span>
						</div>
					</div>
				</div>

				{/* Operations + Hub Status */}
				<div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
					<div className="lg:col-span-2 space-y-6">
						<h2 className="text-xl font-bold tracking-tight flex items-center gap-3 text-slate-800 dark:text-slate-100">
							<MapPin className="w-5 h-5 text-indigo-500 dark:text-indigo-400" />
							Live Operations
						</h2>
						<div className="grid gap-4">
							{data.incidents.slice(0, 5).map((inc) => (
								<Link
									to="/incidents/$incidentId"
									params={{ incidentId: inc.id }}
									key={inc.id}
									className="block relative group overflow-hidden bg-white/80 dark:bg-slate-900/40 border border-slate-200 dark:border-slate-800/50 p-6 rounded-2xl hover:bg-slate-50 dark:hover:bg-slate-800/50 shadow-sm dark:shadow-none transition-all duration-300"
								>
									<div
										className={`absolute top-0 left-0 w-1 h-full ${inc.severity_level >= 4 ? "bg-rose-500 shadow-[0_0_20px_rgba(244,63,94,0.6)]" : "bg-orange-500"}`}
									/>
									<div className="flex justify-between items-start">
										<div>
											<h3 className="text-lg font-bold text-slate-900 dark:text-slate-200 group-hover:text-indigo-600 dark:group-hover:text-indigo-400 transition-colors">
												{inc.title}
											</h3>
											<p className="text-sm text-slate-600 dark:text-slate-400 mt-1 flex items-center gap-4">
												<span className="flex items-center gap-1">
													<Users className="w-4 h-4" /> {inc.affected_people}
												</span>
												<span className="flex items-center gap-1 text-rose-600 dark:text-rose-400/80">
													<Activity className="w-4 h-4" /> {inc.casualty_count}{" "}
													casualties
												</span>
											</p>
										</div>
										<div className="px-3 py-1 rounded-full text-xs font-bold bg-slate-100 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 uppercase tracking-wider text-slate-800 dark:text-slate-200">
											L{inc.severity_level}
										</div>
									</div>
								</Link>
							))}
							{data.incidents.length === 0 && (
								<div className="p-8 text-center text-slate-500 bg-white/50 dark:bg-slate-900/30 rounded-2xl border border-slate-300 dark:border-slate-800/50 border-dashed">
									No active incidents detected on grid.
								</div>
							)}
						</div>
					</div>

					<div className="space-y-6">
						<h2 className="text-xl font-bold tracking-tight flex items-center gap-3 text-slate-800 dark:text-slate-100">
							<Radio className="w-5 h-5 text-fuchsia-500 dark:text-fuchsia-400" />
							Hub Status
						</h2>
						<div className="bg-white/80 dark:bg-slate-900/40 border border-slate-200 dark:border-slate-800/50 rounded-2xl p-6 space-y-4 shadow-sm dark:shadow-none transition-colors">
							{data.centers.map((center) => (
								<Link
									to="/centers/$centerId"
									params={{ centerId: center.id }}
									key={center.id}
									className="flex items-center justify-between group cursor-pointer"
								>
									<span className="text-sm font-medium text-slate-700 dark:text-slate-300 group-hover:text-indigo-600 dark:group-hover:text-indigo-400 transition-colors">
										{center.name}
									</span>
									<div className="flex items-center gap-2">
										{center.is_core_center && (
											<span className="text-[10px] uppercase font-bold text-indigo-600 dark:text-indigo-400 bg-indigo-100 dark:bg-indigo-500/10 px-2 py-0.5 rounded-full">
												Core
											</span>
										)}
										<div className="w-2 h-2 rounded-full bg-emerald-500 shadow-[0_0_10px_rgba(16,185,129,0.5)]" />
									</div>
								</Link>
							))}
							{data.centers.length === 0 && (
								<div className="text-sm text-slate-500 text-center py-4">
									Grid offline.
								</div>
							)}
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}

interface StatCardProps {
	title: string;
	value: number;
	icon: React.ReactNode;
	color: "rose" | "orange" | "indigo";
	pulse?: boolean;
}

function StatCard({ title, value, icon, color, pulse }: StatCardProps) {
	const colorMap: Record<string, string> = {
		rose: "from-rose-100 to-rose-50 dark:from-rose-500/20 dark:to-rose-500/5 border-rose-200 dark:border-rose-500/20 text-rose-600 dark:text-rose-400",
		orange:
			"from-orange-100 to-orange-50 dark:from-orange-500/20 dark:to-orange-500/5 border-orange-200 dark:border-orange-500/20 text-orange-600 dark:text-orange-400",
		indigo:
			"from-indigo-100 to-indigo-50 dark:from-indigo-500/20 dark:to-indigo-500/5 border-indigo-200 dark:border-indigo-500/20 text-indigo-600 dark:text-indigo-400",
	};

	return (
		<div
			className={`relative overflow-hidden bg-gradient-to-br ${colorMap[color]} border p-6 rounded-2xl backdrop-blur-sm group transition-colors`}
		>
			<div className="flex items-center justify-between mb-4">
				<span className="text-xs font-bold uppercase tracking-widest opacity-80">
					{title}
				</span>
				<div
					className={`p-2 rounded-xl bg-white/50 dark:bg-slate-950/50 ${pulse ? "animate-pulse" : ""}`}
				>
					{icon}
				</div>
			</div>
			<div className="text-4xl font-black text-slate-900 dark:text-white group-hover:scale-105 transition-transform duration-300 origin-left">
				{value}
			</div>
			<div className="absolute -bottom-10 -right-10 opacity-10 scale-150 group-hover:scale-[2] transition-transform duration-700 pointer-events-none">
				{icon}
			</div>
		</div>
	);
}
