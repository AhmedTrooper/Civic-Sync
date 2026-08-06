import { useEffect, useRef } from "react";

import { useAdminStore } from "#/store/adminStore.ts";

/**
 * Real-time updates for the admin panel.
 *
 * The Rust backend broadcasts a `{type: "flush", notice: {marks: [...]}}`
 * frame over `/api/v1/sync/ws` whenever a 60-second flush window
 * contained at least one mutation. Marks carry only `{kind, id}` —
 * not the full entity — so on receipt we re-fetch everything to
 * reconcile. This avoids the previous "refetch the world every 5s"
 * behaviour while staying correct.
 *
 * Reconnect: exponential backoff (1s → 30s). After 3 failed connects
 * we silently fall back to 10s polling (handled by the caller; this
 * hook only sets `wsConnected` so the layout can switch).
 */
export function useWebSocket(url: string) {
	const setWsConnected = useAdminStore((s) => s.setWsConnected);
	const fetchData = useAdminStore((s) => s.fetchData);

	const wsRef = useRef<WebSocket | null>(null);
	const retryRef = useRef(0);
	const timerRef = useRef<number | null>(null);
	const cancelledRef = useRef(false);

	useEffect(() => {
		cancelledRef.current = false;

		function connect() {
			if (cancelledRef.current) return;
			let socket: WebSocket;
			try {
				socket = new WebSocket(url);
			} catch (err) {
				console.warn("[ws] failed to construct", err);
				scheduleReconnect();
				return;
			}
			wsRef.current = socket;

			socket.addEventListener("open", () => {
				retryRef.current = 0;
				setWsConnected(true);
			});

			socket.addEventListener("message", (event) => {
				try {
					const payload = JSON.parse(String(event.data)) as {
						type?: string;
						notice?: { marks?: unknown[] };
					};
					if (payload.type === "flush" && payload.notice?.marks?.length) {
						void fetchData();
					}
				} catch {
					// ignore malformed frames
				}
			});

			socket.addEventListener("close", () => {
				setWsConnected(false);
				wsRef.current = null;
				scheduleReconnect();
			});

			socket.addEventListener("error", () => {
				// Let close handler deal with reconnect.
				try {
					socket.close();
				} catch {
					/* ignore */
				}
			});
		}

		function scheduleReconnect() {
			if (cancelledRef.current) return;
			retryRef.current = Math.min(retryRef.current + 1, 5);
			const delay = Math.min(1000 * 2 ** (retryRef.current - 1), 30_000);
			timerRef.current = window.setTimeout(connect, delay);
		}

		connect();

		return () => {
			cancelledRef.current = true;
			if (timerRef.current) window.clearTimeout(timerRef.current);
			if (wsRef.current) {
				try {
					wsRef.current.close();
				} catch {
					/* ignore */
				}
				wsRef.current = null;
			}
			setWsConnected(false);
		};
	}, [url, setWsConnected, fetchData]);
}
