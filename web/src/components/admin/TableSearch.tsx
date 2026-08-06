import { Search } from "lucide-react";
import type { ReactNode } from "react";

import { Input } from "#/components/ui/input.tsx";

interface TableSearchProps {
	value: string;
	onChange: (value: string) => void;
	placeholder?: string;
	className?: string;
	filters?: ReactNode;
}

export function TableSearch({
	value,
	onChange,
	placeholder = "Search…",
	className,
	filters,
}: TableSearchProps) {
	return (
		<div
			className={
				"flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between" +
				(className ? ` ${className}` : "")
			}
		>
			<div className="relative w-full sm:max-w-xs">
				<Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
				<Input
					value={value}
					onChange={(e) => onChange(e.target.value)}
					placeholder={placeholder}
					className="pl-9"
				/>
			</div>
			{filters ? (
				<div className="flex flex-wrap items-center gap-2">{filters}</div>
			) : null}
		</div>
	);
}
