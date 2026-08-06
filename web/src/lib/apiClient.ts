/**
 * Single source of truth for talking to the Civic-Sync API.
 *
 * - Centralizes the base URL (replaceable via `VITE_API_BASE`).
 * - Injects the `x-role: admin` header on every mutation.
 * - Throws an `ApiError` on any non-2xx response with the parsed
 *   `{ code, message }` body so callers can surface server messages
 *   inside toasts / inline form errors.
 */

export const API_BASE: string =
	(import.meta.env.VITE_API_BASE as string | undefined) ?? "";

export class ApiError extends Error {
	readonly status: number;
	readonly code: string | null;

	constructor(message: string, status: number, code: string | null) {
		super(message);
		this.name = "ApiError";
		this.status = status;
		this.code = code;
	}
}

type Method = "GET" | "POST" | "PATCH" | "PUT" | "DELETE";

export interface ApiFetchOptions extends Omit<RequestInit, "method"> {
	method?: Method;
	/** When true, attach `x-role: admin`. Defaults to true for non-GET. */
	auth?: boolean;
	/** Override the role header value (defaults to "admin"). */
	role?: string;
}

function buildUrl(path: string): string {
	const left = API_BASE.replace(/\/+$/, "");
	const right = path.startsWith("/") ? path : `/${path}`;
	return `${left}${right}`;
}

export async function apiFetch<T = unknown>(
	path: string,
	options: ApiFetchOptions = {},
): Promise<T | null> {
	const { method = "GET", auth, role = "admin", headers, ...rest } = options;

	const finalHeaders = new Headers(headers);
	const isMutation = method !== "GET";
	if (auth === true || (auth !== false && isMutation)) {
		finalHeaders.set("x-role", role);
	}
	if (rest.body && !finalHeaders.has("content-type")) {
		finalHeaders.set("content-type", "application/json");
	}

	const targetUrl = buildUrl(path);
	console.log(`[apiClient] -> ${method} ${targetUrl}`, {
		auth,
		role,
		headers: finalHeaders,
	});

	const res = await fetch(targetUrl, {
		...rest,
		method,
		headers: finalHeaders,
	});

	console.log(
		`[apiClient] <- ${method} ${targetUrl} [${res.status} ${res.statusText}]`,
	);

	if (res.status === 204) return null;

	const text = await res.text();
	const data: unknown = text ? safeJson(text) : null;

	if (!res.ok) {
		const message =
			(isRecord(data) && typeof data.message === "string"
				? data.message
				: null) ?? `${res.status} ${res.statusText}`;
		const code =
			isRecord(data) && typeof data.code === "string" ? data.code : null;
		console.error(`[apiClient] Error on ${path}:`, {
			message,
			code,
			status: res.status,
		});
		throw new ApiError(message, res.status, code);
	}

	console.log(`[apiClient] Data from ${path}:`, data);
	return (data ?? null) as T;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function safeJson(text: string): unknown {
	try {
		return JSON.parse(text);
	} catch {
		return text;
	}
}
