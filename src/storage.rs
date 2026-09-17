use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

/// The data a key can hold. Two variants today, but this is the extension
/// point for future value types (sets, hashes, ...).
#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    List(VecDeque<String>),
}

/// A stored value plus whatever bookkeeping goes with it. Kept as its own
/// type - rather than storing `Value` directly - so metadata like a TTL can
/// be added later without changing the store's public API.
#[derive(Debug, Clone)]
struct Entry {
    value: Value,
}

/// Represents errors that can occur while operating on the store.
#[derive(Debug)]
pub enum StorageError {
    /// A command that only works on one value type (e.g. `LPUSH` on a list)
    /// was used against a key holding a different type.
    WrongType,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::WrongType => {
                "WRONGTYPE Operation against a key holding the wrong kind of value".fmt(formatter)
            }
        }
    }
}

/// The server's in-memory key-value store, shared across every client
/// connection.
///
/// Cloning a `KeyValueStore` is cheap - it clones only the `Arc`, so every
/// clone reads and writes the same underlying data. `RwLock` lets concurrent
/// `GET`s proceed in parallel, serializing only when a write needs exclusive
/// access.
#[derive(Debug, Clone, Default)]
pub struct KeyValueStore {
    entries: Arc<RwLock<HashMap<String, Entry>>>,
}

impl KeyValueStore {
    pub fn new() -> KeyValueStore {
        KeyValueStore::default()
    }

    /// Store `value` under `key`, overwriting whatever was there before -
    /// including a value of a different type, matching real Redis's `SET`.
    pub fn set(&self, key: String, value: Value) {
        let mut entries = self.entries.write().expect("key-value store lock poisoned");
        entries.insert(key, Entry { value });
    }

    /// Look up `key`, returning a clone of its value if present.
    pub fn get(&self, key: &str) -> Option<Value> {
        let entries = self.entries.read().expect("key-value store lock poisoned");
        entries.get(key).map(|entry| entry.value.clone())
    }

    /// `LPUSH`: push each of `values` onto the front of the list at `key`,
    /// creating an empty list first if `key` doesn't exist yet. Returns the
    /// list's length after the push.
    pub fn push_front(&self, key: String, values: Vec<String>) -> Result<usize, StorageError> {
        self.push(key, values, VecDeque::push_front)
    }

    /// `RPUSH`: push each of `values` onto the back of the list at `key`,
    /// creating an empty list first if `key` doesn't exist yet. Returns the
    /// list's length after the push.
    pub fn push_back(&self, key: String, values: Vec<String>) -> Result<usize, StorageError> {
        self.push(key, values, VecDeque::push_back)
    }

    fn push(
        &self,
        key: String,
        values: Vec<String>,
        insert: fn(&mut VecDeque<String>, String),
    ) -> Result<usize, StorageError> {
        let mut entries = self.entries.write().expect("key-value store lock poisoned");
        let stored_entry = entries
            .entry(key)
            .or_insert_with(|| Entry {
                value: Value::List(VecDeque::new()),
            });

        let list = match &mut stored_entry.value {
            Value::List(list) => list,
            Value::String(_) => return Err(StorageError::WrongType),
        };

        for value in values {
            insert(list, value);
        }

        Ok(list.len())
    }

    /// `LRANGE`: return a clone of the elements between `start` and `stop`
    /// (both inclusive, zero-based). Negative indices count from the end of
    /// the list (`-1` is the last element). Out-of-range indices are clamped
    /// rather than treated as errors, and a missing key behaves like an
    /// empty list - matching real Redis.
    pub fn range(&self, key: &str, start: i64, stop: i64) -> Result<Vec<String>, StorageError> {
        let entries = self.entries.read().expect("key-value store lock poisoned");

        let list = match entries.get(key) {
            Some(entry) => match &entry.value {
                Value::List(list) => list,
                Value::String(_) => return Err(StorageError::WrongType),
            },
            None => return Ok(Vec::new()),
        };

        let length = list.len() as i64;
        let normalize_index = |index: i64| if index < 0 { (length + index).max(0) } else { index };

        let start = normalize_index(start);
        let stop = normalize_index(stop).min(length - 1);

        if start > stop {
            return Ok(Vec::new());
        }

        Ok(list.range(start as usize..=stop as usize).cloned().collect())
    }
}
