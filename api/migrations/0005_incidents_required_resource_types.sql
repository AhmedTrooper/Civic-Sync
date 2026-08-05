-- Non-spec extension (user-approved): data.md does not list a
-- required_resource_types column on incidents, but kind-matching in the
-- dispatch engine needs an explicit need vector per incident. The vocab
-- CHECK constraint pins values to the ResourceType vocabulary defined in
-- data.md section 6.4.

ALTER TABLE incidents
    ADD COLUMN required_resource_types VARCHAR(50)[] NOT NULL DEFAULT '{}'::varchar[];

ALTER TABLE incidents
    ADD CONSTRAINT chk_required_resource_types_vocab CHECK (
        required_resource_types <@ ARRAY[
            'AMBULANCE',
            'BOAT',
            'HELICOPTER',
            'RELIEF_TRUCK',
            'FOOD_PACK',
            'WATER_SUPPLY',
            'SHELTER_KIT',
            'MEDICAL_RATION'
        ]::varchar[]
    );

CREATE INDEX idx_incidents_required_resource_types
    ON incidents USING GIN (required_resource_types);
