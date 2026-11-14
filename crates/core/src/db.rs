//! Persistência local (SQLite) do histórico de tarefas, eventos e consumo de tokens.
//! Nunca guarda credenciais: só o que as CLIs imprimem na saída da tarefa.

use crate::agents::provider::TokenUsage;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: i64,
    pub project: String,
    pub provider: String,
    pub prompt: String,
    /// Estado serializado (JSON do `TaskState` no orquestrador).
    pub state: String,
    pub created_at: i64,
    pub workdir: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: i64,
    pub task_id: i64,
    pub kind: String,
    /// JSON serializado do evento.
    pub payload: String,
    pub created_at: i64,
    /// Id do subagente a que o evento pertence (None = agente principal).
    #[serde(default)]
    pub subagent: Option<String>,
}

/// Uma linha de consumo (um `AgentEvent::Usage`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageRow {
    pub task_id: i64,
    pub project: String,
    pub provider: String,
    pub usage: TokenUsage,
    pub created_at: i64,
    /// Identificador do turno que gerou a linha (um turno com dois modelos grava
    /// duas linhas com o mesmo valor). None nas linhas antigas.
    #[serde(default)]
    pub turn: Option<i64>,
}

impl UsageRow {
    /// Chave do turno, para contar turnos sem repetir: (tarefa, turno). Nas linhas
    /// antigas, sem a coluna, vale o segundo da gravação (as linhas de um mesmo
    /// turno eram gravadas juntas).
    pub fn turn_key(&self) -> (i64, bool, i64) {
        match self.turn {
            Some(t) => (self.task_id, true, t),
            None => (self.task_id, false, self.created_at),
        }
    }
}

/// Consumo agregado por projeto + provedor + modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectUsage {
    pub project: String,
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// Soma dos custos informados pela CLI (None se nenhuma linha informou).
    pub reported_cost_usd: Option<f64>,
    /// Turnos em que este modelo foi usado. Um turno com dois modelos aparece
    /// nas duas linhas: não some entre modelos (use `metrics::by_project`).
    pub turns: u64,
}

impl ProjectUsage {
    pub fn as_usage(&self) -> TokenUsage {
        TokenUsage {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
            model: self.model.clone(),
            cost_usd: self.reported_cost_usd,
        }
    }
}

/// Intervalo [from, to) em segundos Unix. `None` = sem limite.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub from: Option<i64>,
    pub to: Option<i64>,
}

impl TimeRange {
    pub fn all() -> Self {
        Self::default()
    }
    /// Últimos `days` dias até agora.
    pub fn last_days(days: i64) -> Self {
        Self { from: Some(now() - days * 86_400), to: None }
    }
    fn bounds(&self) -> (i64, i64) {
        (self.from.unwrap_or(i64::MIN), self.to.unwrap_or(i64::MAX))
    }
}

