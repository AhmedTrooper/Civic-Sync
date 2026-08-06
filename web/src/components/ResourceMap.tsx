import {
	MapContainer,
	Marker,
	Polyline,
	Popup,
	TileLayer,
} from "react-leaflet";
import "leaflet/dist/leaflet.css";
import L from "leaflet";
import iconUrl from "leaflet/dist/images/marker-icon.png?url";
import iconRetinaUrl from "leaflet/dist/images/marker-icon-2x.png?url";
import shadowUrl from "leaflet/dist/images/marker-shadow.png?url";

L.Icon.Default.mergeOptions({ iconRetinaUrl, iconUrl, shadowUrl });

const resourceIcon = new L.Icon({
	iconUrl,
	iconRetinaUrl,
	shadowUrl,
	iconSize: [25, 41],
	iconAnchor: [12, 41],
	popupAnchor: [1, -34],
	shadowSize: [41, 41],
	className: "hue-rotate-[100deg] saturate-150",
});

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
	iconSize: [25, 41],
	iconAnchor: [12, 41],
	popupAnchor: [1, -34],
	shadowSize: [41, 41],
	className: "hue-rotate-[320deg] saturate-150",
});

interface ResourceMapProps {
	resource: {
		unit_identifier: string;
		resource_type: string;
		status: string;
		latitude: number;
		longitude: number;
	};
	center: {
		id: string;
		name: string;
		latitude: number;
		longitude: number;
	} | null;
	incident: {
		id: string;
		title: string;
		latitude: number;
		longitude: number;
		severity_level: number;
	} | null;
}

export default function ResourceMap({
	resource,
	center,
	incident,
}: ResourceMapProps) {
	return (
		<MapContainer
			center={[resource.latitude, resource.longitude]}
			zoom={11}
			scrollWheelZoom={true}
			className="w-full h-full z-0"
		>
			<TileLayer
				attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>'
				url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
			/>

			{/* Resource Current Location */}
			<Marker
				position={[resource.latitude, resource.longitude]}
				icon={resourceIcon}
			>
				<Popup>
					<div className="font-bold text-emerald-600">
						{resource.unit_identifier}
					</div>
					<div className="text-xs text-slate-600">
						{resource.resource_type.replace(/_/g, " ")} ·{" "}
						{resource.status.replace(/_/g, " ")}
					</div>
				</Popup>
			</Marker>

			{/* Owner Center */}
			{center && (
				<>
					<Marker
						position={[center.latitude, center.longitude]}
						icon={centerIcon}
					>
						<Popup>
							<div className="font-bold text-indigo-600">
								{center.name} Hub (Base)
							</div>
							<a
								href={`/centers/${center.id}`}
								className="inline-block mt-1.5 text-xs font-bold text-indigo-600 hover:underline"
							>
								View Center &rarr;
							</a>
						</Popup>
					</Marker>
					<Polyline
						positions={[
							[center.latitude, center.longitude],
							[resource.latitude, resource.longitude],
						]}
						pathOptions={{
							color: "#6366f1",
							weight: 2,
							dashArray: "6 6",
							opacity: 0.5,
						}}
					/>
				</>
			)}

			{/* Assigned Incident */}
			{incident && (
				<>
					<Marker
						position={[incident.latitude, incident.longitude]}
						icon={incidentIcon}
					>
						<Popup>
							<div className="font-bold text-rose-600">{incident.title}</div>
							<div className="text-xs text-slate-600">
								Severity Level {incident.severity_level}
							</div>
							<a
								href={`/incidents/${incident.id}`}
								className="inline-block mt-1.5 text-xs font-bold text-rose-600 hover:underline"
							>
								View Incident &rarr;
							</a>
						</Popup>
					</Marker>
					<Polyline
						positions={[
							[resource.latitude, resource.longitude],
							[incident.latitude, incident.longitude],
						]}
						pathOptions={{
							color: resource.status === "STUCK" ? "#f59e0b" : "#10b981",
							weight: 3,
							dashArray: "8 4",
						}}
					/>
				</>
			)}
		</MapContainer>
	);
}
