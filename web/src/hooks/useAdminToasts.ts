/**
 * Toast helper for the admin panel.
 *
 * Wraps sonner with the standard success / error / loading patterns so
 * forms and tables can call a single helper instead of duplicating
 * `toast.success(...)` etc. everywhere.
 */

import { toast } from "sonner";

import { ApiError } from "#/lib/apiClient.ts";

export function useAdminToasts() {
	return {
		success(headline: string, description?: string): void {
			if (description) toast.success(headline, { description });
			else toast.success(headline);
		},
		error(
			headline: string,
			errorOrDescription?: unknown,
			fallback?: string,
		): void {
			const description = extractMessage(errorOrDescription, fallback);
			if (description) toast.error(headline, { description });
			else toast.error(headline);
		},
	};
}

function extractMessage(err: unknown, fallback?: string): string | undefined {
	if (!err) return fallback;
	if (err instanceof ApiError) return err.message;
	if (err instanceof Error) return err.message;
	if (typeof err === "string") return err;
	return fallback;
}
