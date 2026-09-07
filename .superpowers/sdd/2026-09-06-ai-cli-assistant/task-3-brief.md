# Task 3: SQLite Storage & Local In-Memory Schema Cache

**Plan File:** `docs/superpowers/plans/2026-09-06-ai-cli-assistant.md`  
**Spec File:** `docs/superpowers/specs/2026-09-06-ai-cli-assistant-design.md`

## Files
- Create: `src/storage.rs`
- Test: `tests/storage_test.rs`

## Interfaces
- Consumes: `CliCommandSchema`, `CliFlag` from `models.rs`
- Produces:
  - `SchemaStore`:
    - `new(db_path: &Path) -> Result<SchemaStore, ChelpError>`
    - `save_schema(&self, schema: &CliCommandSchema) -> Result<(), ChelpError>`
    - `get_schema(&self, binary: &str, subcommands: &[String]) -> Result<Option<CliCommandSchema>, ChelpError>`
    - `match_flags(&self, binary: &str, subcommands: &[String], prefix: &str) -> Result<Vec<CliFlag>, ChelpError>`

## Steps

### Step 1: Write the failing test

```rust
// tests/storage_test.rs
use chelp::models::{CliCommandSchema, CliFlag};
use chelp::storage::SchemaStore;
use tempfile::NamedTempFile;

#[test]
fn test_save_and_retrieve_schema() {
    let tmp_file = NamedTempFile::new().unwrap();
    let store = SchemaStore::new(tmp_file.path()).unwrap();

    let flag = CliFlag {
        short: Some("-p".to_string()),
        long: Some("--port".to_string()),
        takes_value: true,
        value_hint: Some("PORT".to_string()),
        description: "Port to bind".to_string(),
    };

    let schema = CliCommandSchema {
        binary: "testcli".to_string(),
        subcommand_path: vec!["serve".to_string()],
        usage: "testcli serve --port <PORT>".to_string(),
        description: "Start the server".to_string(),
        flags: vec![flag],
        subcommands: vec![],
        binary_mtime: 12345,
        last_indexed: 67890,
    };

    store.save_schema(&schema).expect("save failed");

    let loaded = store.get_schema("testcli", &["serve".to_string()]).unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.binary, "testcli");
    assert_eq!(loaded.flags[0].long.as_deref(), Some("--port"));

    let matches = store.match_flags("testcli", &["serve".to_string()], "--p").unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].long.as_deref(), Some("--port"));
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test storage_test`  
Expected: FAIL (module `chelp::storage` not found)

### Step 3: Write minimal implementation

```rust
// src/storage.rs
use crate::error::ChelpError;
use crate::models::{CliCommandSchema, CliFlag};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SchemaStore {
    conn: Arc<Mutex<Connection>>,
}

impl SchemaStore {
    pub fn new(db_path: &Path) -> Result<Self, ChelpError> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cli_schemas (
                binary TEXT NOT NULL,
                subcommand_path TEXT NOT NULL,
                schema_json TEXT NOT NULL,
                binary_mtime INTEGER NOT NULL,
                last_indexed INTEGER NOT NULL,
                PRIMARY KEY (binary, subcommand_path)
            )",
            [],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn save_schema(&self, schema: &CliCommandSchema) -> Result<(), ChelpError> {
        let subcmd_str = schema.subcommand_path.join(" ");
        let schema_json = serde_json::to_string(schema)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO cli_schemas (binary, subcommand_path, schema_json, binary_mtime, last_indexed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                schema.binary,
                subcmd_str,
                schema_json,
                schema.binary_mtime as i64,
                schema.last_indexed as i64
            ],
        )?;
        Ok(())
    }

    pub fn get_schema(&self, binary: &str, subcommands: &[String]) -> Result<Option<CliCommandSchema>, ChelpError> {
        let subcmd_str = subcommands.join(" ");
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT schema_json FROM cli_schemas WHERE binary = ?1 AND subcommand_path = ?2",
        )?;
        let mut rows = stmt.query(params![binary, subcmd_str])?;

        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let schema: CliCommandSchema = serde_json::from_str(&json)?;
            Ok(Some(schema))
        } else {
            Ok(None)
        }
    }

    pub fn match_flags(&self, binary: &str, subcommands: &[String], prefix: &str) -> Result<Vec<CliFlag>, ChelpError> {
        if let Some(schema) = self.get_schema(binary, subcommands)? {
            let prefix_lower = prefix.to_lowercase();
            let matches = schema
                .flags
                .into_iter()
                .filter(|f| {
                    f.long.as_ref().map_or(false, |l| l.to_lowercase().starts_with(&prefix_lower))
                        || f.short.as_ref().map_or(false, |s| s.to_lowercase().starts_with(&prefix_lower))
                })
                .collect();
            Ok(matches)
        } else {
            Ok(vec![])
        }
    }
}
```

Add `pub mod storage;` to `src/lib.rs`.

### Step 4: Run test to verify it passes

Run: `cargo test --test storage_test`  
Expected: PASS

### Step 5: Commit

```bash
git add src/storage.rs tests/storage_test.rs src/lib.rs
git commit -m "feat: implement SQLite schema store and flag matching"
```
