-- Rebuild incidents and resources to the CivicSync spec schema (data.md §6).
-- The previous generic model (0001) used jsonb columns; the spec needs
-- structured, geospatial fields tied to the command-center hierarchy.

DROP TABLE IF EXISTS resources;
DROP TABLE IF EXISTS incidents;

-- 2. Incidents
CREATE TABLE incidents (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    severity_level INT NOT NULL CHECK (severity_level BETWEEN 1 AND 5),
    affected_people INT NOT NULL DEFAULT 0 CHECK (affected_people >= 0),
    casualty_count INT NOT NULL DEFAULT 0 CHECK (casualty_count >= 0),
    location GEOGRAPHY(Point, 4326) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    server_synced_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_incidents_status ON incidents (status);
CREATE INDEX IF NOT EXISTS idx_incidents_severity ON incidents (severity_level);
CREATE INDEX IF NOT EXISTS idx_incidents_created_at ON incidents (created_at);

-- 4. Vehicles & Physical Resources
CREATE TABLE resources (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    center_id uuid REFERENCES command_centers(id) ON DELETE SET NULL,
    incident_id uuid REFERENCES incidents(id) ON DELETE SET NULL,
    resource_type VARCHAR(50) NOT NULL, -- AMBULANCE, BOAT, HELICOPTER, FOOD_PACK, ...
    unit_identifier VARCHAR(100) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'AVAILABLE', -- AVAILABLE, EN_ROUTE, STUCK, REJECTED, COMPLETED
    distance_passed_km FLOAT NOT NULL DEFAULT 0.0,
    distance_remaining_km FLOAT NOT NULL DEFAULT 0.0,
    current_location GEOGRAPHY(Point, 4326),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    server_synced_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_resources_status ON resources (status);
CREATE INDEX IF NOT EXISTS idx_resources_type ON resources (resource_type);
CREATE INDEX IF NOT EXISTS idx_resources_center ON resources (center_id);