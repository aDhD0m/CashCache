use std::path::Path;
use std::sync::mpsc;
use std::thread;

use rusqlite::Connection;
use tracing::{error, info};

use talon_types::event::Event;

// ---------------------------------------------------------------------------
// StoreCommand — sent from async code to the blocking writer thread
// ---------------------------------------------------------------------------

pub enum StoreCommand {
    Append(Event),
    SavePortfolioSnapshot(String), // JSON-serialized PortfolioSnapshot
    Checkpoint,
    Shutdown,
}

// ---------------------------------------------------------------------------
// EventStore — channel-backed, dedicated OS thread writer (S9.2)
// ---------------------------------------------------------------------------

pub struct EventStore {
    tx: mpsc::Sender<StoreCommand>,
    _writer_thread: thread::JoinHandle<()>,
}

impl EventStore {
    pub fn open(db_path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(db_path)
            .map_err(|e| StoreError::Open(format!("{db_path:?}: {e}")))?;

        // WAL mode, busy timeout, single-writer eliminates contention
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;
             PRAGMA synchronous=NORMAL;
             PRAGMA cache_size=-64000;",
        )
        .map_err(|e| StoreError::Init(e.to_string()))?;

        Self::create_tables(&conn)?;

        let (tx, rx) = mpsc::channel::<StoreCommand>();

        let writer_thread = thread::Builder::new()
            .name("talon-event-writer".into())
            .spawn(move || Self::writer_loop(conn, rx))
            .map_err(|e| StoreError::Init(format!("failed to spawn writer thread: {e}")))?;

        info!("EventStore opened at {db_path:?}");
        Ok(Self {
            tx,
            _writer_thread: writer_thread,
        })
    }

    fn create_tables(conn: &Connection) -> Result<(), StoreError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                ts       TEXT    NOT NULL,
                kind     TEXT    NOT NULL,
                payload  TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_events_ts ON events(ts);

            CREATE TABLE IF NOT EXISTS portfolio_snapshots (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                ts       TEXT    NOT NULL,
                payload  TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_portfolio_ts ON portfolio_snapshots(ts);

            CREATE TABLE IF NOT EXISTS harvest_events (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                ts              TEXT    NOT NULL,
                symbol          TEXT    NOT NULL,
                realized_pnl    TEXT    NOT NULL,
                harvest_amount  TEXT    NOT NULL,
                payload         TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_harvest_ts ON harvest_events(ts);",
        )
        .map_err(|e| StoreError::Init(format!("create tables: {e}")))?;
        Ok(())
    }

    fn writer_loop(conn: Connection, rx: mpsc::Receiver<StoreCommand>) {
        let mut page_count: u64 = 0;

        for cmd in rx {
            match cmd {
                StoreCommand::Append(event) => {
                    let ts = event.timestamp.to_rfc3339();
                    let kind = format!("{:?}", std::mem::discriminant(&event.kind));
                    let payload = match serde_json::to_string(&event) {
                        Ok(p) => p,
                        Err(e) => {
                            error!("failed to serialize event: {e}");
                            continue;
                        }
                    };

                    if let Err(e) = conn.execute(
                        "INSERT INTO events (ts, kind, payload) VALUES (?1, ?2, ?3)",
                        rusqlite::params![ts, kind, payload],
                    ) {
                        error!("failed to insert event: {e}");
                    }

                    page_count += 1;
                    if page_count.is_multiple_of(1000) {
                        Self::run_checkpoint(&conn);
                    }
                }
                StoreCommand::SavePortfolioSnapshot(json) => {
                    let ts = chrono::Utc::now().to_rfc3339();
                    if let Err(e) = conn.execute(
                        "INSERT INTO portfolio_snapshots (ts, payload) VALUES (?1, ?2)",
                        rusqlite::params![ts, json],
                    ) {
                        error!("failed to save portfolio snapshot: {e}");
                    }
                }
                StoreCommand::Checkpoint => {
                    Self::run_checkpoint(&conn);
                }
                StoreCommand::Shutdown => {
                    info!("EventStore writer shutting down");
                    Self::run_checkpoint(&conn);
                    break;
                }
            }
        }
    }

    fn run_checkpoint(conn: &Connection) {
        if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);") {
            error!("WAL checkpoint failed: {e}");
        }
    }

    pub fn append(&self, event: Event) -> Result<(), StoreError> {
        self.tx
            .send(StoreCommand::Append(event))
            .map_err(|_| StoreError::ChannelClosed)
    }

    pub fn save_portfolio_snapshot(&self, json: String) -> Result<(), StoreError> {
        self.tx
            .send(StoreCommand::SavePortfolioSnapshot(json))
            .map_err(|_| StoreError::ChannelClosed)
    }

    pub fn checkpoint(&self) -> Result<(), StoreError> {
        self.tx
            .send(StoreCommand::Checkpoint)
            .map_err(|_| StoreError::ChannelClosed)
    }

    pub fn shutdown(self) -> Result<(), StoreError> {
        self.tx
            .send(StoreCommand::Shutdown)
            .map_err(|_| StoreError::ChannelClosed)?;
        // Wait for writer thread to finish processing all queued commands
        if let Err(e) = self._writer_thread.join() {
            error!("writer thread panicked: {e:?}");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Event replay (for state reconstruction)
// ---------------------------------------------------------------------------

pub fn replay_events(db_path: &Path) -> Result<Vec<Event>, StoreError> {
    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| StoreError::Open(format!("{db_path:?}: {e}")))?;

    let mut stmt = conn
        .prepare("SELECT payload FROM events ORDER BY id ASC")
        .map_err(|e| StoreError::Query(e.to_string()))?;

    let events: Vec<Event> = stmt
        .query_map([], |row| {
            let payload: String = row.get(0)?;
            serde_json::from_str(&payload).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
            })
        })
        .map_err(|e| StoreError::Query(e.to_string()))?
        .filter_map(|r| match r {
            Ok(ev) => Some(ev),
            Err(e) => {
                error!("failed to deserialize event: {e}");
                None
            }
        })
        .collect();

    info!("replayed {} events from {db_path:?}", events.len());
    Ok(events)
}

