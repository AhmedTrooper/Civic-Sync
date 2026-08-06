import { createFileRoute } from "@tanstack/react-router";
import { MapPin } from "lucide-react";

import { CentersTable } from "#/components/admin/CentersTable.tsx";
import { useAdminStore } from "#/store/adminStore.ts";

export const Route = createFileRoute("/admin/centers")({
	component: AdminCentersTab,
});

function AdminCentersTab() {
	const isLoading = useAdminStore((s) => s.isLoading);
	const centers = useAdminStore((s) => s.centers);

	return (
		<div className="space-y-6">
			<section className="rounded-3xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/50 p-6 shadow-sm dark:shadow-none">
				<div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between mb-4">
					<h2 className="text-lg font-bold flex items-center gap-2">
						<MapPin className="size-5 text-emerald-500" />
						Divisional command centers
					</h2>
					<p className="text-xs text-muted-foreground">
						{centers.length} hub{centers.length === 1 ? "" : "s"} registered
					</p>
				</div>

				<div className="mb-4 rounded-2xl border border-amber-500/30 bg-amber-500/5 p-4 text-xs text-amber-700 dark:text-amber-400">
					The backend exposes only GET + DELETE on <code>/centers</code> (no
					POST or PATCH), and refuses to delete seeded hubs (returns 409).
					Centers can therefore only be added or modified via the API seeder.
				</div>

				<CentersTable loading={isLoading} />
			</section>
		</div>
	);
}
