import { ChevronLeft, ChevronRight } from "lucide-react";

import { Button } from "#/components/ui/button.tsx";

interface PaginationProps {
	page: number;
	pageSize: number;
	total: number;
	onPageChange: (page: number) => void;
}

export function Pagination({
	page,
	pageSize,
	total,
	onPageChange,
}: PaginationProps) {
	const totalPages = Math.max(1, Math.ceil(total / pageSize));
	const isFirst = page <= 1;
	const isLast = page >= totalPages;

	if (total === 0) {
		return (
			<div className="flex items-center justify-between px-2 py-3 text-xs text-muted-foreground">
				<span>0 results</span>
			</div>
		);
	}

	const start = (page - 1) * pageSize + 1;
	const end = Math.min(page * pageSize, total);

	return (
		<div className="flex items-center justify-between px-2 py-3 text-xs text-muted-foreground">
			<span>
				Showing <span className="font-semibold">{start}</span>–
				<span className="font-semibold">{end}</span> of{" "}
				<span className="font-semibold">{total}</span>
			</span>
			<div className="flex items-center gap-2">
				<Button
					size="xs"
					variant="outline"
					disabled={isFirst}
					onClick={() => onPageChange(page - 1)}
				>
					<ChevronLeft className="size-3" /> Prev
				</Button>
				<span className="tabular-nums">
					Page {page} / {totalPages}
				</span>
				<Button
					size="xs"
					variant="outline"
					disabled={isLast}
					onClick={() => onPageChange(page + 1)}
				>
					Next <ChevronRight className="size-3" />
				</Button>
			</div>
		</div>
	);
}
