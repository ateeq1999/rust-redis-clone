use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// The data a key can hold. A single variant today, but this is the
/// extension point for future value types (lists, sets, hashes, ...).
#[derive(Debug, Clone)]
pub enum Value {
    String(String),
}

/// A stored value plus whatever bookkeeping goes with it. Kept as its own
/// type - rather than storing `Value` directly - so metadata like a TTL can
/// be added later without changing the store's public API.
#[derive(Debug, Clone)]
struct Entry {
    value: Value,
}

/// The server's in-memory key-value store, shared across every client
/// connection.
///
/// Cloning a `KeyValueStore` is cheap - it clones only the `Arc`, so every
/// clone reads and writes the same underlying data. `RwLock` lets concurrent
/// `GET`s proceed in parallel, serializing only when a `SET` needs to write.
#[derive(Debug, Clone, Default)]
pub struct KeyValueStore {
    entries: Arc<RwLock<HashMap<String, Entry>>>,
}

impl KeyValueStore {
    pub fn new() -> KeyValueStore {
        KeyValueStore::default()
    }

    /// Store `value` under `key`, overwriting whatever was there before.
    pub fn set(&self, key: String, value: Value) {
        let mut entries = self.entries.write().expect("key-value store lock poisoned");
        entries.insert(key, Entry { value });
    }

    /// Look up `key`, returning a clone of its value if present.
    pub fn get(&self, key: &str) -> Option<Value> {
        let entries = self.entries.read().expect("key-value store lock poisoned");
        entries.get(key).map(|entry| entry.value.clone())
    }
}
