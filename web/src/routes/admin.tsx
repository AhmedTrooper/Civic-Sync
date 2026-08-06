import {
	createFileRoute,
	Link,
	Outlet,
	useRouterState,
} from "@tanstack/react-router";
import { Database, Flame, MapPin, Pause, Play, Truck } from "lucide-react";
import { useEffect, useState } from "react";
import { BulkQueuePanel } from "#/components/admin/BulkQueuePanel.tsx";
import { ConnectionStatus } from "#/components/admin/ConnectionStatus.tsx";
import { useAdminStore } from "#/store/adminStore.ts";

const TABS = [
	{
		to: "/admin/simulation" as const,
		label: "Simulation & Crises",
		icon: Flame,
	},
	{ to: "/admin/assets" as const, label: "Grid Assets", icon: Truck },
	{ to: "/admin/centers" as const, label: "Centers", icon: MapPin },
];

export const Route = createFileRoute("/admin")({
	component: AdminLayout,
});

/**
 * SSR-safe layout: tabs and Outlet always render so TanStack Start can
 * resolve child routes during SSR and hydrate the router Link handlers
 * correctly. Zustand store access, WebSocket, and data fetching are
 * deferred to AdminLiveControls which mounts only on the client.
 */
function AdminLayout() {
	console.log("[AdminLayout] Rendered");
	const routerState = useRouterState();
	const currentPath = routerState.location.pathname;

	return (
		<div className="min-h-screen bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-50 p-4 md:p-8 transition-colors duration-300">
			<div className="max-w-7xl mx-auto space-y-6">
				<header className="flex flex-col md:flex-row md:items-center justify-between pb-6 border-b border-slate-200 dark:border-slate-800 gap-4">
					<div className="flex items-center gap-4">
						<div className="relative flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-indigo-500 to-purple-500 shadow-lg">
							<Database className="w-6 h-6 text-white relative z-10" />
						</div>
						<div>
							<h1 className="text-2xl font-bold tracking-tight text-slate-900 dark:text-white">
								System Administration
							</h1>
							<p className="text-sm text-slate-500 dark:text-slate-400">
								Full entity management & simulation controls.
							</p>
						</div>
					</div>

					<AdminLiveControls />
				</header>

				<nav className="flex overflow-x-auto gap-2 pb-2 scrollbar-hide">
					{TABS.map((tab) => {
						const Icon = tab.icon;
						const isActive = currentPath.startsWith(tab.to);
						return (
							<Link
								key={tab.to}
								to={tab.to}
								className={`flex items-center gap-2 px-5 py-3 rounded-2xl text-sm font-bold uppercase tracking-wider transition-all whitespace-nowrap ${
									isActive
										? "bg-indigo-600 text-white shadow-lg shadow-indigo-500/30"
										: "bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800"
								}`}
							>
								<Icon className="size-4" />
								{tab.label}
							</Link>
						);
					})}
				</nav>

				<div className="mt-6">
					<ClientOnlyOutlet />
				</div>
			</div>
			<ClientOnlyBulkQueue />
		</div>
	);
}

/**
 * Client-only wrapper: Zustand store access, WebSocket connection,
 * and polling. Renders connection badge + simulation play/pause.
 * Returns nothing during SSR to avoid Zustand hydration crashes.
 */
function AdminLiveControls() {
	const [mounted, setMounted] = useState(false);
	useEffect(() => {
		console.log("[AdminLiveControls] Mounted");
		setMounted(true);
	}, []);

	if (!mounted) {
		console.log("[AdminLiveControls] Not mounted yet (SSR)");
		return null;
	}

	return <AdminLiveControlsInner />;
}

function AdminLiveControlsInner() {
	console.log("[AdminLiveControlsInner] Rendered");
	const { simStatus, fetchData, toggleSimulation } = useAdminStore();

	useEffect(() => {
		console.log("[AdminLiveControlsInner] Initial fetchData call");
		void fetchData();
	}, [fetchData]);

	useEffect(() => {
		console.log("[AdminLiveControlsInner] Setting up polling interval");
		const id = window.setInterval(() => {
			console.log("[AdminLiveControlsInner] Polling tick");
			void fetchData();
		}, 30_000);
		return () => {
			console.log("[AdminLiveControlsInner] Clearing polling interval");
			window.clearInterval(id);
		};
	}, [fetchData]);

	return (
		<div className="flex items-center gap-3">
			<ConnectionStatus />
			{simStatus ? (
				<div className="flex items-center gap-3 bg-white/80 dark:bg-slate-900/80 p-3 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm">
					<div className="text-right">
						<p className="text-xs font-bold uppercase tracking-widest text-slate-500">
							Simulation
						</p>
						<p
							className={`text-sm font-bold ${simStatus.paused ? "text-orange-500" : "text-emerald-500 animate-pulse"}`}
						>
							{simStatus.paused ? "PAUSED" : "ACTIVE"}
						</p>
					</div>
					<button
						type="button"
						onClick={() => {
							void toggleSimulation();
						}}
						className={`p-3 rounded-xl transition-all ${
							simStatus.paused
								? "bg-emerald-100 text-emerald-600 hover:bg-emerald-200 dark:bg-emerald-500/20 dark:hover:bg-emerald-500/30 dark:text-emerald-400"
								: "bg-orange-100 text-orange-600 hover:bg-orange-200 dark:bg-orange-500/20 dark:hover:bg-orange-500/30 dark:text-orange-400"
						}`}
						aria-label={
							simStatus.paused ? "Resume simulation" : "Pause simulation"
						}
					>
						{simStatus.paused ? (
							<Play className="w-5 h-5 fill-current" />
						) : (
							<Pause className="w-5 h-5 fill-current" />
						)}
					</button>
				</div>
			) : null}
		</div>
	);
}

/** Child routes use Zustand — render Outlet client-only. */
function ClientOnlyOutlet() {
	const [mounted, setMounted] = useState(false);
	useEffect(() => {
		console.log("[ClientOnlyOutlet] Mounted");
		setMounted(true);
	}, []);
	if (!mounted) {
		console.log("[ClientOnlyOutlet] Not mounted yet (SSR)");
		return null;
	}
	console.log("[ClientOnlyOutlet] Rendering Outlet");
	return <Outlet />;
}

/** BulkQueuePanel uses Zustand — mount client-only. */
function ClientOnlyBulkQueue() {
	const [mounted, setMounted] = useState(false);
	useEffect(() => {
		setMounted(true);
	}, []);
	if (!mounted) return null;
	return <BulkQueuePanel />;
}
