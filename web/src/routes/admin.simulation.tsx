import { createFileRoute } from "@tanstack/react-router";
import { AlertCircle } from "lucide-react";
import { useEffect } from "react";
import { InjectIncidentForm } from "#/components/admin/forms/InjectIncidentForm.tsx";
import { IncidentsTable } from "#/components/admin/IncidentsTable.tsx";
import { useAdminStore } from "#/store/adminStore.ts";

export const Route = createFileRoute("/admin/simulation")({
	component: AdminSimulationTab,
});

function AdminSimulationTab() {
	const isLoading = useAdminStore((s) => s.isLoading);
	useEffect(() => {
		console.log("[AdminSimulationTab] isLoading =", isLoading);
	}, [isLoading]);

	return (
		<div className="grid grid-cols-1 lg:grid-cols-5 gap-6">
			<section className="lg:col-span-2 rounded-3xl border border-rose-200 dark:border-rose-900/30 bg-white dark:bg-slate-900/50 p-6 shadow-sm dark:shadow-none relative overflow-hidden">
				<div className="absolute top-0 right-0 w-32 h-32 bg-rose-500/10 rounded-full blur-3xl -mr-10 -mt-10 pointer-events-none" />
				<h2 className="text-lg font-bold flex items-center gap-2 mb-4 text-rose-600 dark:text-rose-400">
					Inject a crisis
				</h2>
				<InjectIncidentForm />
			</section>

			<section className="lg:col-span-3 rounded-3xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/50 p-6 shadow-sm dark:shadow-none">
				<h2 className="text-lg font-bold flex items-center gap-2 mb-4">
					<AlertCircle className="size-5 text-indigo-500" />
					Active incidents
				</h2>
				<IncidentsTable loading={isLoading} />
			</section>
		</div>
	);
}
