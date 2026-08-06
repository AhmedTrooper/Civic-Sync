import { Loader2, Trash2, X, Zap } from "lucide-react";
import { useEffect } from "react";

import { Button } from "#/components/ui/button.tsx";
import { useAdminToasts } from "#/hooks/useAdminToasts.ts";
import { useAdminStore } from "#/store/adminStore.ts";
import {
	type BulkItem,
	type BulkKind,
	useBulkQueueStore,
} from "#/store/bulkQueueStore.ts";

const KIND_LABEL: Record<BulkKind, string> = {
	incident: "incident",
	resource: "asset",
	center: "center",
};

export function BulkQueuePanel() {
	const { queue, releasing, remove, clear, setReleasing } = useBulkQueueStore();
	const toasts = useAdminToasts();

	const deleteIncident = useAdminStore((s) => s.deleteIncident);
	const deleteResource = useAdminStore((s) => s.deleteResource);
	const deleteCenter = useAdminStore((s) => s.deleteCenter);

	// Drain the queue sequentially. Each item is processed in turn;
	// individual failures surface as toasts but never abort the rest.
	useEffect(() => {
		if (!releasing) return;
		if (queue.length === 0) {
			setReleasing(false);
			return;
		}

		const item = queue[0];
		const run = async () => {
			try {
				const ok = await runItem(item, {
					deleteIncident,
					deleteResource,
					deleteCenter,
				});
				if (ok) toasts.success(`Removed ${KIND_LABEL[item.kind]}`, item.label);
				else
					toasts.error(`Failed to remove ${KIND_LABEL[item.kind]}`, item.label);
			} finally {
				remove(item.id);
			}
		};
		void run();
	}, [
		releasing,
		queue,
		remove,
		setReleasing,
		deleteIncident,
		deleteResource,
		deleteCenter,
		toasts,
	]);

	if (queue.length === 0 && !releasing) return null;

	const grouped = groupByActionAndKind(queue);

	return (
		<div className="fixed bottom-4 right-4 z-50 w-[min(380px,calc(100vw-2rem))] rounded-2xl border bg-white dark:bg-slate-900 shadow-2xl shadow-slate-900/10 overflow-hidden">
			<header className="flex items-center justify-between gap-2 px-4 py-3 border-b bg-slate-50 dark:bg-slate-950">
				<div className="flex items-center gap-2">
					<Zap className="size-4 text-indigo-500" />
					<h3 className="text-sm font-bold uppercase tracking-widest">
						Bulk queue
					</h3>
					<span className="text-xs font-semibold text-muted-foreground tabular-nums">
						{queue.length}
					</span>
				</div>
				<Button
					size="icon-xs"
					variant="ghost"
					onClick={clear}
					disabled={releasing}
					title="Clear queue"
				>
					<X />
				</Button>
			</header>

			<div className="max-h-64 overflow-y-auto px-2 py-2">
				{Object.entries(grouped).map(([key, items]) => (
					<div key={key} className="mb-2 last:mb-0">
						<p className="px-2 py-1 text-[10px] uppercase tracking-widest font-semibold text-muted-foreground">
							{items[0].action} · {items[0].kind} ({items.length})
						</p>
						{items.map((item) => (
							<div
								key={`${item.kind}-${item.id}`}
								className="flex items-center justify-between gap-2 px-2 py-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800"
							>
								<span className="text-xs truncate">{item.label}</span>
								<Button
									size="icon-xs"
									variant="ghost"
									disabled={releasing}
									onClick={() => remove(item.id)}
									title="Remove from queue"
								>
									<X />
								</Button>
							</div>
						))}
					</div>
				))}
			</div>

			<footer className="flex items-center justify-end gap-2 px-4 py-3 border-t bg-slate-50 dark:bg-slate-950">
				<Button
					size="sm"
					variant="outline"
					onClick={clear}
					disabled={releasing}
				>
					Clear
				</Button>
				<Button
					size="sm"
					variant="destructive"
					onClick={() => setReleasing(true)}
					disabled={releasing || queue.length === 0}
				>
					{releasing ? (
						<>
							<Loader2 className="size-3 animate-spin" /> Releasing…
						</>
					) : (
						<>
							<Trash2 className="size-3" /> Release {queue.length}
						</>
					)}
				</Button>
			</footer>
		</div>
	);
}

interface RunHandlers {
	deleteIncident: (id: string) => Promise<boolean>;
	deleteResource: (id: string) => Promise<boolean>;
	deleteCenter: (id: string) => Promise<boolean>;
}

async function runItem(item: BulkItem, h: RunHandlers): Promise<boolean> {
	switch (item.action) {
		case "delete":
			if (item.kind === "incident") return h.deleteIncident(item.id);
			if (item.kind === "resource") return h.deleteResource(item.id);
			return h.deleteCenter(item.id);
	}
}

function groupByActionAndKind(items: BulkItem[]): Record<string, BulkItem[]> {
	return items.reduce<Record<string, BulkItem[]>>((acc, item) => {
		const key = `${item.action}:${item.kind}`;
		if (!acc[key]) acc[key] = [];
		acc[key].push(item);
		return acc;
	}, {});
}
