-- Enable PostGIS geospatial extension (required for GEOGRAPHY(Point, 4326)).
CREATE EXTENSION IF NOT EXISTS postgis;

-- 1. Command Centers — the 8 divisional hubs (Dhaka is the core center).
CREATE TABLE IF NOT EXISTS command_centers (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) UNIQUE NOT NULL,
    is_core_center BOOLEAN DEFAULT FALSE,
    location GEOGRAPHY(Point, 4326) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    server_synced_at TIMESTAMPTZ
);

-- Seed the 8 divisional command hubs (Dhaka core + 7 divisional headquarters).
INSERT INTO command_centers (name, is_core_center, location) VALUES
    ('Dhaka',       TRUE,  ST_SetSRID(ST_MakePoint(90.4125, 23.8103), 4326)::geography),
    ('Chittagong',  FALSE, ST_SetSRID(ST_MakePoint(91.7832, 22.3569), 4326)::geography),
    ('Rajshahi',    FALSE, ST_SetSRID(ST_MakePoint(88.6042, 24.3745), 4326)::geography),
    ('Khulna',      FALSE, ST_SetSRID(ST_MakePoint(89.5403, 22.8456), 4326)::geography),
    ('Barisal',     FALSE, ST_SetSRID(ST_MakePoint(90.3535, 22.7010), 4326)::geography),
    ('Sylhet',      FALSE, ST_SetSRID(ST_MakePoint(91.8687, 24.8949), 4326)::geography),
    ('Rangpur',     FALSE, ST_SetSRID(ST_MakePoint(89.2517, 25.7466), 4326)::geography),
    ('Mymensingh',  FALSE, ST_SetSRID(ST_MakePoint(90.4203, 24.7471), 4326)::geography)
ON CONFLICT (name) DO NOTHING;