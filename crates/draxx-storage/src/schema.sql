CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, sql TEXT NOT NULL) STRICT;
CREATE TABLE commits(
 revision INTEGER PRIMARY KEY CHECK(revision>0), request_id TEXT NOT NULL UNIQUE,
 operation TEXT NOT NULL, canonical_payload TEXT NOT NULL CHECK(json_valid(canonical_payload)),
 response_payload TEXT NOT NULL CHECK(json_valid(response_payload)), entity_type TEXT NOT NULL,
 entity_id TEXT NOT NULL, recorded_at INTEGER NOT NULL
) STRICT;
CREATE TABLE part_numbers(part_number TEXT PRIMARY KEY NOT NULL CHECK(length(trim(part_number))>0),
 created_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED) STRICT;
CREATE TABLE specifications(
 id INTEGER PRIMARY KEY CHECK(id>0), part_number TEXT NOT NULL REFERENCES part_numbers(part_number),
 version INTEGER NOT NULL CHECK(version>0), nominal_nm INTEGER NOT NULL CHECK(nominal_nm>=0),
 lsl_nm INTEGER NOT NULL CHECK(lsl_nm>=0), usl_nm INTEGER NOT NULL, resolution_nm INTEGER NOT NULL CHECK(resolution_nm>0),
 created_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED,
 CHECK(lsl_nm<usl_nm AND lsl_nm<=nominal_nm AND nominal_nm<=usl_nm),
 UNIQUE(part_number,version), UNIQUE(id,part_number)
) STRICT;
CREATE TABLE sessions(
 id INTEGER PRIMARY KEY CHECK(id>0), status TEXT NOT NULL CHECK(status IN('active','suspended','closed')),
 created_at INTEGER NOT NULL, closed_at INTEGER,
 created_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED,
 updated_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED,
 CHECK((status='closed' AND closed_at IS NOT NULL) OR(status<>'closed' AND closed_at IS NULL))
) STRICT;
CREATE TABLE session_items(
 id INTEGER PRIMARY KEY CHECK(id>0), session_id INTEGER NOT NULL REFERENCES sessions(id),
 specification_id INTEGER NOT NULL, part_number TEXT NOT NULL,
 created_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED,
 FOREIGN KEY(specification_id,part_number) REFERENCES specifications(id,part_number),
 UNIQUE(session_id,specification_id), UNIQUE(id,session_id), UNIQUE(id,session_id,specification_id)
) STRICT;
CREATE TABLE session_context(
 session_id INTEGER PRIMARY KEY REFERENCES sessions(id), item_id INTEGER,
 FOREIGN KEY(item_id,session_id) REFERENCES session_items(id,session_id)
) STRICT;
CREATE TABLE item_policies(
 item_id INTEGER PRIMARY KEY REFERENCES session_items(id), policy TEXT CHECK(policy IS NULL OR json_valid(policy)),
 updated_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE TABLE bundles(
 id INTEGER PRIMARY KEY CHECK(id>0), item_id INTEGER NOT NULL, session_id INTEGER NOT NULL, specification_id INTEGER NOT NULL,
 sequence INTEGER NOT NULL CHECK(sequence>0), label TEXT NOT NULL CHECK(length(trim(label))>0),
 status TEXT NOT NULL CHECK(status IN('open','closed')), started_at INTEGER NOT NULL, closed_at INTEGER,
 created_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED,
 updated_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED,
 FOREIGN KEY(item_id,session_id,specification_id) REFERENCES session_items(id,session_id,specification_id),
 CHECK((status='closed' AND closed_at IS NOT NULL) OR(status='open' AND closed_at IS NULL)),
 UNIQUE(item_id,sequence), UNIQUE(id,item_id,session_id,specification_id)
) STRICT;
CREATE UNIQUE INDEX one_open_bundle ON bundles(session_id) WHERE status='open';
CREATE TABLE measurements(
 id INTEGER PRIMARY KEY CHECK(id>0), session_id INTEGER NOT NULL, item_id INTEGER NOT NULL,
 bundle_id INTEGER NOT NULL, specification_id INTEGER NOT NULL, sequence INTEGER NOT NULL CHECK(sequence>0),
 raw_input TEXT NOT NULL, value_nm INTEGER NOT NULL CHECK(value_nm>=0), payload TEXT NOT NULL CHECK(json_valid(payload)),
 captured_at INTEGER NOT NULL, instrument_id TEXT, calibration_reference TEXT, operator TEXT,
 created_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED,
 FOREIGN KEY(bundle_id,item_id,session_id,specification_id) REFERENCES bundles(id,item_id,session_id,specification_id),
 UNIQUE(bundle_id,sequence)
) STRICT;
CREATE TABLE corrections(
 id INTEGER PRIMARY KEY CHECK(id>0), measurement_id INTEGER NOT NULL REFERENCES measurements(id), previous_id INTEGER,
 action TEXT NOT NULL CHECK(action IN('replace','invalidate')), payload TEXT CHECK(payload IS NULL OR json_valid(payload)),
 replacement_value_nm INTEGER CHECK(replacement_value_nm>=0), reason TEXT NOT NULL CHECK(length(trim(reason))>0), actor TEXT,
 recorded_at INTEGER NOT NULL,
 created_revision INTEGER NOT NULL REFERENCES commits(revision) DEFERRABLE INITIALLY DEFERRED,
 FOREIGN KEY(previous_id,measurement_id) REFERENCES corrections(id,measurement_id),
 CHECK(previous_id IS NULL OR previous_id<>id),
 CHECK((action='replace' AND payload IS NOT NULL AND replacement_value_nm IS NOT NULL)
    OR(action='invalidate' AND payload IS NULL AND replacement_value_nm IS NULL)), UNIQUE(id,measurement_id)
) STRICT;
CREATE UNIQUE INDEX correction_root ON corrections(measurement_id) WHERE previous_id IS NULL;
CREATE UNIQUE INDEX correction_successor ON corrections(previous_id) WHERE previous_id IS NOT NULL;
CREATE INDEX correction_history ON corrections(measurement_id,created_revision);
CREATE INDEX measurements_item ON measurements(item_id,created_revision);
CREATE INDEX sessions_status ON sessions(status,id);
CREATE VIEW effective_measurements AS
 SELECT m.id,m.bundle_id,m.item_id,m.session_id,m.sequence,
 CASE WHEN c.id IS NULL THEN m.value_nm WHEN c.action='replace' THEN c.replacement_value_nm ELSE NULL END AS value_nm
 FROM measurements m LEFT JOIN corrections c ON c.measurement_id=m.id
 AND NOT EXISTS(SELECT 1 FROM corrections successor WHERE successor.previous_id=c.id);
CREATE TRIGGER capacity_insert BEFORE INSERT ON measurements BEGIN
 SELECT CASE WHEN (SELECT count(*) FROM effective_measurements WHERE bundle_id=NEW.bundle_id AND value_nm IS NOT NULL)>=42
 THEN RAISE(ABORT,'bundle capacity exceeded') END;
END;
CREATE TRIGGER capacity_restore BEFORE INSERT ON corrections WHEN NEW.action='replace' BEGIN
 SELECT CASE WHEN (SELECT value_nm FROM effective_measurements WHERE id=NEW.measurement_id) IS NULL
 AND (SELECT count(*) FROM effective_measurements WHERE bundle_id=(SELECT bundle_id FROM measurements WHERE id=NEW.measurement_id) AND value_nm IS NOT NULL)>=42
 THEN RAISE(ABORT,'bundle capacity exceeded') END;
END;
CREATE TRIGGER nonempty_invalidation BEFORE INSERT ON corrections WHEN NEW.action='invalidate' BEGIN
 SELECT CASE WHEN (SELECT status FROM bundles WHERE id=(SELECT bundle_id FROM measurements WHERE id=NEW.measurement_id))='closed'
 AND (SELECT value_nm FROM effective_measurements WHERE id=NEW.measurement_id) IS NOT NULL
 AND (SELECT count(*) FROM effective_measurements WHERE bundle_id=(SELECT bundle_id FROM measurements WHERE id=NEW.measurement_id) AND value_nm IS NOT NULL)<=1
 THEN RAISE(ABORT,'closed bundle cannot become empty') END;
END;
CREATE TRIGGER nonempty_close BEFORE UPDATE OF status ON bundles WHEN NEW.status='closed' BEGIN
 SELECT CASE WHEN (SELECT count(*) FROM effective_measurements WHERE bundle_id=NEW.id AND value_nm IS NOT NULL) NOT BETWEEN 1 AND 42
 THEN RAISE(ABORT,'closed bundle requires 1..42 active measurements') END;
END;
CREATE TRIGGER immutable_commits_UPDATE BEFORE UPDATE ON commits BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_commits_DELETE BEFORE DELETE ON commits BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_part_numbers_UPDATE BEFORE UPDATE ON part_numbers BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_part_numbers_DELETE BEFORE DELETE ON part_numbers BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_specifications_UPDATE BEFORE UPDATE ON specifications BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_specifications_DELETE BEFORE DELETE ON specifications BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_session_items_UPDATE BEFORE UPDATE ON session_items BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_session_items_DELETE BEFORE DELETE ON session_items BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_measurements_UPDATE BEFORE UPDATE ON measurements BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_measurements_DELETE BEFORE DELETE ON measurements BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_corrections_UPDATE BEFORE UPDATE ON corrections BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_corrections_DELETE BEFORE DELETE ON corrections BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_schema_migrations_UPDATE BEFORE UPDATE ON schema_migrations BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER immutable_schema_migrations_DELETE BEFORE DELETE ON schema_migrations BEGIN SELECT RAISE(ABORT,'immutable inspection history'); END;
CREATE TRIGGER no_delete_sessions BEFORE DELETE ON sessions BEGIN SELECT RAISE(ABORT,'inspection history cannot be deleted'); END;
CREATE TRIGGER no_delete_bundles BEFORE DELETE ON bundles BEGIN SELECT RAISE(ABORT,'inspection history cannot be deleted'); END;
CREATE TRIGGER immutable_bundle_identity BEFORE UPDATE OF id,item_id,session_id,specification_id,sequence,label,started_at,created_revision ON bundles BEGIN SELECT RAISE(ABORT,'immutable bundle identity'); END;
CREATE TRIGGER immutable_session_identity BEFORE UPDATE OF id,created_at,created_revision ON sessions BEGIN SELECT RAISE(ABORT,'immutable session identity'); END;
