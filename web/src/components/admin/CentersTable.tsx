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
import { Loader2, ShieldAlert, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
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
import { type Center, useAdminStore } from "#/store/adminStore.ts";
import { Pagination } from "./Pagination.tsx";
import { TableSearch } from "./TableSearch.tsx";

interface CentersTableProps {
	loading?: boolean;
}

export function CentersTable({ loading }: CentersTableProps) {
	const { centers, centerFilters, setCenterFilters } = useAdminStore();
	const [search, setSearch] = useState(centerFilters.search);
	const debouncedSearch = useDebouncedValue(search, 200);

	const filtered = useMemo(() => {
		const q = debouncedSearch.trim().toLowerCase();
		return centers.filter((c) => {
			if (
				centerFilters.isCore !== "ALL" &&
				c.is_core_center !== centerFilters.isCore
			)
				return false;
			if (q && !c.name.toLowerCase().includes(q)) return false;
			return true;
		});
	}, [centers, centerFilters, debouncedSearch]);

	const total = filtered.length;
	const page = centerFilters.page;

	const selection = useTableSelection<Center>({
		kind: "center",
		rows: filtered,
		idOf: (c) => c.id,
		labelOf: (c) => c.name,
	});

	const columns = useMemo<ColumnDef<Center>[]>(
		() => [
			{
				id: "select",
				header: () => (
					<Checkbox
						checked={selection.isAllSelected}
						onChange={selection.toggleAll}
						aria-label="Select all centers"
					/>
				),
				cell: ({ row }) => (
					<Checkbox
						checked={selection.selected.has(row.original.id)}
						onChange={() => selection.toggle(row.original.id)}
						aria-label={`Select ${row.original.name}`}
					/>
				),
				enableSorting: false,
			},
			{
				accessorKey: "name",
				header: "Name",
				cell: ({ row }) => (
					<Link
						to="/centers/$centerId"
						params={{ centerId: row.original.id }}
						className="font-semibold hover:text-indigo-600 dark:hover:text-indigo-400"
					>
						{row.original.name}
					</Link>
				),
			},
			{
				id: "type",
				header: "Type",
				cell: ({ row }) =>
					row.original.is_core_center ? (
						<Badge className="border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400">
							<ShieldAlert className="size-3" /> Core hub
						</Badge>
					) : (
						<Badge variant="outline">Divisional</Badge>
					),
			},
			{
				accessorKey: "latitude",
				header: "Latitude",
				cell: ({ row }) => row.original.latitude.toFixed(4),
			},
			{
				accessorKey: "longitude",
				header: "Longitude",
				cell: ({ row }) => row.original.longitude.toFixed(4),
			},
			{
				id: "actions",
				header: "",
				enableSorting: false,
				cell: ({ row }) => (
					<DeleteCenterButton id={row.original.id} name={row.original.name} />
				),
			},
		],
		[selection],
	);

	const table = useReactTable({
		data: filtered,
		columns,
		state: { pagination: { pageIndex: page - 1, pageSize: 25 } },
		onPaginationChange: (updater) => {
			const next =
				typeof updater === "function"
					? updater({ pageIndex: page - 1, pageSize: 25 })
					: updater;
			setCenterFilters({ page: next.pageIndex + 1 });
		},
		getCoreRowModel: getCoreRowModel(),
		getFilteredRowModel: getFilteredRowModel(),
		getSortedRowModel: getSortedRowModel(),
		getPaginationRowModel: getPaginationRowModel(),
	});

	return (
		<div className="space-y-3">
			<TableSearch
				value={search}
				onChange={(v) => {
					setSearch(v);
					setCenterFilters({ search: v, page: 1 });
				}}
				placeholder="Search by name…"
				filters={
					<Select
						value={
							centerFilters.isCore === "ALL"
								? "ALL"
								: centerFilters.isCore
									? "core"
									: "div"
						}
						onValueChange={(v) =>
							setCenterFilters({
								isCore: v === "ALL" ? "ALL" : v === "core",
								page: 1,
							})
						}
					>
						<SelectTrigger size="sm" className="w-40">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="ALL">All centers</SelectItem>
							<SelectItem value="core">Core hubs only</SelectItem>
							<SelectItem value="div">Divisional only</SelectItem>
						</SelectContent>
					</Select>
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
									Loading centers…
								</TableCell>
							</TableRow>
						) : table.getRowModel().rows.length === 0 ? (
							<TableRow>
								<TableCell
									colSpan={columns.length}
									className="text-center text-sm text-muted-foreground py-10"
								>
									No centers match the current filters.
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
				onPageChange={(p) => setCenterFilters({ page: p })}
			/>
		</div>
	);
}

function DeleteCenterButton({ id, name }: { id: string; name: string }) {
	const deleteCenter = useAdminStore((s) => s.deleteCenter);
	const toasts = useAdminToasts();

	return (
		<AlertDialog>
			<AlertDialogTrigger asChild>
				<Button
					size="icon-xs"
					variant="ghost"
					className="text-rose-500 hover:text-rose-600"
					title={`Delete ${name}`}
				>
					<Trash2 />
				</Button>
			</AlertDialogTrigger>
			<AlertDialogContent>
				<AlertDialogHeader>
					<AlertDialogTitle>Delete command center?</AlertDialogTitle>
					<AlertDialogDescription>
						The backend refuses to delete seeded hubs (they return 409). Custom
						centers will be removed permanently.
					</AlertDialogDescription>
				</AlertDialogHeader>
				<AlertDialogFooter>
					<AlertDialogCancel>Cancel</AlertDialogCancel>
					<AlertDialogAction
						onClick={async () => {
							const ok = await deleteCenter(id);
							if (ok) toasts.success("Center deleted", name);
							else
								toasts.error(
									"Could not delete center",
									"This center is likely a seeded hub and cannot be removed.",
								);
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
