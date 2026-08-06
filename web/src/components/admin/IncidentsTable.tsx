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
import { Loader2, Trash2 } from "lucide-react";
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
import { severityClasses, severityLabel } from "#/lib/severity.ts";
import { incidentStatusClasses, incidentStatusLabel } from "#/lib/status.ts";
import {
	type Center,
	type Incident,
	type IncidentFilters,
	IncidentStatusEnum,
	useAdminStore,
} from "#/store/adminStore.ts";
import { Pagination } from "./Pagination.tsx";
import { TableSearch } from "./TableSearch.tsx";

interface IncidentsTableProps {
	loading?: boolean;
}

export function IncidentsTable({ loading }: IncidentsTableProps) {
	const { incidents, centers, incidentFilters, setIncidentFilters } =
		useAdminStore();
	const [search, setSearch] = useState(incidentFilters.search);
	const debouncedSearch = useDebouncedValue(search, 200);

	const centerName = useCallback(
		(id: string) => centers.find((c) => c.id === id)?.name ?? "Unknown",
		[centers],
	);

	const filtered = useMemo(() => {
		const q = debouncedSearch.trim().toLowerCase();
		return incidents.filter((inc) => {
			if (
				incidentFilters.severity !== "ALL" &&
				inc.severity_level !== incidentFilters.severity
			)
				return false;
			if (
				incidentFilters.status !== "ALL" &&
				inc.status !== incidentFilters.status
			)
				return false;
			if (q) {
				const hay =
					`${inc.title} ${centerName(inc.primary_center_id)}`.toLowerCase();
				if (!hay.includes(q)) return false;
			}
			return true;
		});
	}, [incidents, debouncedSearch, incidentFilters, centerName]);

	const total = filtered.length;
	const page = incidentFilters.page;

	const selection = useTableSelection<Incident>({
		kind: "incident",
		rows: filtered,
		idOf: (inc) => inc.id,
		labelOf: (inc) => inc.title,
	});

	const columns = useMemo<ColumnDef<Incident>[]>(() => {
		const columns: ColumnDef<Incident>[] = [
			{
				id: "select",
				header: () => (
					<Checkbox
						checked={selection.isAllSelected}
						onChange={selection.toggleAll}
						aria-label="Select all incidents"
					/>
				),
				cell: ({ row }) => (
					<Checkbox
						checked={selection.selected.has(row.original.id)}
						onChange={() => selection.toggle(row.original.id)}
						aria-label={`Select ${row.original.title}`}
					/>
				),
				enableSorting: false,
			},
			{
				accessorKey: "title",
				header: "Title",
				cell: ({ row }) => (
					<Link
						to="/incidents/$incidentId"
						params={{ incidentId: row.original.id }}
						className="font-medium hover:text-indigo-600 dark:hover:text-indigo-400"
					>
						{row.original.title}
					</Link>
				),
			},
			{
				id: "center",
				header: "Center",
				cell: ({ row }) => (
					<span className="text-xs font-semibold uppercase tracking-widest text-indigo-500">
						{centerName(row.original.primary_center_id)}
					</span>
				),
			},
			{
				accessorKey: "severity_level",
				header: "Severity",
				cell: ({ row }) => (
					<Badge className={severityClasses(row.original.severity_level)}>
						{severityLabel(row.original.severity_level)}
					</Badge>
				),
			},
			{
				accessorKey: "affected_people",
				header: "Affected",
				cell: ({ row }) => row.original.affected_people.toLocaleString(),
			},
			{
				accessorKey: "casualty_count",
				header: "Casualties",
				cell: ({ row }) => (
					<span className="text-rose-600 dark:text-rose-400 font-semibold">
						{row.original.casualty_count.toLocaleString()}
					</span>
				),
			},
			{
				accessorKey: "status",
				header: "Status",
				cell: ({ row }) => (
					<Badge className={incidentStatusClasses(row.original.status)}>
						{incidentStatusLabel(row.original.status)}
					</Badge>
				),
			},
		];
		columns.push({
			id: "actions",
			header: "",
			enableSorting: false,
			cell: ({ row }) => (
				<DeleteIncidentButton id={row.original.id} title={row.original.title} />
			),
		});
		return columns;
	}, [centerName, selection]);

	const table = useReactTable({
		data: filtered,
		columns,
		state: { pagination: { pageIndex: page - 1, pageSize: 25 } },
		onPaginationChange: (updater) => {
			const next =
				typeof updater === "function"
					? updater({ pageIndex: page - 1, pageSize: 25 })
					: updater;
			setIncidentFilters({ page: next.pageIndex + 1 });
		},
		getCoreRowModel: getCoreRowModel(),
		getFilteredRowModel: getFilteredRowModel(),
		getSortedRowModel: getSortedRowModel(),
		getPaginationRowModel: getPaginationRowModel(),
		manualPagination: false,
	});

	return (
		<div className="space-y-3">
			<TableSearch
				value={search}
				onChange={(v) => {
					setSearch(v);
					setIncidentFilters({ search: v, page: 1 });
				}}
				placeholder="Search by title or center…"
				filters={
					<>
						<Select
							value={String(incidentFilters.severity)}
							onValueChange={(v) =>
								setIncidentFilters({
									severity: v === "ALL" ? "ALL" : Number(v),
									page: 1,
								})
							}
						>
							<SelectTrigger size="sm" className="w-32">
								<SelectValue placeholder="Severity" />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="ALL">All severities</SelectItem>
								{[1, 2, 3, 4, 5].map((n) => (
									<SelectItem key={n} value={String(n)}>
										Severity {n}
									</SelectItem>
								))}
							</SelectContent>
						</Select>
						<Select
							value={incidentFilters.status}
							onValueChange={(v) =>
								setIncidentFilters({
									status: v as IncidentFilters["status"],
									page: 1,
								})
							}
						>
							<SelectTrigger size="sm" className="w-36">
								<SelectValue placeholder="Status" />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="ALL">All statuses</SelectItem>
								{IncidentStatusEnum.options.map((s) => (
									<SelectItem key={s} value={s}>
										{s}
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
									className="text-center text-sm text-muted-foreground py-10"
								>
									<Loader2 className="inline size-4 animate-spin mr-2" />
									Loading incidents…
								</TableCell>
							</TableRow>
						) : table.getRowModel().rows.length === 0 ? (
							<TableRow>
								<TableCell
									colSpan={columns.length}
									className="text-center text-sm text-muted-foreground py-10"
								>
									No incidents match the current filters.
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
				onPageChange={(p) => setIncidentFilters({ page: p })}
			/>
		</div>
	);
}

function DeleteIncidentButton({ id, title }: { id: string; title: string }) {
	const deleteIncident = useAdminStore((s) => s.deleteIncident);
	const toasts = useAdminToasts();

	return (
		<AlertDialog>
			<AlertDialogTrigger asChild>
				<Button
					size="icon-xs"
					variant="ghost"
					className="text-rose-500 hover:text-rose-600"
					title={`Delete ${title}`}
				>
					<Trash2 />
				</Button>
			</AlertDialogTrigger>
			<AlertDialogContent>
				<AlertDialogHeader>
					<AlertDialogTitle>Delete incident?</AlertDialogTitle>
					<AlertDialogDescription>
						This will permanently remove <strong>{title}</strong> from the
						system and release any attached resources.
					</AlertDialogDescription>
				</AlertDialogHeader>
				<AlertDialogFooter>
					<AlertDialogCancel>Cancel</AlertDialogCancel>
					<AlertDialogAction
						onClick={async () => {
							const ok = await deleteIncident(id);
							if (ok) toasts.success("Incident deleted", title);
							else toasts.error("Could not delete incident");
						}}
						className="bg-destructive text-white hover:bg-destructive/90"
					>
						Delete
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}

// Type re-export to keep import sites tidy.
export type { Incident, Center };
