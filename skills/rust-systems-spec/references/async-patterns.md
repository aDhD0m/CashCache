# Async Boundary Patterns

## The Problem

Rust systems that interact with external services (brokers, databases, APIs, hardware) inevitably have two kinds of I/O:

1. **Request/response** -- Submit order, get acknowledgment. Query position, get snapshot. These block on a network round-trip. They are semantically synchronous even if the transport is TCP.
2. **Long-lived streams** -- Subscribe to price feed, receive events indefinitely. These are inherently async -- they need cancellation, backpressure, and reconnection.

Mixing both in a single trait creates a design contradiction. The sync methods want `fn foo() -> Result<T, E>`. The async methods want `async fn bar() -> Result<StreamHandle, E>`. Annotating the whole trait with `#[async_trait]` makes the sync methods look async (confusing), and making everything sync forces stream implementations to hide internal task spawning (losing cancellation control).

## The Split-Trait Pattern

Split into two traits with distinct responsibilities:

```rust
/// Synchronous command interface.
/// Implementations block on network I/O -- this is intentional.
/// All calls from async code go through tokio::task::spawn_blocking.
pub trait ServiceCommands: Send + Sync {
    fn request(&self, req: &Request) -> Result<Response, ServiceError>;
    fn query(&self, q: &Query) -> Result<Snapshot, ServiceError>;
    fn service_id(&self) -> ServiceId;
}

/// Asynchronous streaming interface.
/// Long-lived connections that push events.
/// Drop StreamHandle = cancel the stream.
#[async_trait]
pub trait ServiceStreams: Send + Sync {
    async fn subscribe(
        &self,
        filter: &Filter,
        tx: Sender<Event>,
    ) -> Result<StreamHandle, ServiceError>;
}

/// RAII cancellation handle. Drop = cancel.
pub struct StreamHandle {
    _cancel: tokio::sync::oneshot::Sender<()>,
    join: tokio::task::JoinHandle<()>,
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        // oneshot sender is dropped, receiver in the stream task sees Err,
        // task exits, JoinHandle becomes ready.
    }
}
```

## The Blocking Thread Pool Rule

Strategy modules and async business logic never call `ServiceCommands` directly. They go through a session manager that wraps every call in `spawn_blocking`:

```rust
impl SessionManager {
    pub async fn submit(&self, order: &OrderIntent) -> Result<OrderAck, BrokerError> {
        let commands = self.commands.clone(); // Arc<dyn BrokerCommands>
        let order = order.clone();
        tokio::task::spawn_blocking(move || {
            commands.submit_order(&order)
        }).await.map_err(|e| BrokerError::RuntimePanic(e.to_string()))?
    }
}
```

This prevents sync network I/O from stalling the tokio executor. The `spawn_blocking` pool has its own thread limit (default 512 in tokio, configurable).

## The Channel-Backed Writer Pattern

For persistence (SQLite, file I/O) that must not block the async executor:

```rust
pub struct EventStore {
    tx: std::sync::mpsc::Sender<StoreCommand>,
    _writer_thread: std::thread::JoinHandle<()>,
}

enum StoreCommand {
    Append(Event),
    Checkpoint,
    Shutdown,
}

impl EventStore {
    pub fn new(db_path: &Path) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let path = db_path.to_owned();
        let handle = std::thread::spawn(move || {
            let conn = Connection::open(&path).expect("DB open");
            conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
                .expect("PRAGMA");
            for cmd in rx {
                match cmd {
                    StoreCommand::Append(event) => { /* INSERT */ },
                    StoreCommand::Checkpoint => {
                        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").ok();
                    },
                    StoreCommand::Shutdown => break,
                }
            }
        });
        EventStore { tx, _writer_thread: handle }
    }

    pub fn append(&self, event: Event) {
        self.tx.send(StoreCommand::Append(event)).ok();
    }
}
```

Async code calls `store.append(event)` -- this never blocks (channel send is near-instant unless the channel is full, which indicates a deeper problem). The writer thread handles all SQLite I/O in its own blocking context.

## The IntelligencePort Pattern

For consuming data from external systems that may or may not be connected:

```rust
pub trait IntelligencePort<S: Send + Sync>: Send + Sync {
    fn latest(&self) -> Option<&S>;
}
```

This is a read-only, non-blocking, synchronous call. The external system publishes state on its own schedule. The consumer reads whatever's there. `None` = "not connected or not yet initialized."

Implementation: typically an `Arc<RwLock<Option<S>>>` behind the trait. The publisher grabs a write lock and replaces the state. Consumers grab a read lock. Contention is minimal because publishes are infrequent relative to reads.

## Reconciliation State Machine

For systems that maintain local state mirroring an external source of truth:

```rust
pub enum ReconciliationState {
    PullExternalSnapshot,
    PullHistory { since: DateTime<Utc> },
    ReplayMissing { items: Vec<ExternalEvent> },
    Diff { external: Snapshot, local: Snapshot },
    OperatorReview { discrepancies: Vec<Discrepancy> },
    Resolved,
}
```

Rules:
- Always pull history with a time overlap buffer (e.g., 30 minutes before last known event) to handle clock skew and out-of-order delivery.
- Deduplicate by external event ID -- replaying the same event twice must be a no-op.
- External source wins on quantity/state mismatches (adjust local, inject synthetic events).
- Local-only state (phantom entries) requires human review -- never auto-delete.
- The system halts normal operations until reconciliation reaches Resolved.

## Anti-Patterns

### Annotating sync traits with #[async_trait]

```rust
// BAD: async_trait on methods that block synchronously
#[async_trait]
pub trait BrokerGateway: Send + Sync {
    fn submit_order(&self, order: &Order) -> Result<Ack, Error>; // sync signature
    fn subscribe_fills(&self, tx: Sender<Fill>) -> Result<(), Error>; // sync but long-lived
}
```

### Blocking the async executor with sync I/O

```rust
// BAD: sync database call on async thread
async fn handle_fill(fill: Fill, db: &Connection) {
    db.execute("INSERT INTO fills ...", params![fill.id])?; // blocks executor
}
```

### Hiding task spawning inside sync methods

```rust
// BAD: no cancellation handle returned
fn subscribe_fills(&self, tx: Sender<Fill>) -> Result<(), Error> {
    tokio::spawn(async move { /* stream events forever */ }); // caller can't cancel
    Ok(())
}
```

### Writing to persistent storage from multiple threads without coordination

```rust
// BAD: two async tasks writing to the same SQLite connection
let conn = Arc::new(Connection::open("db.sqlite")?);
tokio::spawn(write_events(conn.clone()));  // thread 1
tokio::spawn(write_metrics(conn.clone())); // thread 2 -- WAL contention, potential corruption
```
