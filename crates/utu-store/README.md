# utu-store

`utu-store` is Utu's local SQLite authority for normalized operational state.
It stores projects, tasks and multi-agent assignments, provider-neutral agents
and sessions, chat, append-ordered events and logs, file changes, connector
readiness, integer-micro cost evidence, attention findings, handoffs, and
control request/receipt pairs.

## Composition

```rust
use utu_store::{SearchQuery, Store};

let store = Store::open("utu.sqlite3")?;
let health = store.health()?;
assert!(health.integrity_ok);

// Demonstration data is opt-in and never inserted by Store::open.
let _demo = store.seed_demo_if_empty()?;
let results = store.search(&SearchQuery::new("authentication"))?;
# Ok::<(), utu_store::StoreError>(())
```

The Tauri composition root chooses the database path and decides whether a
demo workspace is appropriate. Production startup should normally call only
`Store::open`.

## Truth and durability rules

- SQLite foreign keys, write-ahead logging, a busy timeout, and versioned
  transactional migrations are enabled for file-backed stores.
- Credentials and browser session material do not belong in this database.
- Unknown cost uses a null amount; it is never converted to zero. Known values
  use unsigned integer micros in Rust and checked 64-bit integers in SQLite.
- Exact cost, confirmed authentication, delivered handoffs, and acknowledged
  controls require observed evidence. Requests alone are not receipts.
- Messages and events have monotonically increasing per-session local
  sequences. Their provider timestamps remain separate and may arrive out of
  order.
- Provider event replay is idempotent only when a provider event ID is present.
  Repeated text without such an ID remains distinct evidence.
