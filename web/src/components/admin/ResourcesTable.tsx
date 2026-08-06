import { Link } from "@tanstack/react-router";
import {
	type ColumnDef,
	flexRender,
	getCoreRowModel,
	getFilteredRowModel,
	getPaginationRowModel,
	getSortedRowModel,
	useReactTable,
} from "@tanstack/react-table";
import { Link2, Link2Off, Loader2, MoreHorizontal, Trash2 } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
	AlertDialogTrigger,
} from "#/components/ui/alert-dialog.tsx";
import { Badge } from "#/components/ui/badge.tsx";
import { Button } from "#/components/ui/button.tsx";
import { Checkbox } from "#/components/ui/checkbox.tsx";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuLabel,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu.tsx";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "#/components/ui/select.tsx";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "#/components/ui/table.tsx";
import { useAdminToasts } from "#/hooks/useAdminToasts.ts";
import { useDebouncedValue } from "#/hooks/useDebouncedValue.ts";
import { useTableSelection } from "#/hooks/useTableSelection.ts";
import { resourceStatusClasses, resourceStatusLabel } from "#/lib/status.ts";
import {
	type Resource,
	type ResourceFilters,
	ResourceStatusEnum,
	ResourceTypeEnum,
	useAdminStore,
} from "#/store/adminStore.ts";
import { AssignResourceDialog } from "./forms/AssignResourceDialog.tsx";
import { Pagination } from "./Pagination.tsx";
import { TableSearch } from "./TableSearch.tsx";

interface ResourcesTableProps {
	loading?: boolean;
}

