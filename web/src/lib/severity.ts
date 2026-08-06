/**
 * Severity (1-5) → Tailwind color classes.
 *
 * Single source of truth so badges, list rows, and detail pages all
 * agree. Mirrors the emerald → rose palette that was duplicated across
 * the original routes.
 */

export type SeverityLevel = 1 | 2 | 3 | 4 | 5;

export function severityClasses(level: number): string {
	switch (level) {
		case 1:
			return "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/30";
		case 2:
			return "bg-lime-500/10 text-lime-600 dark:text-lime-400 border-lime-500/30";
		case 3:
			return "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/30";
		case 4:
			return "bg-orange-500/10 text-orange-600 dark:text-orange-400 border-orange-500/30";
		default:
			return "bg-rose-500/10 text-rose-600 dark:text-rose-400 border-rose-500/30";
	}
}

export function severityLabel(level: number): string {
	return `L${level}`;
}
