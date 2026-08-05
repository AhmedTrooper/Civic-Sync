import { createFileRoute } from "@tanstack/react-router";
import {
	Activity,
	AlertCircle,
	Briefcase,
	Database,
	Flame,
	Pause,
	Play,
	Plus,
	Truck,
	Users,
	Wrench,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";

interface Resource {
	id: string;
	unit_identifier: string;
	resource_type: string;
	status: string;
	latitude: number;
	longitude: number;
}

interface HelperTeam {
	id: string;
	team_name: string;
	total_members: number;
	assigned_members: number;
	status: string;
}

interface Incident {
	id: string;
	title: string;
	status: string;
	severity_level: number;
	affected_people: number;
	casualty_count: number;
}

interface AssistanceRequest {
	id: string;
	resource_id: string;
	issue_description: string;
	status: string;
}

interface HelperAllocation {
	id: string;
	helper_team_id: string;
	incident_id: string | null;
	assistance_request_id: string | null;
	members_deployed: number;
	status: string;
}

interface SimulationStatus {
	paused: boolean;
	generated: number;
	tick_interval_seconds: number;
}

export const Route = createFileRoute("/admin")({
	component: AdminPanel,
});

function AdminPanel() {
	const [activeTab, setActiveTab] = useState<
		"ASSETS" | "TEAMS" | "ALLOCATIONS" | "SIMULATION"
	>("SIMULATION");

	// Data States
	const [resources, setResources] = useState<Resource[]>([]);
	const [helperTeams, setHelperTeams] = useState<HelperTeam[]>([]);
	const [incidents, setIncidents] = useState<Incident[]>([]);
	const [assistanceRequests, setAssistanceRequests] = useState<
		AssistanceRequest[]
	>([]);
	const [allocations, setAllocations] = useState<HelperAllocation[]>([]);
	const [simStatus, setSimStatus] = useState<SimulationStatus | null>(null);

	const [editingIncident, setEditingIncident] = useState<string | null>(null);
	const [editIncidentForm, setEditIncidentForm] = useState({ severity_level: 5, affected_people: 0, casualty_count: 0, status: "ACTIVE" });

	const [editingResource, setEditingResource] = useState<string | null>(null);
	const [editResourceForm, setEditResourceForm] = useState({ status: "EN_ROUTE" });

	// Form States
	const [resourceForm, setResourceForm] = useState({
		unit_identifier: "",
		resource_type: "AMBULANCE",
		latitude: 23.8103, // Dhaka defaults
		longitude: 90.4125,
	});

	const [incidentForm, setIncidentForm] = useState({
		title: "",
		severity_level: 5,
		affected_people: 1500,
		casualty_count: 50,
		latitude: 24.8949, // Sylhet defaults
		longitude: 91.8687,
	});

	const [teamForm, setTeamForm] = useState({
		team_name: "",
		total_members: 10,
		latitude: 23.8103,
		longitude: 90.4125,
	});

	const [requestForm, setRequestForm] = useState({
		resource_id: "",
		issue_description: "",
	});

	const [allocationForm, setAllocationForm] = useState({
		helper_team_id: "",
		target_type: "INCIDENT", // INCIDENT or ASSISTANCE
		target_id: "",
		members_deployed: 1,
	});

	const fetchData = useCallback(async () => {
		try {
			const [resRes, teamRes, incRes, reqRes, allocRes, simRes] =
				await Promise.all([
					fetch("http://localhost:8080/api/v1/resources"),
					fetch("http://localhost:8080/api/v1/helper-teams"),
					fetch("http://localhost:8080/api/v1/incidents"),
					fetch("http://localhost:8080/api/v1/assistance-requests"),
					fetch("http://localhost:8080/api/v1/helper-allocations"),
					fetch("http://localhost:8080/api/v1/admin/simulation"),
				]);

			if (resRes.ok) {
				const data = await resRes.json();
				setResources(Array.isArray(data) ? data : []);
			}
			if (teamRes.ok) {
				const data = await teamRes.json();
				setHelperTeams(Array.isArray(data) ? data : []);
			}
			if (incRes.ok) {
				const data = await incRes.json();
				setIncidents(Array.isArray(data) ? data : []);
			}
			if (reqRes.ok) {
				const data = await reqRes.json();
				setAssistanceRequests(Array.isArray(data) ? data : []);
			}
			if (allocRes.ok) {
				const data = await allocRes.json();
				setAllocations(Array.isArray(data) ? data : []);
			}
			if (simRes.ok) setSimStatus(await simRes.json());
		} catch (_e) {
			// Ignore network errors on polling
		}
	}, []);

	useEffect(() => {
		fetchData();
		const interval = setInterval(fetchData, 5000);
		return () => clearInterval(interval);
	}, [fetchData]);

	const toggleSimulation = async () => {
		if (!simStatus) return;
		try {
			const endpoint = simStatus.paused
				? "http://localhost:8080/api/v1/admin/simulation/resume"
				: "http://localhost:8080/api/v1/admin/simulation/pause";
			const res = await fetch(endpoint, { method: "POST", headers: { "x-role": "admin" } });
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
					center_id: null,
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
						required_resource_types: [],
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

	const handleSubmitTeam = async (e: React.FormEvent) => {
		e.preventDefault();
		try {
			const res = await fetch("http://localhost:8080/api/v1/helper-teams", {
				method: "POST",
				headers: { "Content-Type": "application/json", "x-role": "admin" },
				body: JSON.stringify({
					team_name: teamForm.team_name,
					total_members: Number(teamForm.total_members),
					latitude: Number(teamForm.latitude),
					longitude: Number(teamForm.longitude),
					center_id: null,
				}),
			});
			if (res.ok) {
				setTeamForm({ ...teamForm, team_name: "" });
				fetchData();
			} else {
				const errText = await res.text();
				alert(`Failed to register team. Error: ${errText}`);
			}
		} catch (err) {
			console.error(err);
		}
	};

	const handleSubmitRequest = async (e: React.FormEvent) => {
		e.preventDefault();
		if (!requestForm.resource_id) {
			alert("Please select a resource.");
			return;
		}
		try {
			const res = await fetch(
				"http://localhost:8080/api/v1/assistance-requests",
				{
					method: "POST",
					headers: { "Content-Type": "application/json", "x-role": "admin" },
					body: JSON.stringify({
						resource_id: requestForm.resource_id,
						issue_description: requestForm.issue_description,
					}),
				},
			);
			if (res.ok) {
				setRequestForm({ resource_id: "", issue_description: "" });
				fetchData();
			} else {
				const errText = await res.text();
				alert(`Failed to log request. Error: ${errText}`);
			}
		} catch (err) {
			console.error(err);
		}
	};

	const handleSubmitAllocation = async (e: React.FormEvent) => {
		e.preventDefault();
		if (!allocationForm.helper_team_id) {
			alert("Please select a team to deploy.");
			return;
		}
		if (!allocationForm.target_id) {
			alert("Please select a target mission.");
			return;
		}
		try {
			const res = await fetch(
				"http://localhost:8080/api/v1/helper-allocations",
				{
					method: "POST",
					headers: { "Content-Type": "application/json", "x-role": "admin" },
					body: JSON.stringify({
						helper_team_id: allocationForm.helper_team_id,
						incident_id:
							allocationForm.target_type === "INCIDENT"
								? allocationForm.target_id
								: null,
						assistance_request_id:
							allocationForm.target_type === "ASSISTANCE"
								? allocationForm.target_id
								: null,
						members_deployed: Number(allocationForm.members_deployed),
					}),
				},
			);
			if (res.ok) {
				setAllocationForm({ ...allocationForm, members_deployed: 1 });
				fetchData();
			} else {
				const errText = await res.text();
				alert(`Failed to deploy team. Error: ${errText}`);
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
			const res = await fetch(`http://localhost:8080/api/v1/resources/${id}/status`, {
				method: "PATCH",
				headers: { "Content-Type": "application/json", "x-role": "admin" },
				body: JSON.stringify({
					status: editResourceForm.status,
				}),
			});
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
						{ id: "TEAMS", label: "Helper Teams", icon: Users },
						{ id: "ALLOCATIONS", label: "Allocations & Support", icon: Wrench },
					].map((tab) => (
						<button
							type="button"
							key={tab.id}
							onClick={() =>
								setActiveTab(
									tab.id as "ASSETS" | "TEAMS" | "ALLOCATIONS" | "SIMULATION",
								)
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
									{incidents.map((inc) => (
										<div
											key={inc.id}
											className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
										>
											{editingIncident === inc.id ? (
												<div className="space-y-3">
													<p className="font-bold">{inc.title}</p>
													<div className="grid grid-cols-2 gap-2 text-xs">
														<label className="block">Severity
															<input type="number" min="1" max="5" value={editIncidentForm.severity_level} onChange={(e) => setEditIncidentForm({ ...editIncidentForm, severity_level: Number(e.target.value) })} className="w-full mt-1 bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded px-2 py-1" />
														</label>
														<label className="block">Affected
															<input type="number" min="0" value={editIncidentForm.affected_people} onChange={(e) => setEditIncidentForm({ ...editIncidentForm, affected_people: Number(e.target.value) })} className="w-full mt-1 bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded px-2 py-1" />
														</label>
														<label className="block">Casualties
															<input type="number" min="0" value={editIncidentForm.casualty_count} onChange={(e) => setEditIncidentForm({ ...editIncidentForm, casualty_count: Number(e.target.value) })} className="w-full mt-1 bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded px-2 py-1" />
														</label>
														<label className="block">Status
															<select value={editIncidentForm.status} onChange={(e) => setEditIncidentForm({ ...editIncidentForm, status: e.target.value })} className="w-full mt-1 bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded px-2 py-1">
																<option value="ACTIVE">ACTIVE</option>
																<option value="RESOLVED">RESOLVED</option>
															</select>
														</label>
													</div>
													<div className="flex gap-2">
														<button onClick={() => saveIncidentUpdate(inc.id)} className="bg-indigo-600 text-white px-3 py-1 rounded text-xs font-bold">Save</button>
														<button onClick={() => setEditingIncident(null)} className="bg-slate-200 dark:bg-slate-800 text-slate-800 dark:text-slate-200 px-3 py-1 rounded text-xs font-bold">Cancel</button>
													</div>
												</div>
											) : (
												<div className="flex justify-between items-start">
													<div>
														<p className="font-bold text-sm">{inc.title}</p>
														<p className="text-xs text-slate-500 mt-1 space-x-2">
															<span className="font-bold text-slate-700 dark:text-slate-300">{inc.status}</span>
															<span>L{inc.severity_level}</span>
															<span>Affected: {inc.affected_people}</span>
															<span className="text-rose-500">Casualties: {inc.casualty_count}</span>
														</p>
													</div>
													<button onClick={() => { setEditingIncident(inc.id); setEditIncidentForm({ severity_level: inc.severity_level, affected_people: inc.affected_people, casualty_count: inc.casualty_count, status: inc.status }); }} className="text-xs text-indigo-600 dark:text-indigo-400 font-semibold hover:underline">Edit</button>
												</div>
											)}
										</div>
									))}
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
									{resources.map((res) => (
										<div
											key={res.id}
											className="flex items-center justify-between bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 p-4 rounded-2xl"
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
														{res.resource_type.replace(/_/g, " ")}
													</p>
												</div>
											</div>
											<div className="text-right">
												{editingResource === res.id ? (
													<div className="flex flex-col items-end gap-2">
														<select value={editResourceForm.status} onChange={(e) => setEditResourceForm({ status: e.target.value })} className="text-xs bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded px-2 py-1">
															<option value="EN_ROUTE">EN ROUTE</option>
															<option value="AT_SCENE">AT SCENE</option>
															<option value="RETURNING">RETURNING</option>
															<option value="AVAILABLE">AVAILABLE</option>
															<option value="OFFLINE">OFFLINE</option>
														</select>
														<div className="flex gap-2">
															<button onClick={() => saveResourceUpdate(res.id)} className="bg-indigo-600 text-white px-2 py-1 rounded text-xs font-bold">Save</button>
															<button onClick={() => setEditingResource(null)} className="bg-slate-200 dark:bg-slate-800 text-slate-800 dark:text-slate-200 px-2 py-1 rounded text-xs font-bold">Cancel</button>
														</div>
													</div>
												) : (
													<>
														<span className="inline-block px-3 py-1 rounded-full text-xs font-bold uppercase tracking-widest bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300">
															{res.status.replace(/_/g, " ")}
														</span>
														<p className="text-xs text-slate-400 mt-2 flex items-center justify-end gap-2">
															{res.latitude.toFixed(4)}, {res.longitude.toFixed(4)}
															<button onClick={() => { setEditingResource(res.id); setEditResourceForm({ status: res.status }); }} className="text-indigo-600 dark:text-indigo-400 hover:underline">Edit</button>
														</p>
													</>
												)}
											</div>
										</div>
									))}
								</div>
							</div>
						</div>
					)}

					{/* TEAMS TAB */}
					{activeTab === "TEAMS" && (
						<div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
							<div className="bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none">
								<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
									<Users className="w-5 h-5 text-emerald-500" />
									Register Helper Team
								</h2>
								<form onSubmit={handleSubmitTeam} className="space-y-4">
									<div>
										<label className="block">
											<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
												Team Name
											</span>
											<input
												type="text"
												required
												value={teamForm.team_name}
												onChange={(e) =>
													setTeamForm({
														...teamForm,
														team_name: e.target.value,
													})
												}
												className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/50"
												placeholder="e.g. Bravo Rescue Squad"
											/>
										</label>
									</div>
									<div>
										<label className="block">
											<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
												Total Members
											</span>
											<input
												type="number"
												min="1"
												required
												value={teamForm.total_members}
												onChange={(e) =>
													setTeamForm({
														...teamForm,
														total_members: Number.parseInt(e.target.value, 10),
													})
												}
												className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/50"
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
													value={teamForm.latitude}
													onChange={(e) =>
														setTeamForm({
															...teamForm,
															latitude: Number.parseFloat(e.target.value),
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/50"
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
													value={teamForm.longitude}
													onChange={(e) =>
														setTeamForm({
															...teamForm,
															longitude: Number.parseFloat(e.target.value),
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/50"
												/>
											</label>
										</div>
									</div>
									<button
										type="submit"
										className="w-full mt-4 bg-emerald-600 hover:bg-emerald-700 text-white font-bold py-3 px-4 rounded-xl transition-colors shadow-lg shadow-emerald-500/30"
									>
										Register Team
									</button>
								</form>
							</div>
							<div className="lg:col-span-2 bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none min-h-[400px]">
								<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
									<Users className="w-5 h-5 text-emerald-500" />
									Active Helper Teams
								</h2>
								<div className="grid gap-3 max-h-[500px] overflow-y-auto pr-2">
									{helperTeams.map((team) => (
										<div
											key={team.id}
											className="flex items-center justify-between bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 p-4 rounded-2xl"
										>
											<div>
												<p className="font-bold text-slate-900 dark:text-slate-100">
													{team.team_name}
												</p>
												<p className="text-xs text-slate-500 mt-1">
													Status: {team.status}
												</p>
											</div>
											<div className="text-right">
												<span className="inline-block px-3 py-1 rounded-full text-xs font-bold uppercase tracking-widest bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300">
													{team.assigned_members} / {team.total_members}{" "}
													DEPLOYED
												</span>
											</div>
										</div>
									))}
								</div>
							</div>
						</div>
					)}

					{/* ALLOCATIONS TAB */}
					{activeTab === "ALLOCATIONS" && (
						<div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
							<div className="space-y-8">
								<div className="bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none">
									<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
										<Wrench className="w-5 h-5 text-amber-500" />
										Log Assistance Request
									</h2>
									<form onSubmit={handleSubmitRequest} className="space-y-4">
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Resource (Vehicle)
												</span>
												<select
													required
													value={requestForm.resource_id}
													onChange={(e) =>
														setRequestForm({
															...requestForm,
															resource_id: e.target.value,
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-amber-500/50 appearance-none"
												>
													<option value="">-- Select Resource --</option>
													{resources.map((res) => (
														<option key={res.id} value={res.id}>
															{res.unit_identifier} (
															{res.resource_type.replace(/_/g, " ")})
														</option>
													))}
												</select>
											</label>
										</div>
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Issue Description
												</span>
												<textarea
													required
													rows={3}
													value={requestForm.issue_description}
													onChange={(e) =>
														setRequestForm({
															...requestForm,
															issue_description: e.target.value,
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-amber-500/50"
													placeholder="e.g. Engine failure on route"
												/>
											</label>
										</div>
										<button
											type="submit"
											className="w-full mt-4 bg-amber-600 hover:bg-amber-700 text-white font-bold py-3 px-4 rounded-xl transition-colors shadow-lg shadow-amber-500/30"
										>
											Log Request
										</button>
									</form>
								</div>

								<div className="bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none">
									<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
										<Briefcase className="w-5 h-5 text-indigo-500" />
										Deploy Helper Team
									</h2>
									<form onSubmit={handleSubmitAllocation} className="space-y-4">
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Team to Deploy
												</span>
												<select
													required
													value={allocationForm.helper_team_id}
													onChange={(e) =>
														setAllocationForm({
															...allocationForm,
															helper_team_id: e.target.value,
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50 appearance-none"
												>
													<option value="">-- Select Team --</option>
													{helperTeams.map((team) => (
														<option key={team.id} value={team.id}>
															{team.team_name} (Avail:{" "}
															{team.total_members - team.assigned_members})
														</option>
													))}
												</select>
											</label>
										</div>
										<div className="grid grid-cols-2 gap-4">
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Target Type
												</span>
												<select
													value={allocationForm.target_type}
													onChange={(e) =>
														setAllocationForm({
															...allocationForm,
															target_type: e.target.value,
															target_id: "", // reset
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50 appearance-none"
												>
													<option value="INCIDENT">Incident</option>
													<option value="ASSISTANCE">Assistance Request</option>
												</select>
											</label>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Members
												</span>
												<input
													type="number"
													min="1"
													required
													value={allocationForm.members_deployed}
													onChange={(e) =>
														setAllocationForm({
															...allocationForm,
															members_deployed: Number.parseInt(
																e.target.value,
																10,
															),
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50"
												/>
											</label>
										</div>
										<div>
											<label className="block">
												<span className="block text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
													Target Mission
												</span>
												<select
													required
													value={allocationForm.target_id}
													onChange={(e) =>
														setAllocationForm({
															...allocationForm,
															target_id: e.target.value,
														})
													}
													className="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-xl px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50 appearance-none"
												>
													<option value="">-- Select Target --</option>
													{allocationForm.target_type === "INCIDENT"
														? incidents.map((inc) => (
																<option key={inc.id} value={inc.id}>
																	{inc.title}
																</option>
															))
														: assistanceRequests.map((req) => (
																<option key={req.id} value={req.id}>
																	{req.issue_description}
																</option>
															))}
												</select>
											</label>
										</div>
										<button
											type="submit"
											className="w-full mt-4 bg-indigo-600 hover:bg-indigo-700 text-white font-bold py-3 px-4 rounded-xl transition-colors shadow-lg shadow-indigo-500/30"
										>
											Allocate Team
										</button>
									</form>
								</div>
							</div>

							<div className="space-y-8">
								<div className="bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none min-h-[250px]">
									<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
										<Wrench className="w-5 h-5 text-amber-500" />
										Active Support Requests
									</h2>
									<div className="grid gap-3">
										{assistanceRequests.map((req) => (
											<div
												key={req.id}
												className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
											>
												<p className="font-bold text-sm">
													{req.issue_description}
												</p>
												<p className="text-xs text-slate-500 mt-1">
													Status: {req.status}
												</p>
											</div>
										))}
										{assistanceRequests.length === 0 && (
											<p className="text-slate-500 text-sm text-center py-10">
												No active requests.
											</p>
										)}
									</div>
								</div>

								<div className="bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-3xl p-6 shadow-sm dark:shadow-none min-h-[250px]">
									<h2 className="text-lg font-bold flex items-center gap-2 mb-6">
										<Briefcase className="w-5 h-5 text-indigo-500" />
										Deployed Allocations
									</h2>
									<div className="grid gap-3">
										{allocations.map((alloc) => (
											<div
												key={alloc.id}
												className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
											>
												<p className="font-bold text-sm">
													Deployed: {alloc.members_deployed} Members
												</p>
												<p className="text-xs text-slate-500 mt-1">
													Status: {alloc.status}
												</p>
											</div>
										))}
										{allocations.length === 0 && (
											<p className="text-slate-500 text-sm text-center py-10">
												No teams currently deployed.
											</p>
										)}
									</div>
								</div>
							</div>
						</div>
					)}
				</div>
			</div>
		</div>
	);
}
