import { createFileRoute, Link } from "@tanstack/react-router";
import {
	Activity,
	AlertCircle,
	Database,
	Flame,
	Pause,
	Play,
	Plus,
	Truck,
	MapPin,
	ShieldAlert,
	Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useAdminStore } from "../store/adminStore";

export const Route = createFileRoute("/admin")({
	component: AdminPanel,
});

function AdminPanel() {
	const [activeTab, setActiveTab] = useState<
		"SIMULATION" | "ASSETS" | "CENTERS"
	>("SIMULATION");

	const {
		centers,
		incidents,
		resources,
		simStatus,
		fetchData,
		deleteIncident,
		deleteResource,
		deleteCenter,
	} = useAdminStore();

	const [editingIncident, setEditingIncident] = useState<string | null>(null);
	const [editIncidentForm, setEditIncidentForm] = useState({
		severity_level: 5,
		affected_people: 0,
		casualty_count: 0,
		status: "ACTIVE",
	});

	const [editingResource, setEditingResource] = useState<string | null>(null);
	const [editResourceForm, setEditResourceForm] = useState({
		status: "EN_ROUTE",
		current_capacity: 1,
		latitude: 0,
		longitude: 0,
	});

	// Form States
	const [resourceForm, setResourceForm] = useState({
		unit_identifier: "",
		resource_type: "AMBULANCE",
		latitude: 23.8103, // Dhaka defaults
		longitude: 90.4125,
		total_capacity: 1,
		owner_center_id: "",
	});

	const [incidentForm, setIncidentForm] = useState({
		title: "",
		severity_level: 5,
		affected_people: 1500,
		casualty_count: 50,
		latitude: 24.8949, // Sylhet defaults
		longitude: 91.8687,
	});

	useEffect(() => {
		fetchData();
		const interval = setInterval(fetchData, 5000);
		return () => clearInterval(interval);
	}, [fetchData]);

	// Initialize the first center in the form if not selected
	useEffect(() => {
		if (centers.length > 0 && !resourceForm.owner_center_id) {
			setResourceForm((prev) => ({ ...prev, owner_center_id: centers[0].id }));
		}
	}, [centers, resourceForm.owner_center_id]);

	const toggleSimulation = async () => {
		if (!simStatus) return;
		try {
			const endpoint = simStatus.paused
				? "http://localhost:8080/api/v1/admin/simulation/resume"
				: "http://localhost:8080/api/v1/admin/simulation/pause";
			const res = await fetch(endpoint, {
				method: "POST",
				headers: { "x-role": "admin" },
			});
			if (res.ok) fetchData();
		} catch (err) {
			console.error(err);
		}
	};

	const handleSubmitResource = async (e: React.FormEvent) => {
		e.preventDefault();
		try {
			const res = await fetch("http://localhost:8080/api/v1/resources", {
				method: "POST",
				headers: { "Content-Type": "application/json", "x-role": "admin" },
				body: JSON.stringify({
					unit_identifier: resourceForm.unit_identifier,
					resource_type: resourceForm.resource_type,
					latitude: Number(resourceForm.latitude),
					longitude: Number(resourceForm.longitude),
					total_capacity: Number(resourceForm.total_capacity),
					owner_center_id: resourceForm.owner_center_id,
				}),
			});
			if (res.ok) {
				setResourceForm({ ...resourceForm, unit_identifier: "" });
				fetchData();
			} else {
				const errText = await res.text();
				alert(`Failed to create resource. Error: ${errText}`);
			}
		} catch (err) {
			console.error(err);
		}
	};

	const handleSubmitIncident = async (e: React.FormEvent) => {
		e.preventDefault();
		try {
			const res = await fetch(
				"http://localhost:8080/api/v1/admin/simulation/inject",
				{
					method: "POST",
					headers: { "Content-Type": "application/json", "x-role": "admin" },
					body: JSON.stringify({
						title: incidentForm.title,
						severity_level: Number(incidentForm.severity_level),
						affected_people: Number(incidentForm.affected_people),
						casualty_count: Number(incidentForm.casualty_count),
						latitude: Number(incidentForm.latitude),
						longitude: Number(incidentForm.longitude),
					}),
				},
			);
			if (res.ok) {
				setIncidentForm({ ...incidentForm, title: "" });
				alert("Crisis successfully injected.");
				fetchData();
			} else {
				const errText = await res.text();
				alert(`Failed to inject incident. Error: ${errText}`);
			}
		} catch (err) {
			console.error(err);
		}
	};

	const saveIncidentUpdate = async (id: string) => {
		try {
			const res = await fetch(`http://localhost:8080/api/v1/incidents/${id}`, {
				method: "PATCH",
				headers: { "Content-Type": "application/json", "x-role": "admin" },
				body: JSON.stringify({
					severity_level: Number(editIncidentForm.severity_level),
					affected_people: Number(editIncidentForm.affected_people),
					casualty_count: Number(editIncidentForm.casualty_count),
					status: editIncidentForm.status,
				}),
			});
			if (res.ok) {
				setEditingIncident(null);
				fetchData();
			} else {
				alert(`Failed to update incident: ${await res.text()}`);
			}
		} catch (err) {
			console.error(err);
		}
	};

	const saveResourceUpdate = async (id: string) => {
		try {
			const res = await fetch(
				`http://localhost:8080/api/v1/resources/${id}/status`,
				{
					method: "PATCH",
					headers: { "Content-Type": "application/json", "x-role": "admin" },
					body: JSON.stringify({
						status: editResourceForm.status,
						current_capacity: Number(editResourceForm.current_capacity),
					}),
				},
			);
			if (res.ok) {
				setEditingResource(null);
				fetchData();
			} else {
				alert(`Failed to update resource: ${await res.text()}`);
			}
		} catch (err) {
			console.error(err);
		}
	};

	return (
		<div className="min-h-screen bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-50 p-4 md:p-8 transition-colors duration-300">
			<div className="max-w-7xl mx-auto space-y-6">
				<header className="flex flex-col md:flex-row md:items-center justify-between pb-6 border-b border-slate-200 dark:border-slate-800 gap-4">
					<div className="flex items-center gap-4">
						<div className="relative flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-indigo-500 to-purple-500 shadow-lg">
							<Database className="w-6 h-6 text-white relative z-10" />
						</div>
						<div>
							<h1 className="text-2xl font-bold tracking-tight text-slate-900 dark:text-white">
								System Administration
							</h1>
							<p className="text-sm text-slate-500 dark:text-slate-400">
								Full entity management & simulation controls.
							</p>
						</div>
					</div>
					{simStatus && (
						<div className="flex items-center gap-4 bg-white/80 dark:bg-slate-900/80 p-3 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm">
							<div className="text-right">
								<p className="text-xs font-bold uppercase tracking-widest text-slate-500">
									Simulation Engine
								</p>
								<p
									className={`text-sm font-bold ${simStatus.paused ? "text-orange-500" : "text-emerald-500 animate-pulse"}`}
								>
									{simStatus.paused ? "PAUSED" : "ACTIVE"}
								</p>
							</div>
							<button
								type="button"
								onClick={toggleSimulation}
								className={`p-3 rounded-xl transition-all ${
									simStatus.paused
										? "bg-emerald-100 text-emerald-600 hover:bg-emerald-200 dark:bg-emerald-500/20 dark:hover:bg-emerald-500/30 dark:text-emerald-400"
										: "bg-orange-100 text-orange-600 hover:bg-orange-200 dark:bg-orange-500/20 dark:hover:bg-orange-500/30 dark:text-orange-400"
								}`}
							>
								{simStatus.paused ? (
									<Play className="w-5 h-5 fill-current" />
								) : (
									<Pause className="w-5 h-5 fill-current" />
								)}
							</button>
						</div>
					)}
				</header>

				{/* TABS */}
				<div className="flex overflow-x-auto gap-2 pb-2 scrollbar-hide">
					{[
						{ id: "SIMULATION", label: "Simulation & Crises", icon: Flame },
						{ id: "ASSETS", label: "Grid Assets", icon: Truck },
						{ id: "CENTERS", label: "Centers", icon: MapPin },
					].map((tab) => (
						<button
							type="button"
							key={tab.id}
							onClick={() =>
								setActiveTab(tab.id as "ASSETS" | "SIMULATION" | "CENTERS")
							}
							className={`flex items-center gap-2 px-5 py-3 rounded-2xl text-sm font-bold uppercase tracking-wider transition-all whitespace-nowrap ${
								activeTab === tab.id
									? "bg-indigo-600 text-white shadow-lg shadow-indigo-500/30"
									: "bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800"
							}`}
						>
							<tab.icon className="w-4 h-4" />
							{tab.label}
						</button>
					))}
				</div>

				<div className="mt-6">
					{/* SIMULATION TAB */}
					{activeTab === "SIMULATION" && (
						<div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
							<div className="bg-white dark:bg-slate-900/50 border border-rose-200 dark:border-rose-900/30 rounded-3xl p-6 shadow-sm dark:shadow-none relative overflow-hidden group">
								<div className="absolute top-0 right-0 w-32 h-32 bg-rose-500/10 rounded-full blur-3xl -mr-10 -mt-10 pointer-events-none group-hover:bg-rose-500/20 transition-colors" />
								<h2 className="text-lg font-bold flex items-center gap-2 mb-6 text-rose-600 dark:text-rose-400">
									<Flame className="w-5 h-5" />
									Inject Custom Crisis
								</h2>
								<form onSubmit={handleSubmitIncident} className="space-y-4">
									<div>
										<label className="block">
											<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
												Crisis Title
											</span>
											<input
												type="text"
												required
												value={incidentForm.title}
												onChange={(e) =>
													setIncidentForm({
														...incidentForm,
														title: e.target.value,
													})
												}
												className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-rose-500/50"
												placeholder="e.g. Flash Flood in Sylhet"
											/>
										</label>
									</div>
									<div className="grid grid-cols-2 gap-4">
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Severity (1-5)
												</span>
												<input
													type="number"
													min="1"
													max="5"
													required
													value={incidentForm.severity_level}
													onChange={(e) =>
														setIncidentForm({
															...incidentForm,
															severity_level: Number.parseInt(
																e.target.value,
																10,
															),
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-rose-500/50"
												/>
											</label>
										</div>
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Casualties
												</span>
												<input
													type="number"
													min="0"
													required
													value={incidentForm.casualty_count}
													onChange={(e) =>
														setIncidentForm({
															...incidentForm,
															casualty_count: Number.parseInt(
																e.target.value,
																10,
															),
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-rose-500/50"
												/>
											</label>
										</div>
									</div>
									<div className="grid grid-cols-2 gap-4">
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Lat
												</span>
												<input
													type="number"
													step="any"
													required
													value={incidentForm.latitude}
													onChange={(e) =>
														setIncidentForm({
															...incidentForm,
															latitude: Number.parseFloat(e.target.value),
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-rose-500/50"
												/>
											</label>
										</div>
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Lng
												</span>
												<input
													type="number"
													step="any"
													required
													value={incidentForm.longitude}
													onChange={(e) =>
														setIncidentForm({
															...incidentForm,
															longitude: Number.parseFloat(e.target.value),
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-rose-500/50"
												/>
											</label>
										</div>
									</div>
									<button
										type="submit"
										className="w-full mt-4 bg-rose-600 hover:bg-rose-700 text-white font-bold py-3 px-4 rounded-xl transition-colors shadow-lg shadow-rose-500/30"
									>
										Deploy Crisis
									</button>
								</form>
							</div>

							<div className="bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none">
								<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
									<AlertCircle className="w-5 h-5 text-indigo-500" />
									Active Incidents
								</h2>
								<div className="grid gap-3 max-h-[400px] overflow-y-auto pr-2">
									{incidents.map((inc) => {
										const centerName =
											centers.find((c) => c.id === inc.primary_center_id)
												?.name || "Unknown";
										return (
											<div
												key={inc.id}
												className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
											>
												{editingIncident === inc.id ? (
													<div className="space-y-3">
														<p className="font-bold">{inc.title}</p>
														<div className="grid grid-cols-2 gap-2 text-xs">
															<label className="block text-slate-700 dark:text-slate-300">
																Severity
																<input
																	type="number"
																	min="1"
																	max="5"
																	value={editIncidentForm.severity_level}
																	onChange={(e) =>
																		setEditIncidentForm({
																			...editIncidentForm,
																			severity_level: Number(e.target.value),
																		})
																	}
																	className="w-full mt-1 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 transition-colors"
																/>
															</label>
															<label className="block text-slate-700 dark:text-slate-300">
																Affected
																<input
																	type="number"
																	min="0"
																	value={editIncidentForm.affected_people}
																	onChange={(e) =>
																		setEditIncidentForm({
																			...editIncidentForm,
																			affected_people: Number(e.target.value),
																		})
																	}
																	className="w-full mt-1 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 transition-colors"
																/>
															</label>
															<label className="block text-slate-700 dark:text-slate-300">
																Casualties
																<input
																	type="number"
																	min="0"
																	value={editIncidentForm.casualty_count}
																	onChange={(e) =>
																		setEditIncidentForm({
																			...editIncidentForm,
																			casualty_count: Number(e.target.value),
																		})
																	}
																	className="w-full mt-1 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-rose-500/50 transition-colors"
																/>
															</label>
															<label className="block text-slate-700 dark:text-slate-300">
																Status
																<select
																	value={editIncidentForm.status}
																	onChange={(e) =>
																		setEditIncidentForm({
																			...editIncidentForm,
																			status: e.target.value,
																		})
																	}
																	className="w-full mt-1 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 transition-colors appearance-none"
																>
																	<option value="ACTIVE">ACTIVE</option>
																	<option value="RESOLVED">RESOLVED</option>
																</select>
															</label>
														</div>
														<div className="flex gap-2">
															<button
																type="button"
																onClick={() => saveIncidentUpdate(inc.id)}
																className="bg-indigo-600 hover:bg-indigo-700 text-white px-4 py-2 rounded-lg text-xs font-bold transition-colors shadow-sm shadow-indigo-500/20"
															>
																Save
															</button>
															<button
																type="button"
																onClick={() => setEditingIncident(null)}
																className="bg-slate-200 hover:bg-slate-300 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-800 dark:text-slate-200 px-4 py-2 rounded-lg text-xs font-bold transition-colors"
															>
																Cancel
															</button>
														</div>
													</div>
												) : (
													<div className="flex justify-between items-start">
														<div>
															<Link
																to="/incidents/$incidentId"
																params={{ incidentId: inc.id }}
																className="font-bold text-sm hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors"
															>
																{inc.title}
															</Link>
															<p className="text-xs font-semibold text-indigo-500 mt-1">
																Center: {centerName}
															</p>
															<p className="text-xs text-slate-500 mt-1 space-x-2">
																<span className="font-bold text-slate-700 dark:text-slate-300">
																	{inc.status}
																</span>
																<span>L{inc.severity_level}</span>
																<span>Affected: {inc.affected_people}</span>
																<span className="text-rose-500">
																	Casualties: {inc.casualty_count}
																</span>
															</p>
														</div>
														<div className="flex items-center gap-2">
															<button
																type="button"
																onClick={() => {
																	setEditingIncident(inc.id);
																	setEditIncidentForm({
																		severity_level: inc.severity_level,
																		affected_people: inc.affected_people,
																		casualty_count: inc.casualty_count,
																		status: inc.status,
																	});
																}}
																className="text-xs text-indigo-600 dark:text-indigo-400 font-semibold hover:underline"
															>
																Edit
															</button>
															<button
																type="button"
																onClick={() => {
																	if (confirm("Delete this incident?")) {
																		deleteIncident(inc.id);
																	}
																}}
																className="p-1 text-rose-500 hover:text-rose-600 transition-colors"
																title="Delete Incident"
															>
																<Trash2 className="w-4 h-4" />
															</button>
														</div>
													</div>
												)}
											</div>
										);
									})}
									{incidents.length === 0 && (
										<p className="text-slate-500 text-sm text-center py-10">
											No active incidents.
										</p>
									)}
								</div>
							</div>
						</div>
					)}

					{/* ASSETS TAB */}
					{activeTab === "ASSETS" && (
						<div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
							<div className="bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none">
								<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
									<Plus className="w-5 h-5 text-indigo-500" />
									Provision Asset
								</h2>
								<form onSubmit={handleSubmitResource} className="space-y-4">
									<div>
										<label className="block">
											<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
												Unit Identifier
											</span>
											<input
												type="text"
												required
												value={resourceForm.unit_identifier}
												onChange={(e) =>
													setResourceForm({
														...resourceForm,
														unit_identifier: e.target.value,
													})
												}
												className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50"
												placeholder="e.g. MED-01"
											/>
										</label>
									</div>
									<div>
										<label className="block">
											<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
												Owner Center
											</span>
											<select
												required
												value={resourceForm.owner_center_id}
												onChange={(e) =>
													setResourceForm({
														...resourceForm,
														owner_center_id: e.target.value,
													})
												}
												className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50 appearance-none"
											>
												{centers.map((c) => (
													<option key={c.id} value={c.id}>
														{c.name}
													</option>
												))}
											</select>
										</label>
									</div>
									<div className="grid grid-cols-2 gap-4">
										<label className="block">
											<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
												Resource Type
											</span>
											<select
												value={resourceForm.resource_type}
												onChange={(e) =>
													setResourceForm({
														...resourceForm,
														resource_type: e.target.value,
													})
												}
												className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50 appearance-none"
											>
												<option value="AMBULANCE">Ambulance</option>
												<option value="BOAT">Rescue Boat</option>
												<option value="HELICOPTER">Helicopter</option>
												<option value="RELIEF_TRUCK">Relief Truck</option>
												<option value="FOOD_PACK">Food Pack</option>
												<option value="WATER_SUPPLY">Water Supply</option>
												<option value="SHELTER_KIT">Shelter Kit</option>
												<option value="MEDICAL_RATION">Medical Ration</option>
											</select>
										</label>
										<label className="block">
											<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
												Total Capacity
											</span>
											<input
												type="number"
												min="1"
												required
												value={resourceForm.total_capacity}
												onChange={(e) =>
													setResourceForm({
														...resourceForm,
														total_capacity: Number.parseInt(e.target.value, 10),
													})
												}
												className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50"
											/>
										</label>
									</div>
									<div className="grid grid-cols-2 gap-4">
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Lat
												</span>
												<input
													type="number"
													step="any"
													required
													value={resourceForm.latitude}
													onChange={(e) =>
														setResourceForm({
															...resourceForm,
															latitude: Number.parseFloat(e.target.value),
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50"
												/>
											</label>
										</div>
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Lng
												</span>
												<input
													type="number"
													step="any"
													required
													value={resourceForm.longitude}
													onChange={(e) =>
														setResourceForm({
															...resourceForm,
															longitude: Number.parseFloat(e.target.value),
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50"
												/>
											</label>
										</div>
									</div>
									<button
										type="submit"
										className="w-full mt-4 bg-indigo-600 hover:bg-indigo-700 text-white font-bold py-3 px-4 rounded-xl transition-colors shadow-lg shadow-indigo-500/30"
									>
										Provision Asset
									</button>
								</form>
							</div>
							<div className="lg:col-span-2 bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none min-h-[400px]">
								<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
									<Truck className="w-5 h-5 text-indigo-500" />
									Active Grid Assets
								</h2>
								<div className="grid gap-3 max-h-[500px] overflow-y-auto pr-2">
									{resources.map((res) => {
										const centerName =
											centers.find((c) => c.id === res.owner_center_id)?.name ||
											"Unknown";
										const assignedIncident = incidents.find(
											(i) => i.id === res.assigned_incident_id,
										);
										return (
											<div
												key={res.id}
												className="flex flex-col sm:flex-row items-start sm:items-center justify-between bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 p-4 rounded-2xl gap-4"
											>
												<div className="flex items-center gap-4">
													<div className="w-10 h-10 rounded-full bg-indigo-100 dark:bg-indigo-500/10 flex items-center justify-center">
														<Activity className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
													</div>
													<div>
														<p className="font-bold text-slate-900 dark:text-slate-100">
															{res.unit_identifier}
														</p>
														<p className="text-xs text-slate-500 uppercase tracking-widest font-semibold mt-0.5">
															{res.resource_type.replace(/_/g, " ")} | Cap:{" "}
															{res.current_capacity}/{res.total_capacity}
														</p>
														<p className="text-xs text-slate-500 mt-1">
															Center: {centerName}
														</p>
														{assignedIncident && (
															<p className="text-xs text-rose-500 font-semibold mt-1">
																Assigned: {assignedIncident.title}
															</p>
														)}
													</div>
												</div>
												<div className="text-left sm:text-right w-full sm:w-auto">
													{editingResource === res.id ? (
														<div className="flex flex-col items-end gap-2 w-full">
															<div className="flex gap-2 w-full justify-end">
																<label className="text-xs flex items-center gap-2 text-slate-700 dark:text-slate-300">
																	Status
																	<select
																		value={editResourceForm.status}
																		onChange={(e) =>
																			setEditResourceForm({
																				...editResourceForm,
																				status: e.target.value,
																			})
																		}
																		className="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 transition-colors appearance-none"
																	>
																		<option value="EN_ROUTE">EN ROUTE</option>
																		<option value="STUCK">STUCK</option>
																		<option value="REJECTED">REJECTED</option>
																		<option value="COMPLETED">COMPLETED</option>
																	</select>
																</label>
																<label className="text-xs flex items-center gap-2 text-slate-700 dark:text-slate-300">
																	Capacity
																	<input
																		type="number"
																		min="0"
																		value={editResourceForm.current_capacity}
																		onChange={(e) =>
																			setEditResourceForm({
																				...editResourceForm,
																				current_capacity: Number(
																					e.target.value,
																				),
																			})
																		}
																		className="w-20 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 transition-colors"
																	/>
																</label>
															</div>
															<div className="flex gap-2">
																<button
																	type="button"
																	onClick={() => saveResourceUpdate(res.id)}
																	className="bg-indigo-600 hover:bg-indigo-700 text-white px-4 py-1.5 rounded-lg text-xs font-bold transition-colors shadow-sm shadow-indigo-500/20"
																>
																	Save
																</button>
																<button
																	type="button"
																	onClick={() => setEditingResource(null)}
																	className="bg-slate-200 hover:bg-slate-300 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-800 dark:text-slate-200 px-4 py-1.5 rounded-lg text-xs font-bold transition-colors"
																>
																	Cancel
																</button>
															</div>
														</div>
													) : (
														<>
															<span className="inline-block px-3 py-1 rounded-full text-xs font-bold uppercase tracking-widest bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300">
																{res.status.replace(/_/g, " ")}
															</span>
															<p className="text-xs text-slate-400 mt-2 flex items-center justify-start sm:justify-end gap-2">
																{res.latitude.toFixed(4)},{" "}
																{res.longitude.toFixed(4)}
																<button
																	type="button"
																	onClick={() => {
																		setEditingResource(res.id);
																		setEditResourceForm({
																			status: res.status,
																			current_capacity: res.current_capacity,
																			latitude: res.latitude,
																			longitude: res.longitude,
																		});
																	}}
																	className="text-indigo-600 dark:text-indigo-400 font-semibold hover:underline"
																>
																	Edit
																</button>
																<button
																	type="button"
																	onClick={() => {
																		if (confirm("Delete this resource?")) {
																			deleteResource(res.id);
																		}
																	}}
																	className="p-1 text-rose-500 hover:text-rose-600 transition-colors ml-1"
																	title="Delete Resource"
																>
																	<Trash2 className="w-3.5 h-3.5" />
																</button>
															</p>
														</>
													)}
												</div>
											</div>
										);
									})}
									{resources.length === 0 && (
										<p className="text-slate-500 text-sm text-center py-10">
											No resources provisioned.
										</p>
									)}
								</div>
							</div>
						</div>
					)}

					{/* CENTERS TAB */}
					{activeTab === "CENTERS" && (
						<div className="grid grid-cols-1 gap-8">
							<div className="bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none min-h-[400px]">
								<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
									<MapPin className="w-5 h-5 text-emerald-500" />
									Divisional Centers
								</h2>
								<div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
									{centers.map((center) => (
										<div
											key={center.id}
											className="p-5 rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 relative overflow-hidden group"
										>
											{center.is_core_center && (
												<div className="absolute top-0 right-0 p-2">
													<ShieldAlert className="w-5 h-5 text-amber-500" />
												</div>
											)}
											<h3 className="font-bold text-lg mb-1">{center.name}</h3>
											<p className="text-xs text-slate-500 uppercase tracking-widest font-semibold mb-3">
												{center.is_core_center
													? "CORE HUB"
													: "DIVISIONAL CENTER"}
											</p>
											<div className="text-sm text-slate-600 dark:text-slate-400 space-y-1 flex items-center justify-between mt-3">
												<p className="text-xs">
													Location: {center.latitude.toFixed(4)},{" "}
													{center.longitude.toFixed(4)}
												</p>
												<button
													type="button"
													onClick={() => {
														if (confirm("Delete this command center?")) {
															deleteCenter(center.id);
														}
													}}
													className="p-1 text-rose-500 hover:text-rose-600 transition-colors"
													title="Delete Center"
												>
													<Trash2 className="w-4 h-4" />
												</button>
											</div>
										</div>
									))}
								</div>
							</div>
						</div>
					)}
				</div>
			</div>
		</div>
	);
}
