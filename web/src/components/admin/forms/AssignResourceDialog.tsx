import { useEffect, useMemo, useState } from "react";
import { z } from "zod";

import { Button } from "#/components/ui/button.tsx";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "#/components/ui/dialog.tsx";
import { Input } from "#/components/ui/input.tsx";
import { Label } from "#/components/ui/label.tsx";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "#/components/ui/select.tsx";
import { Separator } from "#/components/ui/separator.tsx";
import { useAdminToasts } from "#/hooks/useAdminToasts.ts";
import {
	type Resource,
	ResourceStatusEnum,
	useAdminStore,
} from "#/store/adminStore.ts";

const assignSchema = z.object({
	status: ResourceStatusEnum,
	assigned_incident_id: z.string().nullable(),
	distance_passed_km: z.number().nonnegative(),
	distance_remaining_km: z.number().nonnegative(),
	current_capacity: z.number().int().nonnegative(),
});

interface AssignResourceDialogProps {
	resource: Resource | null;
	open: boolean;
	onOpenChange: (open: boolean) => void;
}

export function AssignResourceDialog({
	resource,
	open,
	onOpenChange,
}: AssignResourceDialogProps) {
	const { incidents, centers, assignResource } = useAdminStore();
	const toasts = useAdminToasts();

	const candidates = useMemo(
		() =>
			incidents.filter(
				(i) => i.status === "ACTIVE" || i.status === "DISPATCHED",
			),
		[incidents],
	);

	const [incidentId, setIncidentId] = useState<string>("");
	const [status, setStatus] = useState<Resource["status"]>("EN_ROUTE");
	const [distancePassed, setDistancePassed] = useState<string>("0");
	const [distanceRemaining, setDistanceRemaining] = useState<string>("0");
	const [capacity, setCapacity] = useState<string>("0");
	const [submitting, setSubmitting] = useState(false);

	// Sync local state whenever a new resource is opened or the dialog
	// is re-opened. This lets the same dialog unassign + reassign.
	useEffect(() => {
		if (!resource) return;
		setIncidentId(resource.assigned_incident_id ?? "");
		setStatus(resource.status);
		setDistancePassed(String(resource.distance_passed_km ?? 0));
		setDistanceRemaining(String(resource.distance_remaining_km ?? 0));
		setCapacity(String(resource.current_capacity ?? 0));
	}, [resource]);

	if (!resource) return null;

	const onSubmit = async () => {
		const parsed = assignSchema.safeParse({
			status,
			assigned_incident_id: incidentId === "" ? null : incidentId,
			distance_passed_km: Number(distancePassed),
			distance_remaining_km: Number(distanceRemaining),
			current_capacity: Number(capacity),
		});
		if (!parsed.success) {
			toasts.error(
				"Invalid input",
				parsed.error.issues[0]?.message ?? "Check the fields and try again.",
			);
			return;
		}
		setSubmitting(true);
		try {
			const result = await assignResource(resource.id, parsed.data);
			if (result) {
				toasts.success(
					parsed.data.assigned_incident_id
						? "Resource assigned"
						: "Resource unassigned",
					resource.unit_identifier,
				);
				onOpenChange(false);
			} else {
				toasts.error("Could not update resource");
			}
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			toasts.error("Could not update resource", message);
		} finally {
			setSubmitting(false);
		}
	};

	const centerName = (id: string) =>
		centers.find((c) => c.id === id)?.name ?? "Unknown";

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-lg">
				<DialogHeader>
					<DialogTitle>
						{incidentId === "" ? "Assign" : "Update"} {resource.unit_identifier}
					</DialogTitle>
					<DialogDescription>
						{resource.resource_type.replace(/_/g, " ")} · status{" "}
						<span className="font-semibold">{resource.status}</span> · cap{" "}
						{resource.current_capacity}/{resource.total_capacity}
					</DialogDescription>
				</DialogHeader>

				<div className="space-y-4">
					<div className="space-y-2">
						<Label htmlFor="assign-incident">Incident</Label>
						<Select value={incidentId} onValueChange={setIncidentId}>
							<SelectTrigger id="assign-incident" className="w-full">
								<SelectValue placeholder="Select an incident…" />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="">— Unassigned —</SelectItem>
								{candidates.map((inc) => (
									<SelectItem key={inc.id} value={inc.id}>
										{inc.title} — {centerName(inc.primary_center_id)} (L
										{inc.severity_level})
									</SelectItem>
								))}
							</SelectContent>
						</Select>
						<p className="text-xs text-muted-foreground">
							Only ACTIVE and DISPATCHED incidents are listed. Pick "Unassigned"
							to clear the binding.
						</p>
					</div>

					<div className="space-y-2">
						<Label htmlFor="assign-status">Status</Label>
						<Select
							value={status}
							onValueChange={(v) => setStatus(v as Resource["status"])}
						>
							<SelectTrigger id="assign-status" className="w-full">
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								{ResourceStatusEnum.options.map((s) => (
									<SelectItem key={s} value={s}>
										{s}
									</SelectItem>
								))}
							</SelectContent>
						</Select>
					</div>

					<Separator />

					<div className="grid grid-cols-3 gap-3">
						<div className="space-y-2">
							<Label htmlFor="assign-passed">Distance passed (km)</Label>
							<Input
								id="assign-passed"
								type="number"
								min={0}
								step="any"
								value={distancePassed}
								onChange={(e) => setDistancePassed(e.target.value)}
							/>
						</div>
						<div className="space-y-2">
							<Label htmlFor="assign-remaining">Distance remaining (km)</Label>
							<Input
								id="assign-remaining"
								type="number"
								min={0}
								step="any"
								value={distanceRemaining}
								onChange={(e) => setDistanceRemaining(e.target.value)}
							/>
						</div>
						<div className="space-y-2">
							<Label htmlFor="assign-capacity">Capacity</Label>
							<Input
								id="assign-capacity"
								type="number"
								min={0}
								step="1"
								value={capacity}
								onChange={(e) => setCapacity(e.target.value)}
							/>
						</div>
					</div>
				</div>

				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>
						Cancel
					</Button>
					<Button onClick={onSubmit} disabled={submitting}>
						{submitting ? "Saving…" : "Save assignment"}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