/// Load the latest portfolio snapshot from the database.
pub fn load_latest_portfolio_snapshot(
    db_path: &Path,
) -> Result<Option<String>, StoreError> {
    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| StoreError::Open(format!("{db_path:?}: {e}")))?;

    let result: Option<String> = conn
        .query_row(
            "SELECT payload FROM portfolio_snapshots ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    Ok(result)
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("failed to open database: {0}")]
    Open(String),
    #[error("initialization error: {0}")]
    Init(String),
    #[error("query error: {0}")]
    Query(String),
    #[error("writer channel closed")]
    ChannelClosed,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use talon_types::event::EventKind;

    fn temp_db() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("talon_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("events.db")
    }

    #[test]
    fn append_and_replay() {
        let db_path = temp_db();
        let store = EventStore::open(&db_path).unwrap();

        let event = Event {
            id: 0,
            timestamp: Utc::now(),
            kind: EventKind::SystemStartup {
                version: "0.1.0".into(),
            },
        };
        store.append(event).unwrap();
        store.shutdown().unwrap();

        let events = replay_events(&db_path).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0].kind {
            EventKind::SystemStartup { version } => assert_eq!(version, "0.1.0"),
            other => panic!("unexpected event: {other:?}"),
        }

        // Cleanup
        std::fs::remove_dir_all(db_path.parent().unwrap()).ok();
    }

    #[test]
    fn multiple_events() {
        let db_path = temp_db();
        let store = EventStore::open(&db_path).unwrap();

        for i in 0..10 {
            store
                .append(Event {
                    id: i,
                    timestamp: Utc::now(),
                    kind: EventKind::SystemStartup {
                        version: format!("0.{i}.0"),
                    },
                })
                .unwrap();
        }
        store.shutdown().unwrap();

        let events = replay_events(&db_path).unwrap();
        assert_eq!(events.len(), 10);

        std::fs::remove_dir_all(db_path.parent().unwrap()).ok();
    }
}
