-- Drop deprecated tables
DROP TABLE IF EXISTS helper_allocations;
DROP TABLE IF EXISTS assistance_requests;
DROP TABLE IF EXISTS helper_teams;

-- Rename command_centers to centers
ALTER TABLE command_centers RENAME TO centers;

-- Change centers location to lat/lon
ALTER TABLE centers DROP COLUMN location;
ALTER TABLE centers ADD COLUMN latitude FLOAT NOT NULL DEFAULT 0.0;
ALTER TABLE centers ADD COLUMN longitude FLOAT NOT NULL DEFAULT 0.0;

-- Update incidents table
ALTER TABLE incidents DROP COLUMN location;
ALTER TABLE incidents ADD COLUMN latitude FLOAT NOT NULL DEFAULT 0.0;
ALTER TABLE incidents ADD COLUMN longitude FLOAT NOT NULL DEFAULT 0.0;
ALTER TABLE incidents ADD COLUMN primary_center_id UUID REFERENCES centers(id);

-- Update resources table
ALTER TABLE resources RENAME COLUMN center_id TO owner_center_id;
ALTER TABLE resources RENAME COLUMN incident_id TO assigned_incident_id;
ALTER TABLE resources DROP COLUMN current_location;
ALTER TABLE resources ADD COLUMN current_latitude FLOAT NOT NULL DEFAULT 0.0;
ALTER TABLE resources ADD COLUMN current_longitude FLOAT NOT NULL DEFAULT 0.0;
ALTER TABLE resources ADD COLUMN total_capacity INT NOT NULL DEFAULT 1;
ALTER TABLE resources ADD COLUMN current_capacity INT NOT NULL DEFAULT 1;

