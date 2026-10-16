use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use draxx_core::*;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};

use crate::{wire::*, *};

const SCHEMA: &str = include_str!("schema.sql");

/// Local-disk SQLite, foreign keys + WAL + FULL synchronous durability.
pub struct SqliteStore {
    connection: Connection,
}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(code, _) = &error
            && matches!(
                code.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
        {
            return Self::Busy;
        }
        Self::Backend(Box::new(error))
    }
}

fn object(value: impl serde::Serialize) -> Result<Value, StorageError> {
    serde_json::to_value(value).map_err(|e| StorageError::Backend(Box::new(e)))
}
fn number(value: Value) -> Result<i64, StorageError> {
    value
        .as_i64()
        .ok_or_else(|| StorageError::Integrity("invalid receipt identifier".into()))
}
fn map_receipt<T>(
    receipt: CommitReceipt<Value>,
    decode: impl FnOnce(Value) -> Result<T, StorageError>,
) -> Result<CommitReceipt<T>, StorageError> {
    let value = decode(receipt.value).map_err(|cause| StorageError::CommitUnknown {
        request_id: receipt.request_id.clone(),
        cause: Box::new(cause),
    })?;
    Ok(CommitReceipt {
        request_id: receipt.request_id,
        revision: receipt.revision,
        replayed: receipt.replayed,
        value,
    })
}
fn next_id(connection: &Connection, table: &str) -> Result<i64, StorageError> {
    let current: i64 = connection.query_row(
        &format!(
            "SELECT coalesce(max({}),0) FROM {table}",
            if table == "commits" { "revision" } else { "id" }
        ),
        [],
        |r| r.get(0),
    )?;
    current
        .checked_add(1)
        .ok_or(DomainError::NumericOverflow.into())
}
fn revision(connection: &Connection) -> Result<SnapshotRevision, StorageError> {
    Ok(SnapshotRevision::new(connection.query_row(
        "SELECT coalesce(max(revision),0) FROM commits",
        [],
        |r| r.get(0),
    )?)?)
}
fn ids(connection: &Connection, sql: &str, id: i64) -> Result<Vec<i64>, StorageError> {
    Ok(connection
        .prepare(sql)?
        .query_map([id], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}
fn session(connection: &Connection, id: i64) -> Result<InspectionSession, StorageError> {
    let wire = connection
        .query_row(
            "SELECT id,status,created_at,closed_at FROM sessions WHERE id=?",
            [id],
            |r| {
                Ok(SessionWire {
                    id: r.get(0)?,
                    status: r.get(1)?,
                    created_at: r.get(2)?,
                    closed_at: r.get(3)?,
                })
            },
        )
        .optional()?
        .ok_or(StorageError::NotFound)?;
    wire.decode()
}
fn active_session(connection: &Connection, id: i64) -> Result<InspectionSession, StorageError> {
    let session = session(connection, id)?;
    if session.status() != SessionStatus::Active {
        return Err(DomainError::InvalidTransition("operation requires an active session").into());
    }
    Ok(session)
}
fn specification(connection: &Connection, id: i64) -> Result<PartSpecification, StorageError> {
    let row = connection.query_row("SELECT part_number,version,nominal_nm,lsl_nm,usl_nm,resolution_nm FROM specifications WHERE id=?", [id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?))).optional()?.ok_or(StorageError::NotFound)?;
    Ok(PartSpecification::new(
        SpecificationId::new(id)?,
        PartNumber::new(row.0)?,
        SpecificationVersion::new(row.1)?,
        Length::from_nanometres(row.2)?,
        Length::from_nanometres(row.3)?,
        Length::from_nanometres(row.4)?,
        Resolution::new(Length::from_nanometres(row.5)?)?,
    )?)
}
fn item(connection: &Connection, id: i64) -> Result<InspectionItem, StorageError> {
    let (session_id, specification_id): (i64, i64) = connection
        .query_row(
            "SELECT session_id,specification_id FROM session_items WHERE id=?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or(StorageError::NotFound)?;
    Ok(InspectionItem::restore(
        InspectionItemId::new(id)?,
        &session(connection, session_id)?,
        &specification(connection, specification_id)?,
    ))
}
fn bundle(connection: &Connection, id: i64) -> Result<Bundle, StorageError> {
    let wire = connection
        .query_row(
            "SELECT id,item_id,sequence,label,status,started_at,closed_at FROM bundles WHERE id=?",
            [id],
            |r| {
                Ok(BundleWire {
                    id: r.get(0)?,
                    item_id: r.get(1)?,
                    sequence: r.get(2)?,
                    label: r.get(3)?,
                    closed: r.get::<_, String>(4)? == "closed",
                    started_at: r.get(5)?,
                    closed_at: r.get(6)?,
                })
            },
        )
        .optional()?
        .ok_or(StorageError::NotFound)?;
    let item = item(connection, wire.item_id)?;
    wire.decode(&session(connection, item.session_id().get())?, &item)
}
fn bundle_response(connection: &Connection, value: Value) -> Result<Bundle, StorageError> {
    let wire: BundleWire = parse(&value.to_string())?;
    let item = item(connection, wire.item_id)?;
    let current = session(connection, item.session_id().get())?;
    // A start receipt stays Open even if the session has subsequently been closed.
    let historical = InspectionSession::new(current.id(), current.created_at());
    wire.decode(&historical, &item)
}
fn capacity(connection: &Connection, id: i64) -> Result<BundleCapacity, StorageError> {
    let count: i64 = connection.query_row(
        "SELECT count(*) FROM effective_measurements WHERE bundle_id=? AND value_nm IS NOT NULL",
        [id],
        |r| r.get(0),
    )?;
    Ok(BundleCapacity::new(
        usize::try_from(count).map_err(|_| DomainError::NumericOverflow)?,
    )?)
}
fn policy(connection: &Connection, id: i64) -> Result<Option<ReconstructionPolicy>, StorageError> {
    let text: Option<String> = connection.query_row(
        "SELECT policy FROM item_policies WHERE item_id=?",
        [id],
        |r| r.get(0),
    )?;
    text.map(|text| parse::<PolicyWire>(&text)?.decode())
        .transpose()
}
fn measurement(connection: &Connection, id: i64) -> Result<Measurement, StorageError> {
    let row=connection.query_row("SELECT bundle_id,sequence,raw_input,value_nm,payload,captured_at,instrument_id,calibration_reference,operator FROM measurements WHERE id=?",[id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?))).optional()?.ok_or(StorageError::NotFound)?;
    let wire: ValueWire = parse(&row.4)?;
    if wire.raw != row.2 || wire.value_nm != row.3 {
        return Err(StorageError::Integrity(
            "measurement payload/columns disagree".into(),
        ));
    }
    let (input, interpreted) = wire.decode()?;
    let bundle = bundle(connection, row.0)?;
    let item = item(connection, bundle.item_id().get())?;
    let session = session(connection, item.session_id().get())?;
    let spec = specification(connection, item.specification_id().get())?;
    let context = CaptureContext {
        session: &session,
        item: &item,
        bundle: &bundle,
        specification: &spec,
    };
    Ok(Measurement::restore(
        &context,
        MeasurementDraft {
            id: MeasurementId::new(id)?,
            sequence: Sequence::new(row.1)?,
            input,
            interpreted,
            captured_at: UnixMillis(row.5),
            provenance: MeasurementProvenance {
                instrument_id: row.6,
                calibration_reference: row.7,
                operator: row.8,
            },
        },
    )?)
}
fn history(connection: &Connection, id: i64) -> Result<MeasurementHistory, StorageError> {
    let original = measurement(connection, id)?;
    let spec = specification(connection, original.specification_id().get())?;
    let mut statement=connection.prepare("SELECT id,previous_id,action,payload,replacement_value_nm,reason,actor,recorded_at FROM corrections WHERE measurement_id=? ORDER BY created_revision")?;
    let rows = statement.query_map([id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, i64>(7)?,
        ))
    })?;
    let mut events: Vec<MeasurementCorrection> = Vec::new();
    for row in rows {
        let row = row?;
        if row.1 != events.last().map(|e| e.id().get()) {
            return Err(DomainError::InvalidCorrectionChain.into());
        }
        let action = match (row.2.as_str(), row.3, row.4) {
            ("invalidate", None, None) => CorrectionAction::Invalidate,
            ("replace", Some(payload), Some(nm)) => {
                let wire: ValueWire = parse(&payload)?;
                if wire.value_nm != nm {
                    return Err(StorageError::Integrity(
                        "correction payload/columns disagree".into(),
                    ));
                }
                let (input, interpreted) = wire.decode()?;
                CorrectionAction::Replace { input, interpreted }
            }
            _ => return Err(StorageError::Integrity("invalid correction record".into())),
        };
        let event = MeasurementCorrection::new(
            CorrectionId::new(row.0)?,
            &original,
            &spec,
            events.last(),
            CorrectionDraft {
                action,
                reason: row.5,
                actor: row.6,
                recorded_at: UnixMillis(row.7),
            },
        )?;
        events.push(event);
    }
    Ok(MeasurementHistory::new(original, events)?)
}
fn correction(connection: &Connection, id: i64) -> Result<MeasurementCorrection, StorageError> {
    let target: i64 = connection
        .query_row(
            "SELECT measurement_id FROM corrections WHERE id=?",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(StorageError::NotFound)?;
    history(connection, target)?
        .corrections()
        .iter()
        .find(|e| e.id().get() == id)
        .cloned()
        .ok_or(StorageError::NotFound)
}
fn snapshot(connection: &Connection, id: i64) -> Result<InspectionSnapshot, StorageError> {
    let item = item(connection, id)?;
    let session = session(connection, item.session_id().get())?;
    let spec = specification(connection, item.specification_id().get())?;
    let mut bundles = Vec::new();
    for bundle_id in ids(
        connection,
        "SELECT id FROM bundles WHERE item_id=? ORDER BY sequence",
        id,
    )? {
        let mut measurements = Vec::new();
        for measurement_id in ids(
            connection,
            "SELECT id FROM measurements WHERE bundle_id=? ORDER BY sequence",
            bundle_id,
        )? {
            measurements.push(history(connection, measurement_id)?);
        }
        bundles.push(SnapshotBundle {
            bundle: bundle(connection, bundle_id)?,
            measurements,
        });
    }
    let snapshot = InspectionSnapshot {
        revision: revision(connection)?,
        session,
        item,
        specification: spec,
        bundles,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(3))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 1 {
            return Err(StorageError::UnsupportedSchema {
                found: version,
                supported: 1,
            });
        }
        if version == 0 {
            let tables:i64=connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0))?;
            if tables != 0 {
                return Err(StorageError::Integrity(
                    "unversioned nonempty database; refusing to initialize over existing data"
                        .into(),
                ));
            }
            let tx = connection.unchecked_transaction()?;
            tx.execute_batch(SCHEMA)?;
            tx.execute("INSERT INTO schema_migrations VALUES (1,?)", [SCHEMA])?;
            tx.pragma_update(None, "user_version", 1)?;
            tx.commit()?;
        }
        let stored: String = connection.query_row(
            "SELECT sql FROM schema_migrations WHERE version=1",
            [],
            |r| r.get(0),
        )?;
        if stored != SCHEMA {
            return Err(StorageError::Integrity(
                "migration SQL differs from the installed immutable migration".into(),
            ));
        }
        let mode: String = connection.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        if mode != "wal" {
            connection.pragma_update(None, "journal_mode", "WAL")?;
        }
        connection.pragma_update(None, "synchronous", "FULL")?;
        let foreign_keys: i64 =
            connection.pragma_query_value(None, "foreign_keys", |r| r.get(0))?;
        let mode: String = connection.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        let synchronous: i64 = connection.pragma_query_value(None, "synchronous", |r| r.get(0))?;
        if foreign_keys != 1 || mode != "wal" || synchronous != 2 {
            return Err(StorageError::Integrity(
                "SQLite refused required durability settings (use a local file database)".into(),
            ));
        }
        let check: String = connection.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if check != "ok" {
            return Err(StorageError::Integrity(check));
        }
        if connection
            .prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_some()
        {
            return Err(StorageError::Integrity("foreign-key check failed".into()));
        }
        Ok(Self { connection })
    }

    fn mutate(
        &mut self,
        request: RequestId,
        operation: &str,
        payload: Value,
        action: impl FnOnce(&Connection, i64) -> Result<(Value, &'static str, String), StorageError>,
    ) -> Result<CommitReceipt<Value>, StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let payload = json(&json!({"format_version":1,"command":payload}))?;
        let previous:Option<(i64,String,String,String)>=tx.query_row("SELECT revision,operation,canonical_payload,response_payload FROM commits WHERE request_id=?",[request.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        if let Some((revision, old_operation, old_payload, response)) = previous {
            if old_operation != operation || old_payload != payload {
                return Err(StorageError::Conflict);
            }
            let decoded = (|| {
                let envelope: Value = parse(&response)?;
                if envelope["format_version"].as_u64() != Some(1) {
                    return Err(StorageError::Integrity(
                        "unsupported receipt format version".into(),
                    ));
                }
                envelope
                    .get("value")
                    .cloned()
                    .ok_or_else(|| StorageError::Integrity("missing stored receipt value".into()))
            })()
            .map_err(|cause| StorageError::CommitUnknown {
                request_id: request.clone(),
                cause: Box::new(cause),
            })?;
            return Ok(CommitReceipt {
                request_id: request,
                revision: SnapshotRevision::new(revision)?,
                value: decoded,
                replayed: true,
            });
        }
        let revision = next_id(&tx, "commits")?;
        let (value, entity_type, entity_id) = action(&tx, revision)?;
        let timestamp = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| StorageError::Backend(Box::new(e)))?
                .as_millis(),
        )
        .map_err(|_| DomainError::NumericOverflow)?;
        tx.execute(
            "INSERT INTO commits VALUES (?,?,?,?,?,?,?,?)",
            params![
                revision,
                request.as_str(),
                operation,
                payload,
                json(&json!({"format_version":1,"value":value}))?,
                entity_type,
                entity_id,
                timestamp
            ],
        )?;
        if let Err(cause) = tx.commit() {
            return Err(StorageError::CommitUnknown {
                request_id: request,
                cause: Box::new(cause),
            });
        }
        Ok(CommitReceipt {
            request_id: request,
            revision: SnapshotRevision::new(revision)?,
            value,
            replayed: false,
        })
    }
}

