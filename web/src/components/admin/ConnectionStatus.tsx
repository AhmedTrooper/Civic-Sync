import { Loader2, Radio, RadioTower } from "lucide-react";
import { cn } from "#/lib/utils.ts";

interface ConnectionStatusProps {
	connected: boolean;
}

export function ConnectionStatus({ connected }: ConnectionStatusProps) {
	return (
		<div
			className={cn(
				"flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-semibold uppercase tracking-widest",
				connected
					? "border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
					: "border-orange-500/30 bg-orange-500/10 text-orange-600 dark:text-orange-400",
			)}
			title={
				connected
					? "Receiving live updates via WebSocket"
					: "Polling fallback active"
			}
		>
			{connected ? (
				<>
					<Radio className="size-3 animate-pulse" />
					<span>Live</span>
				</>
			) : (
				<>
					<Loader2 className="size-3 animate-spin" />
					<RadioTower className="size-3" />
					<span>Reconnecting…</span>
				</>
			)}
		</div>
	);
}
