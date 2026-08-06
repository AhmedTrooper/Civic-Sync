import type * as React from "react";

import { cn } from "#/lib/utils.ts";

interface CheckboxProps extends React.ComponentProps<"input"> {
	type?: never;
}

/**
 * Minimal shadcn-style checkbox.
 *
 * We deliberately use a native `<input type="checkbox">` (rather than
 * the heavier Radix Checkbox primitive) because the bulk queue only
 * needs a clickable, accessible checkbox — no indeterminate
 * animation, no keyboard-focus trap, no portal.
 */
function Checkbox({ className, ...props }: CheckboxProps) {
	return (
		<input
			type="checkbox"
			data-slot="checkbox"
			className={cn(
				"size-4 shrink-0 rounded-sm border border-input bg-background shadow-xs transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 accent-indigo-600 dark:bg-input/30",
				className,
			)}
			{...props}
		/>
	);
}

export { Checkbox };
