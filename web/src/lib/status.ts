/**
 * Status enum → human label + Tailwind color classes.
 *
 * Used by badges in the new tables and detail rows so the styling
 * stays consistent.
 */

export type IncidentStatusValue = "ACTIVE" | "DISPATCHED" | "RESOLVED";
export type ResourceStatusValue =
	| "EN_ROUTE"
	| "STUCK"
	| "REJECTED"
	| "COMPLETED";

const INCIDENT_LABELS: Record<IncidentStatusValue, string> = {
	ACTIVE: "Active",
	DISPATCHED: "Dispatched",
	RESOLVED: "Resolved",
};

const RESOURCE_LABELS: Record<ResourceStatusValue, string> = {
	EN_ROUTE: "En route",
	STUCK: "Stuck",
	REJECTED: "Rejected",
	COMPLETED: "Completed",
};

export function incidentStatusLabel(status: IncidentStatusValue): string {
	return INCIDENT_LABELS[status] ?? status;
}

export function resourceStatusLabel(status: ResourceStatusValue): string {
	return RESOURCE_LABELS[status] ?? status.replace(/_/g, " ");
}

export function resourceStatusClasses(status: ResourceStatusValue): string {
	switch (status) {
		case "EN_ROUTE":
			return "bg-sky-500/10 text-sky-600 dark:text-sky-400 border-sky-500/30";
		case "STUCK":
			return "bg-orange-500/10 text-orange-600 dark:text-orange-400 border-orange-500/30";
		case "REJECTED":
			return "bg-rose-500/10 text-rose-600 dark:text-rose-400 border-rose-500/30";
		case "COMPLETED":
			return "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/30";
	}
}

export function incidentStatusClasses(status: IncidentStatusValue): string {
	switch (status) {
		case "ACTIVE":
			return "bg-rose-500/10 text-rose-600 dark:text-rose-400 border-rose-500/30";
		case "DISPATCHED":
			return "bg-indigo-500/10 text-indigo-600 dark:text-indigo-400 border-indigo-500/30";
		case "RESOLVED":
			return "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/30";
	}
}