/// Banco thread-safe (conexão protegida por Mutex).
pub struct Db {
    conn: Mutex<Connection>,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn to_i(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        if let Some(p) = path.as_ref().parent() {
            let _ = std::fs::create_dir_all(p);
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS tasks(
                id INTEGER PRIMARY KEY,
                project TEXT NOT NULL,
                provider TEXT NOT NULL,
                prompt TEXT NOT NULL,
                state TEXT NOT NULL,
                created_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS events(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL REFERENCES tasks(id),
                kind TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_events_task ON events(task_id);
             CREATE TABLE IF NOT EXISTS usage(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                project TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                cache_read INTEGER NOT NULL,
                cache_write INTEGER NOT NULL,
                cost_usd REAL,
                created_at INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_usage_time ON usage(created_at);
             CREATE TABLE IF NOT EXISTS autowork(
                project TEXT PRIMARY KEY,
                settings TEXT NOT NULL);",
        )?;
        // Migração de bancos antigos: colunas novas em `tasks`.
        for col in ["workdir TEXT", "session_id TEXT"] {
            let name = col.split_whitespace().next().unwrap();
            let exists: bool = conn
                .prepare("SELECT 1 FROM pragma_table_info('tasks') WHERE name=?1")?
                .exists([name])?;
            if !exists {
                conn.execute_batch(&format!("ALTER TABLE tasks ADD COLUMN {col};"))?;
            }
        }
        // Idem em `events`: id do subagente dono do evento.
        let has_sub: bool =
            conn.prepare("SELECT 1 FROM pragma_table_info('events') WHERE name='subagent'")?.exists([])?;
        if !has_sub {
            conn.execute_batch("ALTER TABLE events ADD COLUMN subagent TEXT;")?;
        }
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_events_sub ON events(task_id, subagent);")?;
        // Idem em `usage`: turno que gerou a linha (para contar turnos, não linhas).
        let has_turn: bool =
            conn.prepare("SELECT 1 FROM pragma_table_info('usage') WHERE name='turn'")?.exists([])?;
        if !has_turn {
            conn.execute_batch("ALTER TABLE usage ADD COLUMN turn INTEGER;")?;
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn c(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Insere a tarefa; com `id` = None o SQLite gera um. Devolve o id.
    pub fn insert_task(&self, id: Option<i64>, project: &str, provider: &str, prompt: &str, state: &str) -> rusqlite::Result<i64> {
        let c = self.c();
        c.execute(
            "INSERT INTO tasks(id, project, provider, prompt, state, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
            params![id, project, provider, prompt, state, now()],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn update_task_state(&self, id: i64, state: &str) -> rusqlite::Result<()> {
        self.c().execute("UPDATE tasks SET state=?2 WHERE id=?1", params![id, state])?;
        Ok(())
    }

    pub fn set_task_session(&self, id: i64, session_id: &str) -> rusqlite::Result<()> {
        self.c().execute("UPDATE tasks SET session_id=?2 WHERE id=?1", params![id, session_id])?;
        Ok(())
    }

    pub fn set_task_workdir(&self, id: i64, workdir: &str) -> rusqlite::Result<()> {
        self.c().execute("UPDATE tasks SET workdir=?2 WHERE id=?1", params![id, workdir])?;
        Ok(())
    }

    pub fn max_task_id(&self) -> rusqlite::Result<i64> {
        self.c().query_row("SELECT COALESCE(MAX(id),0) FROM tasks", [], |r| r.get(0))
    }

    pub fn get_task(&self, id: i64) -> rusqlite::Result<Option<TaskRecord>> {
        self.c()
            .query_row(
                "SELECT id, project, provider, prompt, state, created_at, workdir, session_id FROM tasks WHERE id=?1",
                [id],
                task_row,
            )
            .optional()
    }

    pub fn list_tasks(&self) -> rusqlite::Result<Vec<TaskRecord>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT id, project, provider, prompt, state, created_at, workdir, session_id FROM tasks ORDER BY id DESC",
        )?;
        let rows = st.query_map([], task_row)?;
        rows.collect()
    }

    pub fn insert_event(&self, task_id: i64, kind: &str, payload: &str) -> rusqlite::Result<i64> {
        self.insert_event_sub(task_id, kind, payload, None)
    }

    /// Como `insert_event`, marcando o subagente a que o evento pertence.
    pub fn insert_event_sub(&self, task_id: i64, kind: &str, payload: &str, subagent: Option<&str>) -> rusqlite::Result<i64> {
        let c = self.c();
        c.execute(
            "INSERT INTO events(task_id, kind, payload, created_at, subagent) VALUES (?1,?2,?3,?4,?5)",
            params![task_id, kind, payload, now(), subagent],
        )?;
        Ok(c.last_insert_rowid())
    }

    /// Todos os eventos da tarefa (agente principal e subagentes), em ordem.
    pub fn list_events(&self, task_id: i64) -> rusqlite::Result<Vec<EventRecord>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT id, task_id, kind, payload, created_at, subagent FROM events WHERE task_id=?1 ORDER BY id",
        )?;
        let rows = st.query_map([task_id], event_row)?;
        rows.collect()
    }

    /// Eventos de um subagente da tarefa: o `SubagentStart`, os internos e o `SubagentEnd`.
    pub fn list_subagent_events(&self, task_id: i64, subagent: &str) -> rusqlite::Result<Vec<EventRecord>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT id, task_id, kind, payload, created_at, subagent FROM events
             WHERE task_id=?1 AND subagent=?2 ORDER BY id",
        )?;
        let rows = st.query_map(params![task_id, subagent], event_row)?;
        rows.collect()
    }

    pub fn insert_usage(&self, task_id: i64, project: &str, provider: &str, u: &TokenUsage) -> rusqlite::Result<i64> {
        self.insert_usage_at(task_id, project, provider, u, now())
    }

    /// Como `insert_usage`, marcando o turno: todas as linhas de um mesmo turno
    /// (uma por modelo) levam o mesmo `turn`.
    pub fn insert_usage_turn(&self, task_id: i64, project: &str, provider: &str, u: &TokenUsage, turn: i64) -> rusqlite::Result<i64> {
        self.insert_usage_full(task_id, project, provider, u, now(), Some(turn))
    }

    /// Como `insert_usage`, com data explícita (importação/testes).
    pub fn insert_usage_at(&self, task_id: i64, project: &str, provider: &str, u: &TokenUsage, at: i64) -> rusqlite::Result<i64> {
        self.insert_usage_full(task_id, project, provider, u, at, None)
    }

    /// Data e turno explícitos.
    pub fn insert_usage_full(
        &self,
        task_id: i64,
        project: &str,
        provider: &str,
        u: &TokenUsage,
        at: i64,
        turn: Option<i64>,
    ) -> rusqlite::Result<i64> {
        let c = self.c();
        c.execute(
            "INSERT INTO usage(task_id, project, provider, model, input_tokens, output_tokens, cache_read, cache_write, cost_usd, created_at, turn)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                task_id,
                project,
                provider,
                u.model,
                to_i(u.input_tokens),
                to_i(u.output_tokens),
                to_i(u.cache_read),
                to_i(u.cache_write),
                u.cost_usd,
                at,
                turn
            ],
        )?;
        Ok(c.last_insert_rowid())
    }

    /// Configuração do auto-work de um projeto (JSON).
    pub fn get_autowork(&self, project: &str) -> rusqlite::Result<Option<String>> {
        self.c().query_row("SELECT settings FROM autowork WHERE project=?1", [project], |r| r.get(0)).optional()
    }

    pub fn set_autowork(&self, project: &str, settings: &str) -> rusqlite::Result<()> {
        self.c().execute(
            "INSERT INTO autowork(project, settings) VALUES (?1,?2) ON CONFLICT(project) DO UPDATE SET settings=excluded.settings",
            params![project, settings],
        )?;
        Ok(())
    }

    pub fn list_autowork(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let c = self.c();
        let mut st = c.prepare("SELECT project, settings FROM autowork ORDER BY project")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    /// Linhas brutas de consumo no intervalo (para agregações por dia/provedor em `metrics`).
    pub fn usage_rows(&self, range: TimeRange) -> rusqlite::Result<Vec<UsageRow>> {
        let (a, b) = range.bounds();
        let c = self.c();
        let mut st = c.prepare(
            "SELECT task_id, project, provider, model, input_tokens, output_tokens, cache_read, cache_write, cost_usd, created_at, turn
             FROM usage WHERE created_at >= ?1 AND created_at < ?2 ORDER BY created_at, id",
        )?;
        let rows = st.query_map([a, b], |r| {
            Ok(UsageRow {
                task_id: r.get(0)?,
                project: r.get(1)?,
                provider: r.get(2)?,
                usage: TokenUsage {
                    model: r.get(3)?,
                    input_tokens: r.get::<_, i64>(4)?.max(0) as u64,
                    output_tokens: r.get::<_, i64>(5)?.max(0) as u64,
                    cache_read: r.get::<_, i64>(6)?.max(0) as u64,
                    cache_write: r.get::<_, i64>(7)?.max(0) as u64,
                    cost_usd: r.get(8)?,
                },
                created_at: r.get(9)?,
                turn: r.get(10)?,
            })
        })?;
        rows.collect()
    }

    /// Consumo agregado por projeto/provedor/modelo no intervalo.
    pub fn usage_by_project(&self, range: TimeRange) -> rusqlite::Result<Vec<ProjectUsage>> {
        let (a, b) = range.bounds();
        let c = self.c();
        let mut st = c.prepare(
            "SELECT project, provider, model, SUM(input_tokens), SUM(output_tokens), SUM(cache_read), SUM(cache_write),
                    SUM(cost_usd),
                    COUNT(DISTINCT task_id || CASE WHEN turn IS NULL THEN ':s' || created_at ELSE ':t' || turn END)
             FROM usage WHERE created_at >= ?1 AND created_at < ?2
             GROUP BY project, provider, model ORDER BY project, provider, model",
        )?;
        let rows = st.query_map([a, b], |r| {
            Ok(ProjectUsage {
                project: r.get(0)?,
                provider: r.get(1)?,
                model: r.get(2)?,
                input_tokens: r.get::<_, i64>(3)?.max(0) as u64,
                output_tokens: r.get::<_, i64>(4)?.max(0) as u64,
                cache_read: r.get::<_, i64>(5)?.max(0) as u64,
                cache_write: r.get::<_, i64>(6)?.max(0) as u64,
                reported_cost_usd: r.get(7)?,
                turns: r.get::<_, i64>(8)?.max(0) as u64,
            })
        })?;
        rows.collect()
    }
}

fn event_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
    Ok(EventRecord {
        id: r.get(0)?,
        task_id: r.get(1)?,
        kind: r.get(2)?,
        payload: r.get(3)?,
        created_at: r.get(4)?,
        subagent: r.get(5)?,
    })
}

fn task_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRecord> {
    Ok(TaskRecord {
        id: r.get(0)?,
        project: r.get(1)?,
        provider: r.get(2)?,
        prompt: r.get(3)?,
        state: r.get(4)?,
        created_at: r.get(5)?,
        workdir: r.get(6)?,
        session_id: r.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let db = Db::open_in_memory().unwrap();
        let id = db.insert_task(Some(7), "/p", "claude", "faça x", "Queued").unwrap();
        assert_eq!(id, 7);
        db.update_task_state(7, "Completed").unwrap();
        db.set_task_session(7, "sess").unwrap();
        db.set_task_workdir(7, "/wt").unwrap();
        db.insert_event(7, "Text", "\"oi\"").unwrap();
        db.insert_event(7, "Result", "{}").unwrap();
        let t = db.list_tasks().unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].state, "Completed");
        assert_eq!(t[0].session_id.as_deref(), Some("sess"));
        assert_eq!(db.get_task(7).unwrap().unwrap().workdir.as_deref(), Some("/wt"));
        assert!(db.get_task(8).unwrap().is_none());
        assert_eq!(db.max_task_id().unwrap(), 7);
        db.insert_event_sub(7, "Text", "\"sub\"", Some("toolu_1")).unwrap();
        db.insert_event_sub(7, "Text", "\"outro\"", Some("toolu_2")).unwrap();
        let e = db.list_events(7).unwrap();
        assert_eq!(e.len(), 4);
        assert_eq!(e[0].kind, "Text");
        assert_eq!(e[0].subagent, None);
        assert_eq!(e[2].subagent.as_deref(), Some("toolu_1"));
        let sub = db.list_subagent_events(7, "toolu_1").unwrap();
        assert_eq!(sub.len(), 1);
        assert_eq!(sub[0].payload, "\"sub\"");
        assert!(db.list_subagent_events(8, "toolu_1").unwrap().is_empty());
        assert_eq!(db.get_autowork("/p").unwrap(), None);
        db.set_autowork("/p", "{}").unwrap();
        db.set_autowork("/p", "{\"enabled\":true}").unwrap();
        assert_eq!(db.get_autowork("/p").unwrap().as_deref(), Some("{\"enabled\":true}"));
        assert_eq!(db.list_autowork().unwrap().len(), 1);
    }

