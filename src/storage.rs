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
