-- Database-scoped singleton owner for the unified projection maintenance runtime.

BEGIN;

CREATE TABLE projection_maintenance_owner (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  owner TEXT,
  lease_token TEXT,
  lease_expires_at INTEGER,
  mode TEXT CHECK(mode IN ('once', 'continuous')),
  started_at INTEGER,
  last_heartbeat_at INTEGER,
  updated_at INTEGER NOT NULL,
  CHECK(
    (owner IS NULL AND lease_token IS NULL AND lease_expires_at IS NULL
      AND mode IS NULL AND started_at IS NULL AND last_heartbeat_at IS NULL)
    OR
    (length(trim(owner)) > 0 AND length(trim(lease_token)) > 0
      AND lease_expires_at IS NOT NULL AND mode IS NOT NULL
      AND started_at IS NOT NULL AND last_heartbeat_at IS NOT NULL)
  )
);

INSERT INTO projection_maintenance_owner(
  singleton, owner, lease_token, lease_expires_at, mode,
  started_at, last_heartbeat_at, updated_at
) VALUES (1, NULL, NULL, NULL, NULL, NULL, NULL, 0);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (
  27,
  '027_projection_maintenance_owner',
  '',
  CAST(strftime('%s','now') AS INTEGER) * 1000
);

COMMIT;
