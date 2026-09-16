-- Files are an extension lineage. The historical object-model v2 checksum is unchanged.
-- @object table file_model_schema
CREATE TABLE file_model_schema (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  version INTEGER NOT NULL CHECK(version = 1),
  checksum TEXT NOT NULL,
  installed_at INTEGER NOT NULL,
  imported_attachments INTEGER NOT NULL CHECK(imported_attachments >= 0)
);
-- @object table file_blobs
CREATE TABLE file_blobs (
  id TEXT PRIMARY KEY,
  storage_key TEXT NOT NULL UNIQUE CHECK(length(storage_key) BETWEEN 1 AND 1024),
  size_bytes INTEGER NOT NULL CHECK(size_bytes BETWEEN 0 AND 268435456),
  sha256 TEXT CHECK(sha256 IS NULL OR (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*')),
  verification TEXT NOT NULL CHECK(verification IN ('verified','legacy')),
  created_at INTEGER NOT NULL,
  CHECK(verification != 'verified' OR sha256 IS NOT NULL)
);
-- @object table file_objects
CREATE TABLE file_objects (
  object_id TEXT PRIMARY KEY,
  board_id TEXT NOT NULL,
  type_key TEXT NOT NULL DEFAULT 'file' CHECK(type_key = 'file'),
  blob_id TEXT NOT NULL REFERENCES file_blobs(id),
  original_filename TEXT NOT NULL CHECK(length(CAST(original_filename AS BLOB)) BETWEEN 1 AND 255),
  content_type TEXT,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(object_id,board_id,type_key) REFERENCES objects(id,board_id,type_key)
);
-- @object index idx_file_objects_blob
CREATE INDEX idx_file_objects_blob ON file_objects(blob_id,object_id);
-- @object index idx_file_objects_board
CREATE INDEX idx_file_objects_board ON file_objects(board_id,object_id);
-- @object trigger file_blob_identity_immutable
CREATE TRIGGER file_blob_identity_immutable BEFORE UPDATE ON file_blobs BEGIN
  SELECT RAISE(ABORT, 'blob content is immutable; publish a new blob');
END;
-- @object trigger file_capability_immutable
CREATE TRIGGER file_capability_immutable BEFORE UPDATE ON file_objects BEGIN
  SELECT RAISE(ABORT, 'file content identity is immutable; create a new file object');
END;
-- @object trigger file_legacy_attachment_insert_disabled
CREATE TRIGGER file_legacy_attachment_insert_disabled BEFORE INSERT ON task_attachments BEGIN
  SELECT RAISE(ABORT, 'task_attachments is retired; use the canonical file service');
END;
