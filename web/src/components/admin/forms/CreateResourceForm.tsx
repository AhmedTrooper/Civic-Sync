import { zodResolver } from "@hookform/resolvers/zod";
import { Truck } from "lucide-react";
import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { Button } from "#/components/ui/button.tsx";
import { Input } from "#/components/ui/input.tsx";
import { Label } from "#/components/ui/label.tsx";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "#/components/ui/select.tsx";
import { useAdminToasts } from "#/hooks/useAdminToasts.ts";
import { useCenterAutoFill } from "#/hooks/useCenterAutoFill.ts";
import { ResourceTypeEnum, useAdminStore } from "#/store/adminStore.ts";

const schema = z.object({
	unit_identifier: z.string().min(2, "Must be at least 2 characters").max(50),
	owner_center_id: z.string().uuid("Choose an owner center"),
	resource_type: ResourceTypeEnum,
	total_capacity: z
		.number("Total capacity must be a number")
		.int()
		.min(1, "Must be at least 1"),
	latitude: z.number().min(-90).max(90),
	longitude: z.number().min(-180).max(180),
});

type CreateResourceValues = z.infer<typeof schema>;

interface CreateResourceFormProps {
	onCreated?: () => void;
}

export function CreateResourceForm({ onCreated }: CreateResourceFormProps) {
	const { centers, createResource } = useAdminStore();
	const toasts = useAdminToasts();

	const defaultCenterId = centers[0]?.id ?? "";

	const form = useForm<CreateResourceValues>({
		resolver: zodResolver(schema),
		defaultValues: {
			unit_identifier: "",
			owner_center_id: defaultCenterId,
			resource_type: "AMBULANCE",
			total_capacity: 1,
			latitude: 23.8103,
			longitude: 90.4125,
		},
	});

	const { syncFromCenter, markTouched, resetToCenter, coordsTouched } =
		useCenterAutoFill(form.setValue);

	// Re-sync when the center list grows (e.g. another center was created).
	useEffect(() => {
		const id = form.watch("owner_center_id");
		if (id) syncFromCenter(id, centers);
	}, [centers, form, syncFromCenter]);

	const watchedCenter = form.watch("owner_center_id");

	return (
		<form
			className="space-y-4"
			onSubmit={form.handleSubmit(async (values) => {
				try {
					const created = await createResource(values);
					if (created) {
						toasts.success("Asset provisioned", created.unit_identifier);
						form.reset({
							unit_identifier: "",
							owner_center_id: form.getValues("owner_center_id"),
							resource_type: form.getValues("resource_type"),
							total_capacity: form.getValues("total_capacity"),
							latitude: form.getValues("latitude"),
							longitude: form.getValues("longitude"),
						});
						onCreated?.();
					}
				} catch (err) {
					toasts.error("Could not provision asset", err);
				}
			})}
		>
			<p className="text-xs text-muted-foreground">
				Coordinates default to the owning center's location. Adjust only if this
				unit deploys from a different depot.
			</p>

			<div className="space-y-2">
				<Label htmlFor="unit_identifier">Unit identifier</Label>
				<Input
					id="unit_identifier"
					placeholder="e.g. MED-01"
					{...form.register("unit_identifier")}
					aria-invalid={!!form.formState.errors.unit_identifier}
				/>
				{form.formState.errors.unit_identifier ? (
					<p className="text-xs text-rose-600 dark:text-rose-400">
						{form.formState.errors.unit_identifier.message}
					</p>
				) : null}
			</div>

			<div className="space-y-2">
				<Label htmlFor="owner_center_id">Owner center</Label>
				<Select
					value={form.watch("owner_center_id")}
					onValueChange={(value) => {
						form.setValue("owner_center_id", value, { shouldDirty: true });
						syncFromCenter(value, centers);
					}}
				>
					<SelectTrigger id="owner_center_id" className="w-full">
						<SelectValue placeholder="Choose a center…" />
					</SelectTrigger>
					<SelectContent>
						{centers.map((c) => (
							<SelectItem key={c.id} value={c.id}>
								{c.name}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
				{form.formState.errors.owner_center_id ? (
					<p className="text-xs text-rose-600 dark:text-rose-400">
						{form.formState.errors.owner_center_id.message}
					</p>
				) : null}
			</div>

			<div className="grid grid-cols-2 gap-4">
				<div className="space-y-2">
					<Label htmlFor="resource_type">Type</Label>
					<Select
						value={form.watch("resource_type")}
						onValueChange={(value) =>
							form.setValue(
								"resource_type",
								value as CreateResourceValues["resource_type"],
								{
									shouldDirty: true,
								},
							)
						}
					>
						<SelectTrigger id="resource_type" className="w-full">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							{ResourceTypeEnum.options.map((t) => (
								<SelectItem key={t} value={t}>
									{t.replace(/_/g, " ")}
								</SelectItem>
							))}
						</SelectContent>
					</Select>
				</div>
				<div className="space-y-2">
					<Label htmlFor="total_capacity">Total capacity</Label>
					<Input
						id="total_capacity"
						type="number"
						min={1}
						step={1}
						{...form.register("total_capacity", { valueAsNumber: true })}
						aria-invalid={!!form.formState.errors.total_capacity}
					/>
					{form.formState.errors.total_capacity ? (
						<p className="text-xs text-rose-600 dark:text-rose-400">
							{form.formState.errors.total_capacity.message}
						</p>
					) : null}
				</div>
			</div>

			<fieldset className="space-y-2">
				<div className="flex items-center justify-between">
					<Label>Initial coordinates</Label>
					{coordsTouched ? (
						<button
							type="button"
							onClick={() => resetToCenter(watchedCenter, centers)}
							className="text-xs font-medium text-indigo-600 dark:text-indigo-400 hover:underline"
						>
							Reset to center coords
						</button>
					) : null}
				</div>
				<div className="grid grid-cols-2 gap-4">
					<div className="space-y-2">
						<Label
							htmlFor="latitude"
							className="text-[10px] uppercase tracking-widest text-muted-foreground"
						>
							Latitude
						</Label>
						<Input
							id="latitude"
							type="number"
							step="any"
							{...form.register("latitude", { valueAsNumber: true })}
							onChange={(e) => {
								markTouched();
								form.setValue("latitude", Number(e.target.value), {
									shouldDirty: true,
								});
							}}
							aria-invalid={!!form.formState.errors.latitude}
						/>
					</div>
					<div className="space-y-2">
						<Label
							htmlFor="longitude"
							className="text-[10px] uppercase tracking-widest text-muted-foreground"
						>
							Longitude
						</Label>
						<Input
							id="longitude"
							type="number"
							step="any"
							{...form.register("longitude", { valueAsNumber: true })}
							onChange={(e) => {
								markTouched();
								form.setValue("longitude", Number(e.target.value), {
									shouldDirty: true,
								});
							}}
							aria-invalid={!!form.formState.errors.longitude}
						/>
					</div>
				</div>
				{form.formState.errors.latitude || form.formState.errors.longitude ? (
					<p className="text-xs text-rose-600 dark:text-rose-400">
						Coordinates must be valid lat/lng numbers.
					</p>
				) : null}
			</fieldset>

			<Button
				type="submit"
				className="w-full"
				disabled={form.formState.isSubmitting}
			>
				<Truck className="size-4" /> Provision asset
			</Button>
		</form>
	);
}
