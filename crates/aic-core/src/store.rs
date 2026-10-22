//! Persistência em SQLite. Cada entidade é uma linha `(id, json)`: o schema do JSON
//! evolui com `#[serde(default)]` sem precisar de migration.
//!
//! O app e a CLI (`aic add`) abrem o mesmo arquivo. O app detecta escrita de
//! outro processo por `PRAGMA data_version` e recarrega.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};

use crate::model::{Block, Id, Session, Task, Workspace};

pub struct Store {
    conn: Connection,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS tasks      (id TEXT PRIMARY KEY, json TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS workspaces (id TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sessions   (id TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS blocks     (id TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS scrollback (session TEXT PRIMARY KEY, text TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS kv         (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"#;

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path).with_context(|| format!("abrindo {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Muda quando OUTRA conexão escreve no banco.
    pub fn data_version(&self) -> i64 {
        self.conn
            .query_row("PRAGMA data_version", [], |r| r.get(0))
            .unwrap_or(0)
    }

    fn all<T: DeserializeOwned>(&self, table: &str) -> Result<Vec<T>> {
        let mut stmt = self.conn.prepare(&format!("SELECT json FROM {table}"))?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for json in rows {
            match serde_json::from_str(&json?) {
                Ok(v) => out.push(v),
                Err(e) => eprintln!("aic: linha inválida em {table}: {e}"),
            }
        }
        Ok(out)
    }

    fn put<T: Serialize>(&self, table: &str, id: Id, v: &T) -> Result<()> {
        self.conn.execute(
            &format!("INSERT INTO {table} (id, json) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET json = excluded.json"),
            params![id.to_string(), serde_json::to_string(v)?],
        )?;
        Ok(())
    }

    fn delete(&self, table: &str, id: Id) -> Result<()> {
        self.conn.execute(
            &format!("DELETE FROM {table} WHERE id = ?1"),
            params![id.to_string()],
        )?;
        Ok(())
    }

    // ── tarefas ──

    pub fn tasks(&self) -> Result<Vec<Task>> {
        self.all("tasks")
    }

    pub fn task(&self, id: Id) -> Result<Option<Task>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM tasks WHERE id = ?1",
                params![id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
    }

    pub fn save_task(&self, t: &Task) -> Result<()> {
        self.conn.execute(
            "INSERT INTO tasks (id, json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json, updated_at = excluded.updated_at",
            params![
                t.id.to_string(),
                serde_json::to_string(t)?,
                t.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn delete_task(&self, id: Id) -> Result<()> {
        self.delete("tasks", id)
    }

    // ── workspaces e sessões (o "cache" estilo tmux) ──

    pub fn workspaces(&self) -> Result<Vec<Workspace>> {
        let mut v: Vec<Workspace> = self.all("workspaces")?;
        v.sort_by_key(|w| w.created_at);
        Ok(v)
    }

    pub fn save_workspace(&self, w: &Workspace) -> Result<()> {
        self.put("workspaces", w.id, w)
    }

    pub fn delete_workspace(&self, id: Id) -> Result<()> {
        self.delete("workspaces", id)
    }

    pub fn sessions(&self) -> Result<Vec<Session>> {
        self.all("sessions")
    }

    pub fn save_session(&self, s: &Session) -> Result<()> {
        self.put("sessions", s.id, s)
    }

    pub fn delete_session(&self, id: Id) -> Result<()> {
        self.delete("sessions", id)?;
        self.conn.execute(
            "DELETE FROM scrollback WHERE session = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    pub fn blocks(&self) -> Result<Vec<Block>> {
        self.all("blocks")
    }

    pub fn save_block(&self, b: &Block) -> Result<()> {
        self.put("blocks", b.id, b)
    }

    pub fn delete_block(&self, id: Id) -> Result<()> {
        self.delete("blocks", id)
    }

    pub fn save_scrollback(&self, session: Id, text: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO scrollback (session, text) VALUES (?1, ?2) ON CONFLICT(session) DO UPDATE SET text = excluded.text",
            params![session.to_string(), text],
        )?;
        Ok(())
    }

    pub fn scrollback(&self, session: Id) -> Option<String> {
        self.conn
            .query_row(
                "SELECT text FROM scrollback WHERE session = ?1",
                params![session.to_string()],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Existe uma sessão anterior salva para oferecer "reabrir"?
    pub fn has_cache(&self) -> bool {
        self.get_kv("cache.saved").as_deref() == Some("1")
            && self
                .conn
                .query_row("SELECT COUNT(*) FROM workspaces", [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap_or(0)
                > 0
    }

    pub fn mark_cache(&self, saved: bool) -> Result<()> {
        self.set_kv("cache.saved", if saved { "1" } else { "0" })
    }

    /// Apaga workspaces, sessões e scrollback. Tarefas ficam.
    pub fn clear_cache(&self) -> Result<()> {
        self.conn.execute_batch("DELETE FROM workspaces; DELETE FROM sessions; DELETE FROM blocks; DELETE FROM scrollback;")?;
        self.mark_cache(false)
    }

    // ── chave/valor ──

    pub fn get_kv(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_kv(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn transaction<F: FnOnce(&Self) -> Result<()>>(&self, f: F) -> Result<()> {
        self.conn.execute_batch("BEGIN")?;
        match f(self) {
            Ok(()) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SessionKind, TaskStatus};

    #[test]
    fn tarefa_ida_e_volta() {
        let s = Store::open_in_memory().unwrap();
        let mut t = Task::new("corrigir juros");
        s.save_task(&t).unwrap();
        t.status = TaskStatus::Doing;
        s.save_task(&t).unwrap();
        let all = s.tasks().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].status, TaskStatus::Doing);
        s.delete_task(t.id).unwrap();
        assert!(s.tasks().unwrap().is_empty());
    }

    #[test]
    fn limpar_cache_mantem_tarefas() {
        let s = Store::open_in_memory().unwrap();
        let w = Workspace::new("Sprint");
        s.save_workspace(&w).unwrap();
        s.save_session(&Session::new(w.id, "a", SessionKind::Claude, "/tmp".into()))
            .unwrap();
        s.save_task(&Task::new("fica")).unwrap();
        s.mark_cache(true).unwrap();
        assert!(s.has_cache());
        s.clear_cache().unwrap();
        assert!(!s.has_cache());
        assert!(s.sessions().unwrap().is_empty());
        assert_eq!(s.tasks().unwrap().len(), 1);
    }

    #[test]
    fn data_version_detecta_outra_conexao() {
        let dir = std::env::temp_dir().join(format!("aic-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("t.db");
        let a = Store::open(&path).unwrap();
        let b = Store::open(&path).unwrap();
        let v0 = a.data_version();
        b.save_task(&Task::new("de fora")).unwrap();
        assert_ne!(a.data_version(), v0);
        assert_eq!(a.tasks().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
