import { useCallback, useState } from "react";
import type { UseFormSetValue } from "react-hook-form";

import type { Center } from "#/store/adminStore.ts";

/**
 * Shared auto-fill helper for the Inject-Crisis and Asset-Create forms.
 *
 * Behavior:
 *  - When the center selector changes, look up that center's coords
 *    and write them into the lat / lng fields of the form.
 *  - If the user has manually edited either coordinate, leave them
 *    alone and let the user keep their values.
 *  - Returns a `resetToCenter()` function that re-enables auto-fill
 *    (so users can recover from a manual override).
 *
 * Generic constraints: the form values must contain numeric `latitude`
 * and `longitude` fields.
 */
export function useCenterAutoFill<T extends Record<string, unknown>>(
	setValue: UseFormSetValue<T>,
) {
	const [coordsTouched, setCoordsTouched] = useState(false);

	const syncFromCenter = useCallback(
		(centerId: string, centers: Center[]) => {
			const c = centers.find((x) => x.id === centerId);
			if (!c) return;
			if (coordsTouched) return;
			setValue(
				"latitude" as unknown as Parameters<UseFormSetValue<T>>[0],
				Number(c.latitude) as unknown as Parameters<UseFormSetValue<T>>[1],
				{ shouldDirty: false },
			);
			setValue(
				"longitude" as unknown as Parameters<UseFormSetValue<T>>[0],
				Number(c.longitude) as unknown as Parameters<UseFormSetValue<T>>[1],
				{ shouldDirty: false },
			);
		},
		[coordsTouched, setValue],
	);

	const markTouched = useCallback(() => setCoordsTouched(true), []);

	const resetToCenter = useCallback(
		(centerId: string, centers: Center[]) => {
			setCoordsTouched(false);
			syncFromCenter(centerId, centers);
		},
		[syncFromCenter],
	);

	return { syncFromCenter, markTouched, resetToCenter, coordsTouched };
}
