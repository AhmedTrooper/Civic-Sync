import { useEffect, useState } from "react";

/**
 * Returns a value that only updates after `delayMs` of stability. Used
 * to throttle search inputs in the admin tables so we don't re-filter
 * the list on every keystroke.
 */
export function useDebouncedValue<T>(value: T, delayMs = 200): T {
	const [debounced, setDebounced] = useState(value);

	useEffect(() => {
		const id = window.setTimeout(() => setDebounced(value), delayMs);
		return () => window.clearTimeout(id);
	}, [value, delayMs]);

	return debounced;
}
