-- Align resources.status with data.md §6.4.
--
-- The spec defines four states (EN_ROUTE, STUCK, REJECTED, COMPLETED) and
-- defaults a freshly-created resource to EN_ROUTE. The previous migration
-- 0003 used AVAILABLE as both the default and an extra status value, which
-- is not in the spec. FAILED is mentioned in data.md §5B as a *delta trigger*
-- condition, not a stored status, so it is intentionally absent here.

ALTER TABLE resources DROP CONSTRAINT IF EXISTS resources_status_check;

ALTER TABLE resources
    ADD CONSTRAINT resources_status_check CHECK (
        status IN ('EN_ROUTE', 'STUCK', 'REJECTED', 'COMPLETED')
    );

-- Backfill any rows that still carry the old AVAILABLE value to the spec default.
UPDATE resources SET status = 'EN_ROUTE' WHERE status = 'AVAILABLE';

ALTER TABLE resources ALTER COLUMN status DROP DEFAULT;
ALTER TABLE resources ALTER COLUMN status SET DEFAULT 'EN_ROUTE';
