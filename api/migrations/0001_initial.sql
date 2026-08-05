CREATE TABLE IF NOT EXISTS incidents (
    id uuid PRIMARY KEY,
    region text NOT NULL,
    severity jsonb NOT NULL,
    casualties integer NOT NULL CHECK (casualties >= 0),
    affected_population integer NOT NULL CHECK (affected_population >= 0),
    time_sensitivity smallint NOT NULL CHECK (time_sensitivity BETWEEN 1 AND 10),
    resource_needs jsonb NOT NULL,
    environment jsonb NOT NULL,
    status jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT NOW(),
    updated_at timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_incidents_status ON incidents ((status ->> 'status'));
CREATE INDEX IF NOT EXISTS idx_incidents_created_at ON incidents (created_at);

CREATE TABLE IF NOT EXISTS resources (
    id uuid PRIMARY KEY,
    kind jsonb NOT NULL,
    capacity integer NOT NULL CHECK (capacity > 0),
    latitude double precision NOT NULL CHECK (latitude BETWEEN -90 AND 90),
    longitude double precision NOT NULL CHECK (longitude BETWEEN -180 AND 180),
    status jsonb NOT NULL,
    current_incident_id uuid NULL,
    updated_at timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_resources_status ON resources ((status ->> 'status'));
CREATE INDEX IF NOT EXISTS idx_resources_kind ON resources ((kind ->> 'kind'));