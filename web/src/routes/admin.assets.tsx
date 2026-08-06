import { createFileRoute } from "@tanstack/react-router";
import { Truck } from "lucide-react";
import { CreateResourceForm } from "#/components/admin/forms/CreateResourceForm.tsx";
import { ResourcesTable } from "#/components/admin/ResourcesTable.tsx";
import { useAdminStore } from "#/store/adminStore.ts";

export const Route = createFileRoute("/admin/assets")({
	component: AdminAssetsTab,
});

function AdminAssetsTab() {
	const isLoading = useAdminStore((s) => s.isLoading);

	return (
		<div className="grid grid-cols-1 lg:grid-cols-5 gap-6">
			<section className="lg:col-span-2 rounded-3xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/50 p-6 shadow-sm dark:shadow-none">
				<h2 className="text-lg font-bold flex items-center gap-2 mb-4">
					<Truck className="size-5 text-indigo-500" />
					Provision a new asset
				</h2>
				<CreateResourceForm />
			</section>

			<section className="lg:col-span-3 rounded-3xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/50 p-6 shadow-sm dark:shadow-none">
				<h2 className="text-lg font-bold flex items-center gap-2 mb-4">
					Grid assets
				</h2>
				<ResourcesTable loading={isLoading} />
			</section>
		</div>
	);
}
