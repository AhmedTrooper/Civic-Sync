-- 3. Human Helper Teams
CREATE TABLE IF NOT EXISTS helper_teams (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    center_id uuid REFERENCES command_centers(id) ON DELETE SET NULL,
    team_name VARCHAR(100) UNIQUE NOT NULL,
    total_members INT NOT NULL CHECK (total_members >= 0),
    assigned_members INT NOT NULL DEFAULT 0 CHECK (assigned_members <= total_members),
    status VARCHAR(50) NOT NULL DEFAULT 'AVAILABLE', -- AVAILABLE, DEPLOYED, OFFLINE
    current_location GEOGRAPHY(Point, 4326),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    server_synced_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_helper_teams_status ON helper_teams (status);
CREATE INDEX IF NOT EXISTS idx_helper_teams_center ON helper_teams (center_id);

-- 5. Resource Assistance Requests
CREATE TABLE IF NOT EXISTS assistance_requests (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    resource_id uuid NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
    issue_description TEXT NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING', -- PENDING, APPROVED, REJECTED, RESOLVED
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    server_synced_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_assistance_requests_status ON assistance_requests (status);
CREATE INDEX IF NOT EXISTS idx_assistance_requests_resource ON assistance_requests (resource_id);

-- 6. Helper Allocations (junction: helper team -> incident OR assistance request)
CREATE TABLE IF NOT EXISTS helper_allocations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    helper_team_id uuid NOT NULL REFERENCES helper_teams(id) ON DELETE CASCADE,
    incident_id uuid REFERENCES incidents(id) ON DELETE CASCADE,
    assistance_request_id uuid REFERENCES assistance_requests(id) ON DELETE CASCADE,
    members_deployed INT NOT NULL CHECK (members_deployed > 0),
    status VARCHAR(50) NOT NULL DEFAULT 'EN_ROUTE', -- EN_ROUTE, ACTIVE, COMPLETED, CANCELLED
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    server_synced_at TIMESTAMPTZ,
    CONSTRAINT check_allocation_target_exists CHECK (
        incident_id IS NOT NULL OR assistance_request_id IS NOT NULL
    )
);

CREATE INDEX IF NOT EXISTS idx_helper_allocations_team ON helper_allocations (helper_team_id);
CREATE INDEX IF NOT EXISTS idx_helper_allocations_incident ON helper_allocations (incident_id);
CREATE INDEX IF NOT EXISTS idx_helper_allocations_status ON helper_allocations (status);