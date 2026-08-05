import {
	MapContainer,
	TileLayer,
	Marker,
	Popup,
	Circle,
	Polyline,
} from "react-leaflet";
import "leaflet/dist/leaflet.css";
import L from "leaflet";

// Fix default Leaflet icon paths for bundlers
import iconRetinaUrl from "leaflet/dist/images/marker-icon-2x.png?url";
import iconUrl from "leaflet/dist/images/marker-icon.png?url";
import shadowUrl from "leaflet/dist/images/marker-shadow.png?url";

L.Icon.Default.mergeOptions({ iconRetinaUrl, iconUrl, shadowUrl });

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

const resourceIcon = new L.Icon({
	iconUrl,
	iconRetinaUrl,
	shadowUrl,
	iconSize: [20, 33],
	iconAnchor: [10, 33],
	popupAnchor: [1, -28],
	shadowSize: [33, 33],
	className: "hue-rotate-[100deg] saturate-150",
});

interface IncidentMapProps {
	incident: {
		title: string;
		latitude: number;
		longitude: number;
		severity_level: number;
		status: string;
	};
	center: {
		name: string;
		latitude: number;
		longitude: number;
		is_core_center: boolean;
	} | null;
	resources: Array<{
		id: string;
		unit_identifier: string;
		resource_type: string;
		status: string;
		latitude: number;
		longitude: number;
	}>;
	allCenters: Array<{
		id: string;
		name: string;
		latitude: number;
		longitude: number;
		is_core_center: boolean;
	}>;
}

export default function IncidentMap({
	incident,
	center,
	resources,
	allCenters,
}: IncidentMapProps) {
	return (
		<MapContainer
			center={[incident.latitude, incident.longitude]}
			zoom={10}
			scrollWheelZoom={true}
			className="w-full h-full z-0"
		>
			<TileLayer
				attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>'
				url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
			/>

			{/* Incident danger zone */}
			<Circle
				center={[incident.latitude, incident.longitude]}
				pathOptions={{
					color: "#f43f5e",
					fillColor: "#f43f5e",
					fillOpacity: 0.12,
					weight: 2,
					dashArray: "6 4",
				}}
				radius={incident.severity_level * 2000}
			/>

			{/* Incident marker */}
			<Marker
				position={[incident.latitude, incident.longitude]}
				icon={incidentIcon}
			>
				<Popup>
					<div className="font-bold text-rose-600">{incident.title}</div>
					<div className="text-xs font-semibold text-slate-600">
						Severity L{incident.severity_level} · {incident.status}
					</div>
				</Popup>
			</Marker>

			{/* Primary center marker + connection line */}
			{center && (
				<>
					<Marker
						position={[center.latitude, center.longitude]}
						icon={centerIcon}
					>
						<Popup>
							<div className="font-bold text-indigo-600">{center.name} Hub</div>
							<div className="text-xs text-slate-600">
								{center.is_core_center
									? "Core Operations Center"
									: "Divisional Center"}
							</div>
						</Popup>
					</Marker>
					<Polyline
						positions={[
							[incident.latitude, incident.longitude],
							[center.latitude, center.longitude],
						]}
						pathOptions={{
							color: "#6366f1",
							weight: 2,
							dashArray: "8 6",
							opacity: 0.6,
						}}
					/>
				</>
			)}

			{/* Other centers (dimmed) */}
			{allCenters
				.filter((c) => c.id !== center?.id)
				.map((c) => (
					<Marker
						key={c.id}
						position={[c.latitude, c.longitude]}
						icon={centerIcon}
						opacity={0.4}
					>
						<Popup>
							<div className="font-bold text-indigo-400">{c.name}</div>
							<div className="text-xs text-slate-500">
								{c.is_core_center ? "Core Center" : "Divisional"}
							</div>
						</Popup>
					</Marker>
				))}

			{/* Resource markers + connection lines to incident */}
			{resources.map((res) => (
				<>
					<Marker
						key={res.id}
						position={[res.latitude, res.longitude]}
						icon={resourceIcon}
					>
						<Popup>
							<div className="font-bold text-emerald-600">
								{res.unit_identifier}
							</div>
							<div className="text-xs text-slate-600">
								{res.resource_type.replace(/_/g, " ")} ·{" "}
								{res.status.replace(/_/g, " ")}
							</div>
						</Popup>
					</Marker>
					<Polyline
						key={`line-${res.id}`}
						positions={[
							[res.latitude, res.longitude],
							[incident.latitude, incident.longitude],
						]}
						pathOptions={{
							color: res.status === "STUCK" ? "#f59e0b" : "#10b981",
							weight: 1.5,
							dashArray: "4 4",
							opacity: 0.5,
						}}
					/>
				</>
			))}
		</MapContainer>
	);
}
