-- Bind the singleton projection maintenance lease to its compiled store
-- capabilities and the exact runtime artifact that acquired it.
--
-- A v27 lease cannot prove either property. Upgrade therefore invalidates an
-- outstanding v27 lease instead of allowing an unidentified runtime to renew
-- it after the schema boundary.

BEGIN;

ALTER TABLE projection_maintenance_owner
  ADD COLUMN capabilities_json TEXT NOT NULL DEFAULT '[]'
  CHECK(length(trim(capabilities_json)) >= 2);

ALTER TABLE projection_maintenance_owner
  ADD COLUMN build_identity TEXT
  CHECK(build_identity IS NULL OR length(trim(build_identity)) > 0);

UPDATE projection_maintenance_owner
SET owner=NULL,
    lease_token=NULL,
    lease_expires_at=NULL,
    mode=NULL,
    started_at=NULL,
    last_heartbeat_at=NULL,
    capabilities_json='[]',
    build_identity=NULL,
    updated_at=CAST(strftime('%s','now') AS INTEGER) * 1000
WHERE singleton=1;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (
  28,
  '028_projection_maintenance_runtime_identity',
  '',
  CAST(strftime('%s','now') AS INTEGER) * 1000
);

COMMIT;
