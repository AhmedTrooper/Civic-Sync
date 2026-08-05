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

interface CenterMapProps {
	center: {
		id: string;
		name: string;
		latitude: number;
		longitude: number;
		is_core_center: boolean;
	};
	resources: Array<{
		id: string;
		unit_identifier: string;
		resource_type: string;
		status: string;
		latitude: number;
		longitude: number;
	}>;
	incidents: Array<{
		id: string;
		title: string;
		severity_level: number;
		status: string;
		latitude: number;
		longitude: number;
	}>;
}

export default function CenterMap({
	center,
	resources,
	incidents,
}: CenterMapProps) {
	return (
		<MapContainer
			center={[center.latitude, center.longitude]}
			zoom={10}
			scrollWheelZoom={true}
			className="w-full h-full z-0"
		>
			<TileLayer
				attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>'
				url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
			/>

			{/* Center coverage area */}
			<Circle
				center={[center.latitude, center.longitude]}
				pathOptions={{
					color: "#6366f1",
					fillColor: "#6366f1",
					fillOpacity: 0.08,
					weight: 2,
					dashArray: "6 4",
				}}
				radius={15000}
			/>

			{/* Center marker */}
			<Marker position={[center.latitude, center.longitude]} icon={centerIcon}>
				<Popup>
					<div className="font-bold text-indigo-600">{center.name}</div>
					<div className="text-xs text-slate-600">
						{center.is_core_center
							? "Core Operations Center"
							: "Divisional Center"}
					</div>
				</Popup>
			</Marker>

			{/* Resource markers + connection lines to center */}
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
						key={`res-line-${res.id}`}
						positions={[
							[res.latitude, res.longitude],
							[center.latitude, center.longitude],
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

			{/* Incident markers + connection lines to center */}
			{incidents.map((inc) => (
				<>
					<Marker
						key={inc.id}
						position={[inc.latitude, inc.longitude]}
						icon={incidentIcon}
					>
						<Popup>
							<div className="font-bold text-rose-600">{inc.title}</div>
							<div className="text-xs font-semibold text-slate-600">
								Severity L{inc.severity_level} · {inc.status}
							</div>
						</Popup>
					</Marker>
					<Circle
						key={`inc-zone-${inc.id}`}
						center={[inc.latitude, inc.longitude]}
						pathOptions={{
							color: "#f43f5e",
							fillColor: "#f43f5e",
							fillOpacity: 0.12,
							weight: 2,
							dashArray: "6 4",
						}}
						radius={inc.severity_level * 2000}
					/>
					<Polyline
						key={`inc-line-${inc.id}`}
						positions={[
							[inc.latitude, inc.longitude],
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
			))}
		</MapContainer>
	);
}