impl InspectionStore for SqliteStore {
    fn bundle_capacity(&self, id: BundleId) -> Result<BundleCapacity, StorageError> {
        bundle(&self.connection, id.get())?;
        capacity(&self.connection, id.get())
    }
    fn add_part(
        &mut self,
        request: RequestId,
        part: PartNumber,
    ) -> Result<CommitReceipt<PartNumber>, StorageError> {
        let receipt = self.mutate(
            request,
            "add_part",
            json!({"part":part.as_str()}),
            |c, rev| {
                c.execute(
                    "INSERT OR IGNORE INTO part_numbers VALUES (?,?)",
                    params![part.as_str(), rev],
                )?;
                Ok((json!(part.as_str()), "part", part.as_str().into()))
            },
        )?;
        map_receipt(receipt, |value| {
            Ok(PartNumber::new(
                value
                    .as_str()
                    .ok_or_else(|| StorageError::Integrity("invalid part receipt".into()))?
                    .into(),
            )?)
        })
    }
    fn add_specification(
        &mut self,
        request: RequestId,
        spec: CreateSpecification,
    ) -> Result<CommitReceipt<PartSpecification>, StorageError> {
        let payload = json!({"part":spec.part_number.as_str(),"version":spec.version.get(),"nominal":spec.nominal.nanometres(),"lsl":spec.lsl.nanometres(),"usl":spec.usl.nanometres(),"resolution":spec.resolution.length().nanometres()});
        let receipt = self.mutate(request, "add_specification", payload, |c, rev| {
            let id = next_id(c, "specifications")?;
            PartSpecification::new(
                SpecificationId::new(id)?,
                spec.part_number.clone(),
                spec.version,
                spec.nominal,
                spec.lsl,
                spec.usl,
                spec.resolution,
            )?;
            c.execute(
                "INSERT INTO specifications VALUES (?,?,?,?,?,?,?,?)",
                params![
                    id,
                    spec.part_number.as_str(),
                    spec.version.get(),
                    spec.nominal.nanometres(),
                    spec.lsl.nanometres(),
                    spec.usl.nanometres(),
                    spec.resolution.length().nanometres(),
                    rev
                ],
            )?;
            Ok((json!(id), "specification", id.to_string()))
        })?;
        map_receipt(receipt, |v| specification(&self.connection, number(v)?))
    }
    fn create_session(
        &mut self,
        request: RequestId,
        at: UnixMillis,
    ) -> Result<CommitReceipt<InspectionSession>, StorageError> {
        let receipt = self.mutate(request, "create_session", json!({"at":at.0}), |c, rev| {
            let id = next_id(c, "sessions")?;
            let session = InspectionSession::new(SessionId::new(id)?, at);
            c.execute(
                "INSERT INTO sessions VALUES (?,'active',?,NULL,?,?)",
                params![id, at.0, rev, rev],
            )?;
            c.execute("INSERT INTO session_context VALUES (?,NULL)", [id])?;
            Ok((
                object(SessionWire::encode(&session))?,
                "session",
                id.to_string(),
            ))
        })?;
        map_receipt(receipt, |v| parse::<SessionWire>(&v.to_string())?.decode())
    }
    fn add_item(
        &mut self,
        request: RequestId,
        session_id: SessionId,
        spec_id: SpecificationId,
    ) -> Result<CommitReceipt<InspectionItem>, StorageError> {
        let receipt = self.mutate(
            request,
            "add_item",
            json!({"session":session_id.get(),"specification":spec_id.get()}),
            |c, rev| {
                let session = active_session(c, session_id.get())?;
                let spec = specification(c, spec_id.get())?;
                let existing: Option<i64> = c
                    .query_row(
                        "SELECT id FROM session_items WHERE session_id=? AND specification_id=?",
                        params![session_id.get(), spec_id.get()],
                        |r| r.get(0),
                    )
                    .optional()?;
                let id = if let Some(id) = existing {
                    id
                } else {
                    let id = next_id(c, "session_items")?;
                    InspectionItem::new(InspectionItemId::new(id)?, &session, &spec)?;
                    c.execute(
                        "INSERT INTO session_items VALUES (?,?,?,?,?)",
                        params![
                            id,
                            session_id.get(),
                            spec_id.get(),
                            spec.part_number().as_str(),
                            rev
                        ],
                    )?;
                    c.execute(
                        "INSERT INTO item_policies VALUES (?,NULL,?)",
                        params![id, rev],
                    )?;
                    id
                };
                let open: i64 = c.query_row(
                    "SELECT count(*) FROM bundles WHERE session_id=? AND status='open'",
                    [session_id.get()],
                    |r| r.get(0),
                )?;
                if open != 0 {
                    return Err(DomainError::OpenBundlesRemain.into());
                }
                c.execute(
                    "UPDATE session_context SET item_id=? WHERE session_id=?",
                    params![id, session_id.get()],
                )?;
                Ok((json!(id), "item", id.to_string()))
            },
        )?;
        map_receipt(receipt, |v| item(&self.connection, number(v)?))
    }
    fn start_bundle(
        &mut self,
        request: RequestId,
        item_id: InspectionItemId,
        label: String,
        at: UnixMillis,
    ) -> Result<CommitReceipt<Bundle>, StorageError> {
        let receipt = self.mutate(
            request,
            "start_bundle",
            json!({"item":item_id.get(),"label":label,"at":at.0}),
            |c, rev| {
                let item = item(c, item_id.get())?;
                let session = active_session(c, item.session_id().get())?;
                let open: i64 = c.query_row(
                    "SELECT count(*) FROM bundles WHERE session_id=? AND status='open'",
                    [session.id().get()],
                    |r| r.get(0),
                )?;
                if open != 0 {
                    return Err(DomainError::OpenBundlesRemain.into());
                }
                let id = next_id(c, "bundles")?;
                let last: i64 = c.query_row(
                    "SELECT coalesce(max(sequence),0) FROM bundles WHERE item_id=?",
                    [item_id.get()],
                    |r| r.get(0),
                )?;
                let sequence =
                    Sequence::new(last.checked_add(1).ok_or(DomainError::NumericOverflow)?)?;
                let bundle = Bundle::start(
                    BundleId::new(id)?,
                    &session,
                    &item,
                    sequence,
                    label.clone(),
                    at,
                )?;
                c.execute(
                    "INSERT INTO bundles VALUES (?,?,?,?,?,?,'open',?,NULL,?,?)",
                    params![
                        id,
                        item_id.get(),
                        session.id().get(),
                        item.specification_id().get(),
                        sequence.get(),
                        label,
                        at.0,
                        rev,
                        rev
                    ],
                )?;
                c.execute(
                    "UPDATE session_context SET item_id=? WHERE session_id=?",
                    params![item_id.get(), session.id().get()],
                )?;
                Ok((
                    object(BundleWire::encode(&bundle))?,
                    "bundle",
                    id.to_string(),
                ))
            },
        )?;
        map_receipt(receipt, |v| bundle_response(&self.connection, v))
    }
    fn close_bundle(
        &mut self,
        request: RequestId,
        id: BundleId,
        at: UnixMillis,
    ) -> Result<CommitReceipt<Bundle>, StorageError> {
        let receipt = self.mutate(
            request,
            "close_bundle",
            json!({"bundle":id.get(),"at":at.0}),
            |c, rev| {
                let mut bundle = bundle(c, id.get())?;
                let session = session(c, bundle.session_id().get())?;
                bundle.close(&session, at, capacity(c, id.get())?)?;
                c.execute(
                    "UPDATE bundles SET status='closed',closed_at=?,updated_revision=? WHERE id=?",
                    params![at.0, rev, id.get()],
                )?;
                Ok((
                    object(BundleWire::encode(&bundle))?,
                    "bundle",
                    id.to_string(),
                ))
            },
        )?;
        map_receipt(receipt, |v| bundle_response(&self.connection, v))
    }
    fn transition_session(
        &mut self,
        request: RequestId,
        id: SessionId,
        transition: SessionTransition,
        at: UnixMillis,
    ) -> Result<CommitReceipt<InspectionSession>, StorageError> {
        let receipt = self.mutate(
            request,
            "transition_session",
            json!({"session":id.get(),"transition":format!("{transition:?}"),"at":at.0}),
            |c, rev| {
                let mut session = session(c, id.get())?;
                let open: i64 = c.query_row(
                    "SELECT count(*) FROM bundles WHERE session_id=? AND status='open'",
                    [id.get()],
                    |r| r.get(0),
                )?;
                if transition == SessionTransition::Close {
                    let selected: Option<i64> = c.query_row(
                        "SELECT item_id FROM session_context WHERE session_id=?",
                        [id.get()],
                        |r| r.get(0),
                    )?;
                    if selected.is_some() {
                        return Err(DomainError::InvalidTransition(
                            "close the Part Number context before the session",
                        )
                        .into());
                    }
                }
                session.transition(
                    transition,
                    usize::try_from(open).map_err(|_| DomainError::NumericOverflow)?,
                    at,
                )?;
                c.execute(
                    "UPDATE sessions SET status=?,closed_at=?,updated_revision=? WHERE id=?",
                    params![
                        status_name(session.status()),
                        session.closed_at().map(|v| v.0),
                        rev,
                        id.get()
                    ],
                )?;
                Ok((
                    object(SessionWire::encode(&session))?,
                    "session",
                    id.to_string(),
                ))
            },
        )?;
        map_receipt(receipt, |v| parse::<SessionWire>(&v.to_string())?.decode())
    }
    fn record_measurement(
        &mut self,
        command: RecordMeasurement,
    ) -> Result<CommitReceipt<Measurement>, StorageError> {
        let value = ValueWire::encode(&command.input, &command.interpreted);
        let payload = json!({"session":command.session_id.get(),"item":command.item_id.get(),"bundle":command.bundle_id.get(),"value":value,"at":command.captured_at.0,"instrument":command.provenance.instrument_id,"calibration":command.provenance.calibration_reference,"operator":command.provenance.operator});
        let receipt = self.mutate(
            command.request_id.clone(),
            "record_measurement",
            payload,
            |c, rev| {
                let session = session(c, command.session_id.get())?;
                let item = item(c, command.item_id.get())?;
                let bundle = bundle(c, command.bundle_id.get())?;
                let spec = specification(c, item.specification_id().get())?;
                capacity(c, bundle.id().get())?.ensure_can_append()?;
                validate_capture_interpretation(&command.input, &command.interpreted, &spec)?;
                if let InterpretationMethod::IntegerMillimetreSuffix {
                    policy: submitted, ..
                } = &command.interpreted.method
                    && policy(c, item.id().get())?.as_ref() != Some(submitted)
                {
                    return Err(StorageError::Conflict);
                }
                let id = next_id(c, "measurements")?;
                let last: i64 = c.query_row(
                    "SELECT coalesce(max(sequence),0) FROM measurements WHERE bundle_id=?",
                    [bundle.id().get()],
                    |r| r.get(0),
                )?;
                let sequence =
                    Sequence::new(last.checked_add(1).ok_or(DomainError::NumericOverflow)?)?;
                Measurement::new(
                    &CaptureContext {
                        session: &session,
                        item: &item,
                        bundle: &bundle,
                        specification: &spec,
                    },
                    MeasurementDraft {
                        id: MeasurementId::new(id)?,
                        sequence,
                        input: command.input.clone(),
                        interpreted: command.interpreted.clone(),
                        captured_at: command.captured_at,
                        provenance: command.provenance.clone(),
                    },
                )?;
                c.execute(
                    "INSERT INTO measurements VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    params![
                        id,
                        session.id().get(),
                        item.id().get(),
                        bundle.id().get(),
                        spec.id().get(),
                        sequence.get(),
                        command.input.raw(),
                        command.interpreted.value.nanometres(),
                        json(&value)?,
                        command.captured_at.0,
                        command.provenance.instrument_id,
                        command.provenance.calibration_reference,
                        command.provenance.operator,
                        rev
                    ],
                )?;
                Ok((json!(id), "measurement", id.to_string()))
            },
        )?;
        map_receipt(receipt, |v| measurement(&self.connection, number(v)?))
    }
    fn correct_measurement(
        &mut self,
        command: CorrectMeasurement,
    ) -> Result<CommitReceipt<MeasurementCorrection>, StorageError> {
        let wire = match &command.action {
            CorrectionAction::Invalidate => None,
            CorrectionAction::Replace { input, interpreted } => {
                Some(ValueWire::encode(input, interpreted))
            }
        };
        let payload = json!({"measurement":command.measurement_id.get(),"previous":command.expected_previous.map(|id|id.get()),"replacement":wire,"reason":command.reason,"actor":command.actor,"at":command.recorded_at.0});
        let receipt = self.mutate(
            command.request_id.clone(),
            "correct_measurement",
            payload,
            |c, rev| {
                let history = history(c, command.measurement_id.get())?;
                let target = history.original();
                active_session(c, target.session_id().get())?;
                if history.corrections().last().map(MeasurementCorrection::id)
                    != command.expected_previous
                {
                    return Err(StorageError::Conflict);
                }
                let bundle = bundle(c, target.bundle_id().get())?;
                capacity(c, bundle.id().get())?.after_correction(
                    history.effective().is_some(),
                    &command.action,
                    bundle.status(),
                )?;
                let spec = specification(c, target.specification_id().get())?;
                if let CorrectionAction::Replace { input, interpreted } = &command.action {
                    validate_capture_interpretation(input, interpreted, &spec)?;
                    if let InterpretationMethod::IntegerMillimetreSuffix {
                        policy: submitted, ..
                    } = &interpreted.method
                        && policy(c, target.item_id().get())?.as_ref() != Some(submitted)
                    {
                        return Err(StorageError::Conflict);
                    }
                }
                let id = next_id(c, "corrections")?;
                MeasurementCorrection::new(
                    CorrectionId::new(id)?,
                    target,
                    &spec,
                    history.corrections().last(),
                    CorrectionDraft {
                        action: command.action.clone(),
                        reason: command.reason.clone(),
                        actor: command.actor.clone(),
                        recorded_at: command.recorded_at,
                    },
                )?;
                c.execute(
                    "INSERT INTO corrections VALUES (?,?,?,?,?,?,?,?,?,?)",
                    params![
                        id,
                        target.id().get(),
                        command.expected_previous.map(|v| v.get()),
                        if wire.is_some() {
                            "replace"
                        } else {
                            "invalidate"
                        },
                        wire.as_ref().map(json).transpose()?,
                        wire.as_ref().map(|v| v.value_nm),
                        command.reason,
                        command.actor,
                        command.recorded_at.0,
                        rev
                    ],
                )?;
                Ok((json!(id), "correction", id.to_string()))
            },
        )?;
        map_receipt(receipt, |v| correction(&self.connection, number(v)?))
    }
    fn lookup_request(&self, request: &RequestId) -> Result<Option<CommitRecord>, StorageError> {
        let row: Option<(i64, String, String)> = self
            .connection
            .query_row(
                "SELECT revision,entity_type,entity_id FROM commits WHERE request_id=?",
                [request.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        row.map(|(revision, kind, id)| {
            let numeric = || {
                id.parse::<i64>()
                    .map_err(|_| StorageError::Integrity("invalid commit identity".into()))
            };
            let entity = match kind.as_str() {
                "part" => CommittedEntity::Part(PartNumber::new(id.clone())?),
                "specification" => {
                    CommittedEntity::Specification(SpecificationId::new(numeric()?)?)
                }
                "session" => CommittedEntity::Session(SessionId::new(numeric()?)?),
                "item" => CommittedEntity::Item(InspectionItemId::new(numeric()?)?),
                "bundle" => CommittedEntity::Bundle(BundleId::new(numeric()?)?),
                "measurement" => CommittedEntity::Measurement(MeasurementId::new(numeric()?)?),
                "correction" => CommittedEntity::Correction(CorrectionId::new(numeric()?)?),
                _ => return Err(StorageError::Integrity("invalid commit kind".into())),
            };
            Ok(CommitRecord {
                request_id: request.clone(),
                revision: SnapshotRevision::new(revision)?,
                entity,
            })
        })
        .transpose()
    }
    fn specification(&self, id: SpecificationId) -> Result<PartSpecification, StorageError> {
        specification(&self.connection, id.get())
    }
    fn specifications_for_part(
        &self,
        part: &PartNumber,
    ) -> Result<Vec<PartSpecification>, StorageError> {
        let ids: Vec<i64> = self
            .connection
            .prepare("SELECT id FROM specifications WHERE part_number=? ORDER BY version")?
            .query_map([part.as_str()], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        ids.into_iter()
            .map(|id| specification(&self.connection, id))
            .collect()
    }
    fn recover_session(&self, id: SessionId) -> Result<RecoveryState, StorageError> {
        let tx = self.connection.unchecked_transaction()?;
        let session = session(&tx, id.get())?;
        let items = ids(
            &tx,
            "SELECT id FROM session_items WHERE session_id=? ORDER BY id",
            id.get(),
        )?
        .into_iter()
        .map(|id| item(&tx, id))
        .collect::<Result<Vec<_>, _>>()?;
        let open: Option<i64> = tx
            .query_row(
                "SELECT id FROM bundles WHERE session_id=? AND status='open'",
                [id.get()],
                |r| r.get(0),
            )
            .optional()?;
        let selected: Option<i64> = tx.query_row(
            "SELECT item_id FROM session_context WHERE session_id=?",
            [id.get()],
            |r| r.get(0),
        )?;
        let state = RecoveryState {
            session,
            items,
            open_bundle: open.map(|id| bundle(&tx, id)).transpose()?,
            revision: revision(&tx)?,
            active_item: selected.map(InspectionItemId::new).transpose()?,
        };
        if let Some(bundle) = &state.open_bundle
            && state.active_item != Some(bundle.item_id())
        {
            return Err(StorageError::Integrity(
                "open bundle and selected Part Number disagree".into(),
            ));
        }
        tx.commit()?;
        Ok(state)
    }
    fn snapshot(&self, id: InspectionItemId) -> Result<InspectionSnapshot, StorageError> {
        let tx = self.connection.unchecked_transaction()?;
        let snapshot = snapshot(&tx, id.get())?;
        tx.commit()?;
        Ok(snapshot)
    }
    fn sessions(&self) -> Result<Vec<InspectionSession>, StorageError> {
        let ids: Vec<i64> = self
            .connection
            .prepare("SELECT id FROM sessions ORDER BY id")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        ids.into_iter()
            .map(|id| session(&self.connection, id))
            .collect()
    }
    fn select_item(
        &mut self,
        request: RequestId,
        id: SessionId,
        selected: Option<InspectionItemId>,
        at: UnixMillis,
    ) -> Result<CommitReceipt<InspectionSession>, StorageError> {
        let receipt = self.mutate(
            request,
            "select_item",
            json!({"session":id.get(),"item":selected.map(|id|id.get()),"at":at.0}),
            |c, _| {
                let session = active_session(c, id.get())?;
                let open: i64 = c.query_row(
                    "SELECT count(*) FROM bundles WHERE session_id=? AND status='open'",
                    [id.get()],
                    |r| r.get(0),
                )?;
                if open != 0 {
                    return Err(DomainError::OpenBundlesRemain.into());
                }
                if let Some(selected) = selected
                    && item(c, selected.get())?.session_id() != id
                {
                    return Err(DomainError::AssociationMismatch.into());
                }
                c.execute(
                    "UPDATE session_context SET item_id=? WHERE session_id=?",
                    params![selected.map(|id| id.get()), id.get()],
                )?;
                Ok((
                    object(SessionWire::encode(&session))?,
                    "session",
                    id.to_string(),
                ))
            },
        )?;
        map_receipt(receipt, |v| parse::<SessionWire>(&v.to_string())?.decode())
    }
    fn configure_policy(
        &mut self,
        request: RequestId,
        id: InspectionItemId,
        policy: Option<ReconstructionPolicy>,
        at: UnixMillis,
    ) -> Result<CommitReceipt<InspectionItem>, StorageError> {
        let wire = policy.as_ref().map(PolicyWire::encode);
        let receipt = self.mutate(
            request,
            "configure_policy",
            json!({"item":id.get(),"policy":wire,"at":at.0}),
            |c, rev| {
                let item = item(c, id.get())?;
                active_session(c, item.session_id().get())?;
                if let Some(policy) = &policy
                    && (policy.name() != SUFFIX_POLICY_NAME || policy.version().get() != 1)
                {
                    return Err(StorageError::Integrity(
                        "unsupported reconstruction policy/version".into(),
                    ));
                }
                c.execute(
                    "UPDATE item_policies SET policy=?,updated_revision=? WHERE item_id=?",
                    params![wire.as_ref().map(json).transpose()?, rev, id.get()],
                )?;
                Ok((json!(id.get()), "item", id.to_string()))
            },
        )?;
        map_receipt(receipt, |v| item(&self.connection, number(v)?))
    }
    fn policy(&self, id: InspectionItemId) -> Result<Option<ReconstructionPolicy>, StorageError> {
        policy(&self.connection, id.get())
    }
    fn session_snapshots(&self, id: SessionId) -> Result<Vec<InspectionSnapshot>, StorageError> {
        let tx = self.connection.unchecked_transaction()?;
        session(&tx, id.get())?;
        let snapshots = ids(
            &tx,
            "SELECT id FROM session_items WHERE session_id=? ORDER BY id",
            id.get(),
        )?
        .into_iter()
        .map(|id| snapshot(&tx, id))
        .collect::<Result<_, _>>()?;
        tx.commit()?;
        Ok(snapshots)
    }
}

impl SchemaMigrator for SqliteStore {
    fn current_version(&self) -> Result<u32, StorageError> {
        Ok(self
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
    }
    fn migrate(&mut self, migrations: &[Migration]) -> Result<(), StorageError> {
        if migrations.len() != 1 || migrations[0].version != 1 || migrations[0].sql != SCHEMA {
            return Err(StorageError::Integrity(
                "only the shipped immutable migration 1 is supported".into(),
            ));
        }
        Ok(())
    }
}
