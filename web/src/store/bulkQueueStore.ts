/**
 * Frontend-only bulk-action queue.
 *
 * The backend has no bulk endpoints, so admins can select rows across
 * the three admin tables, queue them here, and "release" them
 * sequentially. Each item is processed one at a time with its own
 * success / error toast so a single failure does not abort the rest.
 */

import { create } from "zustand";

export type BulkKind = "incident" | "resource" | "center";
export type BulkAction = "delete";

export interface BulkItem {
	id: string;
	kind: BulkKind;
	action: BulkAction;
	label: string;
}

interface BulkQueueState {
	queue: BulkItem[];
	releasing: boolean;
	enqueue: (item: BulkItem) => void;
	enqueueMany: (items: BulkItem[]) => void;
	remove: (id: string) => void;
	clear: () => void;
	setReleasing: (releasing: boolean) => void;
}

export const useBulkQueueStore = create<BulkQueueState>((set) => ({
	queue: [],
	releasing: false,
	enqueue: (item) =>
		set((s) =>
			s.queue.some(
				(q) =>
					q.id === item.id && q.kind === item.kind && q.action === item.action,
			)
				? s
				: { queue: [...s.queue, item] },
		),
	enqueueMany: (items) =>
		set((s) => {
			const seen = new Set(s.queue.map((q) => `${q.kind}:${q.action}:${q.id}`));
			const additions = items.filter(
				(i) => !seen.has(`${i.kind}:${i.action}:${i.id}`),
			);
			return additions.length ? { queue: [...s.queue, ...additions] } : s;
		}),
	remove: (id) => set((s) => ({ queue: s.queue.filter((q) => q.id !== id) })),
	clear: () => set({ queue: [] }),
	setReleasing: (releasing) => set({ releasing }),
}));
