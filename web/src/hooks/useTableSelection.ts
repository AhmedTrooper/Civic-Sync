import { useCallback, useState } from "react";
import type { BulkItem } from "#/store/bulkQueueStore.ts";
import { useBulkQueueStore } from "#/store/bulkQueueStore.ts";

/**
 * Shared row-selection state for the three admin tables.
 *
 * Tracks which row IDs are currently checked, exposes
 * `toggle / toggleAll / clear` and an `enqueueSelected` helper that
 * builds `BulkItem[]` for each kind+action and pushes them into the
 * global bulk queue.
 */
export function useTableSelection<TRow>(opts: {
	kind: BulkItem["kind"];
	rows: TRow[];
	idOf: (row: TRow) => string;
	labelOf: (row: TRow) => string;
}) {
	const { kind, rows, idOf, labelOf } = opts;
	const [selected, setSelected] = useState<Set<string>>(new Set());
	const enqueueMany = useBulkQueueStore((s) => s.enqueueMany);

	const toggle = useCallback((id: string) => {
		setSelected((prev) => {
			const next = new Set(prev);
			if (next.has(id)) next.delete(id);
			else next.add(id);
			return next;
		});
	}, []);

	const toggleAll = useCallback(() => {
		setSelected((prev) =>
			prev.size === rows.length ? new Set() : new Set(rows.map((r) => idOf(r))),
		);
	}, [rows, idOf]);

	const clear = useCallback(() => setSelected(new Set()), []);

	const enqueueSelected = useCallback(
		(items: BulkItem[]) => {
			enqueueMany(items);
			setSelected(new Set());
		},
		[enqueueMany],
	);

	const isAllSelected = rows.length > 0 && selected.size === rows.length;

	const buildDeleteItems = useCallback((): BulkItem[] => {
		const lookup = new Map(rows.map((r) => [idOf(r), r]));
		return Array.from(selected)
			.map((id) => lookup.get(id))
			.filter((r): r is TRow => Boolean(r))
			.map((r) => ({
				id: idOf(r),
				kind,
				action: "delete",
				label: labelOf(r),
			}));
	}, [rows, selected, idOf, labelOf, kind]);

	return {
		selected,
		isAllSelected,
		toggle,
		toggleAll,
		clear,
		enqueueSelected,
		buildDeleteItems,
	};
}
