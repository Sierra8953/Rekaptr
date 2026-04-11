use rusqlite::{params, Connection, Result};
use std::path::Path;

pub struct GameDatabase {
    conn: Connection,
}

impl GameDatabase {
    pub fn open(game_dir: &Path) -> Result<Self> {
        let db_path = game_dir.join("history.db");
        let conn = Connection::open(db_path)?;
        
        // Initialize table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY,
                duration REAL NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;
        
        Ok(Self { conn })
    }

    pub fn register_session(&self, id: u64) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        self.conn.execute(
            "INSERT OR IGNORE INTO sessions (id, duration, created_at) VALUES (?1, ?2, ?3)",
            params![id, 0.0, now],
        )?;
        Ok(())
    }

    pub fn update_duration(&self, id: u64, duration: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET duration = ?1 WHERE id = ?2",
            params![duration, id],
        )?;
        Ok(())
    }

    pub fn get_all_sessions(&self) -> Result<Vec<(u64, f64)>> {
        let mut stmt = self.conn.prepare("SELECT id, duration FROM sessions ORDER BY id ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_session_duration(&self, id: u64) -> Result<Option<f64>> {
        let mut stmt = self.conn.prepare("SELECT duration FROM sessions WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }
}
