import { zodResolver } from "@hookform/resolvers/zod";
import { Flame } from "lucide-react";
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
import { Slider } from "#/components/ui/slider.tsx";
import { useAdminToasts } from "#/hooks/useAdminToasts.ts";
import { useCenterAutoFill } from "#/hooks/useCenterAutoFill.ts";
import { severityClasses, severityLabel } from "#/lib/severity.ts";
import { useAdminStore } from "#/store/adminStore.ts";

const schema = z
	.object({
		title: z.string().min(3, "Title must be at least 3 characters"),
		primary_center_id: z.string().uuid().optional(),
		severity_level: z.number().int().min(1).max(5),
		affected_people: z.number().int().nonnegative(),
		casualty_count: z.number().int().nonnegative(),
		latitude: z.number().min(-90).max(90),
		longitude: z.number().min(-180).max(180),
	})
	.superRefine((data, ctx) => {
		if (data.casualty_count > data.affected_people) {
			ctx.addIssue({
				code: z.ZodIssueCode.custom,
				path: ["casualty_count"],
				message: "Casualties cannot exceed affected count",
			});
		}
	});

type InjectIncidentValues = z.infer<typeof schema>;

interface InjectIncidentFormProps {
	onCreated?: () => void;
}

export function InjectIncidentForm({ onCreated }: InjectIncidentFormProps) {
	const { centers, injectIncident } = useAdminStore();
	const toasts = useAdminToasts();
	const defaultCenterId = centers[0]?.id ?? "";

	const form = useForm<InjectIncidentValues>({
		resolver: zodResolver(schema),
		defaultValues: {
			title: "",
			primary_center_id: defaultCenterId,
			severity_level: 5,
			affected_people: 1500,
			casualty_count: 50,
			latitude: 24.8949,
			longitude: 91.8687,
		},
	});

	const { syncFromCenter, markTouched, resetToCenter, coordsTouched } =
		useCenterAutoFill(form.setValue);

	const watchedCenter = form.watch("primary_center_id");

	useEffect(() => {
		const id = form.watch("primary_center_id");
		if (id) syncFromCenter(id, centers);
	}, [centers, form, syncFromCenter]);

	const severity = form.watch("severity_level");

	return (
		<form
			className="space-y-4"
			onSubmit={form.handleSubmit(async (values) => {
				try {
					// Backend ignores primary_center_id on inject (it derives the
					// nearest hub server-side), but we still send it so the user
					// has an explicit choice. We send only the fields the API
					// accepts on /admin/simulation/inject.
					const created = await injectIncident({
						title: values.title,
						severity_level: values.severity_level,
						affected_people: values.affected_people,
						casualty_count: values.casualty_count,
						latitude: values.latitude,
						longitude: values.longitude,
					});
					if (created) {
						toasts.success("Crisis injected", created.title);
						form.reset({
							title: "",
							primary_center_id: form.getValues("primary_center_id"),
							severity_level: form.getValues("severity_level"),
							affected_people: form.getValues("affected_people"),
							casualty_count: form.getValues("casualty_count"),
							latitude: form.getValues("latitude"),
							longitude: form.getValues("longitude"),
						});
						onCreated?.();
					}
				} catch (err) {
					toasts.error("Could not inject crisis", err);
				}
			})}
		>
			<p className="text-xs text-muted-foreground">
				Manually inject a crisis into the running simulation. Coordinates
				default to the chosen center's location but can be edited below.
			</p>

			<div className="space-y-2">
				<Label htmlFor="title">Crisis title</Label>
				<Input
					id="title"
					placeholder="e.g. Flash flood in Sylhet"
					{...form.register("title")}
					aria-invalid={!!form.formState.errors.title}
				/>
				{form.formState.errors.title ? (
					<p className="text-xs text-rose-600 dark:text-rose-400">
						{form.formState.errors.title.message}
					</p>
				) : null}
			</div>

			<div className="space-y-2">
				<Label htmlFor="primary_center_id">Primary center</Label>
				<Select
					value={form.watch("primary_center_id")}
					onValueChange={(value) => {
						form.setValue("primary_center_id", value, { shouldDirty: true });
						syncFromCenter(value, centers);
					}}
				>
					<SelectTrigger id="primary_center_id" className="w-full">
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
				<p className="text-xs text-muted-foreground">
					The backend computes the nearest hub from the coordinates; this
					selector also seeds lat/lng for you.
				</p>
			</div>

			<div className="space-y-2">
				<div className="flex items-center justify-between">
					<Label htmlFor="severity_level">Severity</Label>
					<span
						className={`inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-semibold ${severityClasses(severity)}`}
					>
						{severityLabel(severity)}
					</span>
				</div>
				<Slider
					id="severity_level"
					min={1}
					max={5}
					step={1}
					value={[form.watch("severity_level")]}
					onValueChange={(value) =>
						form.setValue("severity_level", value[0] ?? 1, {
							shouldDirty: true,
						})
					}
				/>
				<p className="text-xs text-muted-foreground">
					1 = minor incident · 5 = catastrophic event with mass casualties.
				</p>
			</div>

			<div className="grid grid-cols-2 gap-4">
				<div className="space-y-2">
					<Label htmlFor="affected_people">Affected people</Label>
					<Input
						id="affected_people"
						type="number"
						min={0}
						step={1}
						{...form.register("affected_people", { valueAsNumber: true })}
						aria-invalid={!!form.formState.errors.affected_people}
					/>
					{form.formState.errors.affected_people ? (
						<p className="text-xs text-rose-600 dark:text-rose-400">
							{form.formState.errors.affected_people.message}
						</p>
					) : null}
				</div>
				<div className="space-y-2">
					<Label htmlFor="casualty_count">Casualties</Label>
					<Input
						id="casualty_count"
						type="number"
						min={0}
						step={1}
						{...form.register("casualty_count", { valueAsNumber: true })}
						aria-invalid={!!form.formState.errors.casualty_count}
					/>
					{form.formState.errors.casualty_count ? (
						<p className="text-xs text-rose-600 dark:text-rose-400">
							{form.formState.errors.casualty_count.message}
						</p>
					) : null}
				</div>
			</div>

			<fieldset className="space-y-2">
				<div className="flex items-center justify-between">
					<Label>Coordinates</Label>
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
					<Input
						placeholder="Latitude"
						type="number"
						step="any"
						{...form.register("latitude", { valueAsNumber: true })}
						onChange={(e) => {
							markTouched();
							form.setValue("latitude", Number(e.target.value), {
								shouldDirty: true,
							});
						}}
					/>
					<Input
						placeholder="Longitude"
						type="number"
						step="any"
						{...form.register("longitude", { valueAsNumber: true })}
						onChange={(e) => {
							markTouched();
							form.setValue("longitude", Number(e.target.value), {
								shouldDirty: true,
							});
						}}
					/>
				</div>
			</fieldset>

			<Button
				type="submit"
				variant="destructive"
				className="w-full"
				disabled={form.formState.isSubmitting}
			>
				<Flame className="size-4" /> Deploy crisis
			</Button>
		</form>
	);
}