export function ResourcesTable({ loading }: ResourcesTableProps) {
	const { resources, centers, incidents, resourceFilters, setResourceFilters } =
		useAdminStore();
	const [search, setSearch] = useState(resourceFilters.search);
	const debouncedSearch = useDebouncedValue(search, 200);
	const [assignTarget, setAssignTarget] = useState<Resource | null>(null);

	const centerName = useCallback(
		(id: string) => centers.find((c) => c.id === id)?.name ?? "Unknown",
		[centers],
	);

	const filtered = useMemo(() => {
		const q = debouncedSearch.trim().toLowerCase();
		return resources.filter((r) => {
			if (
				resourceFilters.type !== "ALL" &&
				r.resource_type !== resourceFilters.type
			)
				return false;
			if (
				resourceFilters.status !== "ALL" &&
				r.status !== resourceFilters.status
			)
				return false;
			if (
				resourceFilters.ownerCenterId !== "ALL" &&
				r.owner_center_id !== resourceFilters.ownerCenterId
			)
				return false;
			if (q) {
				const hay =
					`${r.unit_identifier} ${centerName(r.owner_center_id)} ${r.resource_type}`.toLowerCase();
				if (!hay.includes(q)) return false;
			}
			return true;
		});
	}, [resources, resourceFilters, debouncedSearch, centerName]);

	const total = filtered.length;
	const page = resourceFilters.page;

	const selection = useTableSelection<Resource>({
		kind: "resource",
		rows: filtered,
		idOf: (r) => r.id,
		labelOf: (r) => r.unit_identifier,
	});

	const columns = useMemo<ColumnDef<Resource>[]>(() => {
		return [
			{
				id: "select",
				header: () => (
					<Checkbox
						checked={selection.isAllSelected}
						onChange={selection.toggleAll}
						aria-label="Select all assets"
					/>
				),
				cell: ({ row }) => (
					<Checkbox
						checked={selection.selected.has(row.original.id)}
						onChange={() => selection.toggle(row.original.id)}
						aria-label={`Select ${row.original.unit_identifier}`}
					/>
				),
				enableSorting: false,
			},
			{
				id: "unit",
				header: "Unit",
				cell: ({ row }) => (
					<Link
						to="/resources/$resourceId"
						params={{ resourceId: row.original.id }}
						className="font-semibold hover:text-indigo-600 dark:hover:text-indigo-400"
					>
						{row.original.unit_identifier}
					</Link>
				),
			},
			{
				accessorKey: "resource_type",
				header: "Type",
				cell: ({ row }) => (
					<span className="text-xs uppercase tracking-widest font-semibold text-muted-foreground">
						{row.original.resource_type.replace(/_/g, " ")}
					</span>
				),
			},
			{
				id: "center",
				header: "Owner center",
				cell: ({ row }) => (
					<span className="text-xs">
						{centerName(row.original.owner_center_id)}
					</span>
				),
			},
			{
				accessorKey: "status",
				header: "Status",
				cell: ({ row }) => (
					<Badge className={resourceStatusClasses(row.original.status)}>
						{resourceStatusLabel(row.original.status)}
					</Badge>
				),
			},
			{
				id: "capacity",
				header: "Capacity",
				cell: ({ row }) =>
					`${row.original.current_capacity}/${row.original.total_capacity}`,
			},
			{
				id: "assigned",
				header: "Assigned to",
				cell: ({ row }) => {
					const inc = incidents.find(
						(i) => i.id === row.original.assigned_incident_id,
					);
					return inc ? (
						<Link
							to="/incidents/$incidentId"
							params={{ incidentId: inc.id }}
							className="text-rose-600 dark:text-rose-400 font-semibold text-xs hover:underline"
						>
							{inc.title}
						</Link>
					) : (
						<span className="text-xs text-muted-foreground italic">
							Unassigned
						</span>
					);
				},
			},
			{
				id: "actions",
				header: "",
				enableSorting: false,
				cell: ({ row }) => (
					<RowActions
						resource={row.original}
						onAssign={(r) => setAssignTarget(r)}
					/>
				),
			},
		];
	}, [incidents, centerName, selection]);

	const table = useReactTable({
		data: filtered,
		columns,
		state: { pagination: { pageIndex: page - 1, pageSize: 25 } },
		onPaginationChange: (updater) => {
			const next =
				typeof updater === "function"
					? updater({ pageIndex: page - 1, pageSize: 25 })
					: updater;
			setResourceFilters({ page: next.pageIndex + 1 });
		},
		getCoreRowModel: getCoreRowModel(),
		getFilteredRowModel: getFilteredRowModel(),
		getSortedRowModel: getSortedRowModel(),
		getPaginationRowModel: getPaginationRowModel(),
	});

	return (
		<>
			<div className="space-y-3">
				<TableSearch
					value={search}
					onChange={(v) => {
						setSearch(v);
						setResourceFilters({ search: v, page: 1 });
					}}
					placeholder="Search by unit, type, or center…"
					filters={
						<>
							<Select
								value={resourceFilters.type}
								onValueChange={(v) =>
									setResourceFilters({
										type: v as ResourceFilters["type"],
										page: 1,
									})
								}
							>
								<SelectTrigger size="sm" className="w-40">
									<SelectValue placeholder="Type" />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="ALL">All types</SelectItem>
									{ResourceTypeEnum.options.map((t) => (
										<SelectItem key={t} value={t}>
											{t.replace(/_/g, " ")}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
							<Select
								value={resourceFilters.status}
								onValueChange={(v) =>
									setResourceFilters({
										status: v as ResourceFilters["status"],
										page: 1,
									})
								}
							>
								<SelectTrigger size="sm" className="w-36">
									<SelectValue placeholder="Status" />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="ALL">All statuses</SelectItem>
									{ResourceStatusEnum.options.map((s) => (
										<SelectItem key={s} value={s}>
											{s}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
							<Select
								value={resourceFilters.ownerCenterId}
								onValueChange={(v) =>
									setResourceFilters({ ownerCenterId: v, page: 1 })
								}
							>
								<SelectTrigger size="sm" className="w-44">
									<SelectValue placeholder="Owner center" />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="ALL">All centers</SelectItem>
									{centers.map((c) => (
										<SelectItem key={c.id} value={c.id}>
											{c.name}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
						</>
					}
				/>

				{selection.selected.size > 0 ? (
					<div className="flex items-center justify-between rounded-2xl border border-indigo-200 bg-indigo-50 px-4 py-2 text-xs text-indigo-700 dark:border-indigo-900/40 dark:bg-indigo-500/10 dark:text-indigo-300">
						<span className="font-semibold">
							{selection.selected.size} selected
						</span>
						<div className="flex items-center gap-2">
							<Button size="xs" variant="outline" onClick={selection.clear}>
								Clear
							</Button>
							<Button
								size="xs"
								variant="destructive"
								onClick={() =>
									selection.enqueueSelected(selection.buildDeleteItems())
								}
							>
								<Trash2 className="size-3" /> Queue delete
							</Button>
						</div>
					</div>
				) : null}

				<div className="rounded-md border bg-card">
					<Table>
						<TableHeader>
							{table.getHeaderGroups().map((hg) => (
								<TableRow key={hg.id}>
									{hg.headers.map((header) => (
										<TableHead key={header.id}>
											{header.isPlaceholder
												? null
												: flexRender(
														header.column.columnDef.header,
														header.getContext(),
													)}
										</TableHead>
									))}
								</TableRow>
							))}
						</TableHeader>
						<TableBody>
							{loading && filtered.length === 0 ? (
								<TableRow>
									<TableCell
										colSpan={columns.length}
										className="text-center py-10"
									>
										<Loader2 className="inline size-4 animate-spin mr-2" />
										Loading resources…
									</TableCell>
								</TableRow>
							) : table.getRowModel().rows.length === 0 ? (
								<TableRow>
									<TableCell
										colSpan={columns.length}
										className="text-center text-sm text-muted-foreground py-10"
									>
										No resources match the current filters.
									</TableCell>
								</TableRow>
							) : (
								table.getRowModel().rows.map((row) => (
									<TableRow key={row.id}>
										{row.getVisibleCells().map((cell) => (
											<TableCell key={cell.id}>
												{flexRender(
													cell.column.columnDef.cell,
													cell.getContext(),
												)}
											</TableCell>
										))}
									</TableRow>
								))
							)}
						</TableBody>
					</Table>
				</div>

				<Pagination
					page={page}
					pageSize={25}
					total={total}
					onPageChange={(p) => setResourceFilters({ page: p })}
				/>
			</div>

			<AssignResourceDialog
				resource={assignTarget}
				open={assignTarget !== null}
				onOpenChange={(open) => {
					if (!open) setAssignTarget(null);
				}}
			/>
		</>
	);
}

function RowActions({
	resource,
	onAssign,
}: {
	resource: Resource;
	onAssign: (r: Resource) => void;
}) {
	const deleteResource = useAdminStore((s) => s.deleteResource);
	const assignResource = useAdminStore((s) => s.assignResource);
	const toasts = useAdminToasts();

	const handleUnassign = async () => {
		try {
			const ok = await assignResource(resource.id, {
				status: resource.status,
				assigned_incident_id: null,
			});
			if (ok) toasts.success("Resource unassigned", resource.unit_identifier);
			else toasts.error("Could not unassign resource");
		} catch (err) {
			toasts.error("Could not unassign resource", err);
		}
	};

	return (
		<div className="flex justify-end">
			<DropdownMenu>
				<DropdownMenuTrigger asChild>
					<Button size="icon-xs" variant="ghost" title="Actions">
						<MoreHorizontal />
					</Button>
				</DropdownMenuTrigger>
				<DropdownMenuContent align="end" className="w-48">
					<DropdownMenuLabel>{resource.unit_identifier}</DropdownMenuLabel>
					<DropdownMenuSeparator />
					<DropdownMenuItem onSelect={() => onAssign(resource)}>
						<Link2 />
						{resource.assigned_incident_id
							? "Update assignment…"
							: "Assign to incident…"}
					</DropdownMenuItem>
					{resource.assigned_incident_id ? (
						<DropdownMenuItem onSelect={handleUnassign}>
							<Link2Off />
							Unassign
						</DropdownMenuItem>
					) : null}
					<DropdownMenuSeparator />
					<AlertDialog>
						<AlertDialogTrigger asChild>
							<DropdownMenuItem
								variant="destructive"
								onSelect={(e) => e.preventDefault()}
							>
								<Trash2 />
								Delete
							</DropdownMenuItem>
						</AlertDialogTrigger>
						<AlertDialogContent>
							<AlertDialogHeader>
								<AlertDialogTitle>Delete resource?</AlertDialogTitle>
								<AlertDialogDescription>
									This permanently removes{" "}
									<strong>{resource.unit_identifier}</strong> from the grid. Any
									incident assignment will be cleared.
								</AlertDialogDescription>
							</AlertDialogHeader>
							<AlertDialogFooter>
								<AlertDialogCancel>Cancel</AlertDialogCancel>
								<AlertDialogAction
									onClick={async () => {
										const ok = await deleteResource(resource.id);
										if (ok)
											toasts.success(
												"Resource deleted",
												resource.unit_identifier,
											);
										else toasts.error("Could not delete resource");
									}}
									className="bg-destructive text-white hover:bg-destructive/90"
								>
									Delete
								</AlertDialogAction>
							</AlertDialogFooter>
						</AlertDialogContent>
					</AlertDialog>
				</DropdownMenuContent>
			</DropdownMenu>
		</div>
	);
}

// re-export so callers don't need a second import for the type
export type { Resource };
