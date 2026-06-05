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
            .unwrap_or_default()
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

}
