import React from "react";
import { MapContainer, TileLayer, Marker, Popup, Circle } from "react-leaflet";
import "leaflet/dist/leaflet.css";
import L from "leaflet";

import iconRetinaUrl from "leaflet/dist/images/marker-icon-2x.png?url";
import iconUrl from "leaflet/dist/images/marker-icon.png?url";
import shadowUrl from "leaflet/dist/images/marker-shadow.png?url";

L.Icon.Default.mergeOptions({ iconRetinaUrl, iconUrl, shadowUrl });

const centerIcon = new L.Icon({
	iconUrl,
	iconRetinaUrl,
	shadowUrl,
	iconSize: [25, 41],
	iconAnchor: [12, 41],
	popupAnchor: [1, -34],
	shadowSize: [41, 41],
	className: "hue-rotate-[200deg] saturate-150",
});

const incidentIcon = new L.Icon({
	iconUrl,
	iconRetinaUrl,
	shadowUrl,
	iconSize: [20, 33],
	iconAnchor: [10, 33],
	popupAnchor: [1, -28],
	shadowSize: [33, 33],
	className: "hue-rotate-[320deg] saturate-150",
});

interface DashboardMapProps {
	centers: Array<{
		id: string;
		name: string;
		latitude: number;
		longitude: number;
		is_core_center: boolean;
	}>;
	incidents: Array<{
		id: string;
		title: string;
		latitude: number;
		longitude: number;
		severity_level: number;
		status: string;
	}>;
}

export default function DashboardMap({
	centers,
	incidents,
}: DashboardMapProps) {
	// Center the map on Bangladesh
	const bangladeshCenter: [number, number] = [23.8, 90.4];

	return (
		<MapContainer
			center={bangladeshCenter}
			zoom={7}
			scrollWheelZoom={true}
			className="w-full h-full z-0"
		>
			<TileLayer
				attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>'
				url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
			/>

			{/* Centers */}
			{centers.map((c) => (
				<Marker
					key={c.id}
					position={[c.latitude, c.longitude]}
					icon={centerIcon}
				>
					<Popup>
						<div className="font-bold text-indigo-600">{c.name}</div>
						<div className="text-xs text-slate-500">
							{c.is_core_center ? "Core Operations Center" : "Divisional Hub"}
						</div>
						<a
							href={`/centers/${c.id}`}
							className="inline-block mt-1.5 text-xs font-bold text-indigo-600 hover:underline"
						>
							View Center Details &rarr;
						</a>
					</Popup>
				</Marker>
			))}

			{/* Incidents */}
			{incidents
				.filter((i) => i.status === "ACTIVE")
				.map((inc) => (
					<React.Fragment key={inc.id}>
						<Circle
							center={[inc.latitude, inc.longitude]}
							pathOptions={{
								color: "#f43f5e",
								fillColor: "#f43f5e",
								fillOpacity: 0.1,
								weight: 1,
							}}
							radius={inc.severity_level * 1500}
						/>
						<Marker
							position={[inc.latitude, inc.longitude]}
							icon={incidentIcon}
						>
							<Popup>
								<div className="font-bold text-rose-600">{inc.title}</div>
								<div className="text-xs text-slate-600">
									L{inc.severity_level} · {inc.status}
								</div>
								<a
									href={`/incidents/${inc.id}`}
									className="inline-block mt-1.5 text-xs font-bold text-rose-600 hover:underline"
								>
									View Incident Details &rarr;
								</a>
							</Popup>
						</Marker>
					</React.Fragment>
				))}
		</MapContainer>
	);
}