    #[test]
    fn usage_aggregation() {
        let db = Db::open_in_memory().unwrap();
        let u = |i, m: &str, c| TokenUsage {
            input_tokens: i,
            output_tokens: 10,
            cache_read: 1,
            cache_write: 2,
            model: Some(m.into()),
            cost_usd: c,
        };
        db.insert_usage_at(1, "/a", "claude", &u(100, "sonnet", Some(0.5)), 1_000).unwrap();
        db.insert_usage_at(2, "/a", "claude", &u(50, "sonnet", Some(0.25)), 2_000).unwrap();
        db.insert_usage_at(3, "/b", "opencode", &u(7, "x", None), 3_000).unwrap();
        let all = db.usage_by_project(TimeRange::all()).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].project, "/a");
        assert_eq!(all[0].input_tokens, 150);
        assert_eq!(all[0].output_tokens, 20);
        assert_eq!(all[0].turns, 2);
        assert_eq!(all[0].reported_cost_usd, Some(0.75));
        assert_eq!(all[1].reported_cost_usd, None);
        let r = db.usage_by_project(TimeRange { from: Some(1_500), to: Some(3_000) }).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].input_tokens, 50);
        assert_eq!(db.usage_rows(TimeRange::all()).unwrap().len(), 3);
    }

    /// Um turno que usa dois modelos grava duas linhas, mas é um turno só.
    #[test]
    fn one_turn_two_models_is_one_turn() {
        let db = Db::open_in_memory().unwrap();
        let u = |m: &str| TokenUsage { input_tokens: 5, model: Some(m.into()), ..Default::default() };
        db.insert_usage_turn(1, "/a", "claude", &u("opus"), 77).unwrap();
        db.insert_usage_turn(1, "/a", "claude", &u("haiku"), 77).unwrap();
        db.insert_usage_turn(1, "/a", "claude", &u("opus"), 78).unwrap();
        let rows = db.usage_rows(TimeRange::all()).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].turn, Some(77));
        let keys: std::collections::HashSet<_> = rows.iter().map(UsageRow::turn_key).collect();
        assert_eq!(keys.len(), 2);
        // Por modelo: opus participou de 2 turnos, haiku de 1.
        let by = db.usage_by_project(TimeRange::all()).unwrap();
        let turns = |m: &str| by.iter().find(|r| r.model.as_deref() == Some(m)).unwrap().turns;
        assert_eq!((turns("opus"), turns("haiku")), (2, 1));
    }

    /// Banco antigo, sem a coluna `turn`: migra e as linhas antigas contam pelo segundo.
    #[test]
    fn migrates_usage_without_turn() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("u.db");
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch(
                "CREATE TABLE usage(id INTEGER PRIMARY KEY AUTOINCREMENT, task_id INTEGER NOT NULL, project TEXT NOT NULL,
                 provider TEXT NOT NULL, model TEXT, input_tokens INTEGER NOT NULL, output_tokens INTEGER NOT NULL,
                 cache_read INTEGER NOT NULL, cache_write INTEGER NOT NULL, cost_usd REAL, created_at INTEGER NOT NULL);
                 INSERT INTO usage(task_id, project, provider, model, input_tokens, output_tokens, cache_read, cache_write, created_at)
                 VALUES (1,'/a','claude','opus',1,1,0,0,100), (1,'/a','claude','haiku',1,1,0,0,100), (1,'/a','claude','opus',1,1,0,0,200);",
            )
            .unwrap();
        }
        let db = Db::open(&p).unwrap();
        let rows = db.usage_rows(TimeRange::all()).unwrap();
        assert!(rows.iter().all(|r| r.turn.is_none()));
        let keys: std::collections::HashSet<_> = rows.iter().map(UsageRow::turn_key).collect();
        assert_eq!(keys.len(), 2);
        drop(db);
        assert_eq!(Db::open(&p).unwrap().usage_rows(TimeRange::all()).unwrap().len(), 3);
    }

    #[test]
    fn migrates_old_schema() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("h.db");
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch(
                "CREATE TABLE tasks(id INTEGER PRIMARY KEY, project TEXT NOT NULL, provider TEXT NOT NULL,
                 prompt TEXT NOT NULL, state TEXT NOT NULL, created_at INTEGER NOT NULL);
                 INSERT INTO tasks VALUES(1,'/p','claude','x','Completed',0);
                 CREATE TABLE events(id INTEGER PRIMARY KEY AUTOINCREMENT, task_id INTEGER NOT NULL,
                 kind TEXT NOT NULL, payload TEXT NOT NULL, created_at INTEGER NOT NULL);
                 INSERT INTO events(task_id, kind, payload, created_at) VALUES(1,'Text','antigo',0);",
            )
            .unwrap();
        }
        let db = Db::open(&p).unwrap();
        let t = db.get_task(1).unwrap().unwrap();
        assert_eq!(t.session_id, None);
        db.set_task_session(1, "s").unwrap();
        // Eventos antigos continuam legíveis, sem subagente; os novos aceitam a coluna.
        db.insert_event_sub(1, "Text", "\"novo\"", Some("toolu_1")).unwrap();
        let e = db.list_events(1).unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].subagent, None);
        assert_eq!(e[1].subagent.as_deref(), Some("toolu_1"));
        // Reabrir não tenta criar a coluna de novo.
        drop(db);
        assert_eq!(Db::open(&p).unwrap().list_events(1).unwrap().len(), 2);
    }
}
