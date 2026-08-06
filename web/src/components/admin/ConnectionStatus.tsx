import { RadioTower } from "lucide-react";
import { cn } from "#/lib/utils.ts";

export function ConnectionStatus() {
	return (
		<div
			className={cn(
				"flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-semibold uppercase tracking-widest",
				"border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
			)}
			title="Polling for updates every 30 seconds"
		>
			<RadioTower className="size-3" />
			<span>Live (Polling)</span>
		</div>
	);
}
