//! 数据模型与持久化辅助（设计文档 #28 / #29）

use super::{new_id, now_ms};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

/// 任务状态（设计文档 #39）
pub const TASK_STATUSES: &[&str] = &[
    "pending",
    "running",
    "waiting_approval",
    "paused",
    "completed",
    "failed",
    "cancelled",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    pub id: String,
    pub task_id: String,
    pub step_index: i64,
    pub tool: Option<String>,
    pub input: Option<String>,
    pub output: Option<String>,
    pub status: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String,
    pub task_id: Option<String>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    #[serde(rename = "type")]
    pub memory_type: String,
    pub content: String,
    pub source: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub task_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub technology: Option<String>,
    pub git_url: Option<String>,
    pub is_active: bool,
    pub created_at: i64,
}

fn row_to_task(r: &Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: r.get(0)?,
        title: r.get(1)?,
        status: r.get(2)?,
        created_at: r.get(3)?,
        updated_at: r.get(4)?,
        completed_at: r.get(5)?,
    })
}

pub fn create_task(conn: &Connection, title: &str) -> Result<Task, String> {
    let task = Task {
        id: new_id(),
        title: title.to_string(),
        status: "pending".into(),
        created_at: now_ms(),
        updated_at: now_ms(),
        completed_at: None,
    };
    conn.execute(
        "INSERT INTO tasks (id, title, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![task.id, task.title, task.status, task.created_at, task.updated_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(task)
}

pub fn update_task_status(conn: &Connection, id: &str, status: &str) -> Result<(), String> {
    if !TASK_STATUSES.contains(&status) {
        return Err(format!("未知任务状态: {status}"));
    }
    let completed_at = if matches!(status, "completed" | "failed" | "cancelled") {
        Some(now_ms())
    } else {
        // 任务被重新置为进行中时清除旧的完成时间
        None
    };
    conn.execute(
        "UPDATE tasks SET status = ?2, updated_at = ?3, completed_at = ?4 WHERE id = ?1",
        params![id, status, now_ms(), completed_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_task(conn: &Connection, id: &str) -> Result<Option<Task>, String> {
    conn.query_row(
        "SELECT id, title, status, created_at, updated_at, completed_at FROM tasks WHERE id = ?1",
        [id],
        row_to_task,
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn list_tasks(conn: &Connection, limit: i64) -> Result<Vec<Task>, String> {
    let limit = limit.clamp(1, 1000);
    let mut stmt = conn
        .prepare("SELECT id, title, status, created_at, updated_at, completed_at FROM tasks ORDER BY created_at DESC LIMIT ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit], row_to_task)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// 追加任务步骤（Task Timeline，设计文档 #32）
pub fn append_step(
    conn: &Connection,
    task_id: &str,
    tool: Option<&str>,
    input: Option<&str>,
    output: Option<&str>,
    status: &str,
) -> Result<TaskStep, String> {
    let step_index: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(step_index), -1) + 1 FROM task_steps WHERE task_id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let step = TaskStep {
        id: new_id(),
        task_id: task_id.into(),
        step_index,
        tool: tool.map(String::from),
        input: input.map(String::from),
        output: output.map(String::from),
        status: Some(status.into()),
        created_at: now_ms(),
    };
    conn.execute(
        "INSERT INTO task_steps (id, task_id, step_index, tool, input, output, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            step.id,
            step.task_id,
            step.step_index,
            step.tool,
            step.input,
            step.output,
            step.status,
            step.created_at
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(step)
}

pub fn update_step_status(
    conn: &Connection,
    step_id: &str,
    status: &str,
    output: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE task_steps SET status = ?2, output = COALESCE(?3, output) WHERE id = ?1",
        params![step_id, status, output],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_steps(conn: &Connection, task_id: &str) -> Result<Vec<TaskStep>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, task_id, step_index, tool, input, output, status, created_at
             FROM task_steps WHERE task_id = ?1 ORDER BY step_index ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([task_id], |r| {
            Ok(TaskStep {
                id: r.get(0)?,
                task_id: r.get(1)?,
                step_index: r.get(2)?,
                tool: r.get(3)?,
                input: r.get(4)?,
                output: r.get(5)?,
                status: r.get(6)?,
                created_at: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// 记录事件（Activity 面板数据源，设计文档 #13）
pub fn append_event(
    conn: &Connection,
    task_id: Option<&str>,
    event_type: &str,
    data: Option<&str>,
) -> Result<EventRecord, String> {
    let ev = EventRecord {
        id: new_id(),
        task_id: task_id.map(String::from),
        event_type: event_type.into(),
        data: data.map(String::from),
        created_at: now_ms(),
    };
    conn.execute(
        "INSERT INTO events (id, task_id, type, data, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![ev.id, ev.task_id, ev.event_type, ev.data, ev.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(ev)
}

pub fn list_events(conn: &Connection, task_id: Option<&str>, limit: i64) -> Result<Vec<EventRecord>, String> {
    let limit = limit.clamp(1, 5000);
    let mut stmt = conn
        .prepare(
            "SELECT id, task_id, type, data, created_at FROM events
             WHERE (?1 IS NULL OR task_id = ?1) ORDER BY created_at DESC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![task_id, limit], |r| {
            Ok(EventRecord {
                id: r.get(0)?,
                task_id: r.get(1)?,
                event_type: r.get(2)?,
                data: r.get(3)?,
                created_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn save_message(conn: &Connection, task_id: Option<&str>, role: &str, content: &str) -> Result<Message, String> {
    let msg = Message {
        id: new_id(),
        task_id: task_id.map(String::from),
        role: role.into(),
        content: content.into(),
        created_at: now_ms(),
    };
    conn.execute(
        "INSERT INTO messages (id, task_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![msg.id, msg.task_id, msg.role, msg.content, msg.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(msg)
}

pub fn list_messages(conn: &Connection, task_id: &str) -> Result<Vec<Message>, String> {
    let mut stmt = conn
        .prepare("SELECT id, task_id, role, content, created_at FROM messages WHERE task_id = ?1 ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([task_id], |r| {
            Ok(Message {
                id: r.get(0)?,
                task_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                created_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn add_memory(conn: &Connection, memory_type: &str, content: &str, source: Option<&str>) -> Result<Memory, String> {
    let m = Memory {
        id: new_id(),
        memory_type: memory_type.into(),
        content: content.into(),
        source: source.map(String::from),
        created_at: now_ms(),
    };
    conn.execute(
        "INSERT INTO memories (id, type, content, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![m.id, m.memory_type, m.content, m.source, m.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(m)
}

/// 把用户输入转成安全的 FTS5 查询：按空白分词并整体加引号，
/// 避免 `*`、`"`、`:`、`OR` 等特殊语法导致 MATCH 报错。
fn to_fts_query(query: &str) -> Option<String> {
    // 先剔除 FTS5 控制字符，再把标点当分隔符（`alpha:beta` → alpha beta）
    let normalized: String = query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let terms: Vec<String> = normalized
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" OR "))
    }
}

/// 基于 FTS5 的记忆检索（设计文档 #27：SQLite + FTS5）
pub fn search_memories(conn: &Connection, query: &str, limit: i64) -> Result<Vec<Memory>, String> {
    let Some(fts_query) = to_fts_query(query) else {
        return Ok(Vec::new());
    };
    let limit = limit.clamp(1, 100);
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.type, m.content, m.source, m.created_at
             FROM memories_fts f JOIN memories m ON m.rowid = f.rowid
             WHERE memories_fts MATCH ?1
             ORDER BY rank LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![fts_query, limit], |r| {
            Ok(Memory {
                id: r.get(0)?,
                memory_type: r.get(1)?,
                content: r.get(2)?,
                source: r.get(3)?,
                created_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn list_projects(conn: &Connection) -> Result<Vec<Project>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, path, description, technology, git_url, is_active, created_at
             FROM projects ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                path: r.get(2)?,
                description: r.get(3)?,
                technology: r.get(4)?,
                git_url: r.get(5)?,
                is_active: r.get::<_, i64>(6)? != 0,
                created_at: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn add_project(
    conn: &Connection,
    name: &str,
    path: &str,
    description: Option<&str>,
    technology: Option<&str>,
    git_url: Option<&str>,
) -> Result<Project, String> {
    let p = Project {
        id: new_id(),
        name: name.into(),
        path: path.into(),
        description: description.map(String::from),
        technology: technology.map(String::from),
        git_url: git_url.map(String::from),
        is_active: false,
        created_at: now_ms(),
    };
    conn.execute(
        "INSERT INTO projects (id, name, path, description, technology, git_url, is_active, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![p.id, p.name, p.path, p.description, p.technology, p.git_url, p.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(p)
}

pub fn set_active_project(conn: &Connection, id: Option<&str>) -> Result<(), String> {
    conn.execute("UPDATE projects SET is_active = 0", [])
        .map_err(|e| e.to_string())?;
    if let Some(id) = id {
        conn.execute("UPDATE projects SET is_active = 1 WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn get_active_project(conn: &Connection) -> Result<Option<Project>, String> {
    conn.query_row(
        "SELECT id, name, path, description, technology, git_url, is_active, created_at
         FROM projects WHERE is_active = 1 LIMIT 1",
        [],
        |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                path: r.get(2)?,
                description: r.get(3)?,
                technology: r.get(4)?,
                git_url: r.get(5)?,
                is_active: r.get::<_, i64>(6)? != 0,
                created_at: r.get(7)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

// ---------- Settings ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// LLM Provider: openai | anthropic | gemini | openrouter | custom | local
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// 权限自动批准（设计文档 #19 / #30）
    pub auto_approve_safe: bool,
    pub auto_approve_low: bool,
    pub auto_approve_medium: bool,
    /// 安全命令自动执行（设计文档 #19）
    pub shell_safe_auto_exec: bool,
    /// Agent Loop 最大步数上限
    pub max_steps: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            auto_approve_safe: true,
            auto_approve_low: false,
            auto_approve_medium: false,
            shell_safe_auto_exec: true,
            max_steps: 30,
        }
    }
}

pub fn load_settings(conn: &Connection) -> Result<AppSettings, String> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM settings")
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let settings = AppSettings::default();
    // 合并 JSON（逐键覆盖，向后兼容新增键）
    let mut merged = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    if let Some(obj) = merged.as_object_mut() {
        for (k, v) in rows {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&v) {
                obj.insert(k, val);
            }
        }
    }
    serde_json::from_value(merged).map_err(|e| e.to_string())
}

pub fn save_settings(conn: &Connection, settings: &AppSettings) -> Result<(), String> {
    let value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v.to_string()],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn fts_query_is_sanitized() {
        let conn = setup();
        add_memory(&conn, "note", "用户 偏好 深色 主题", None).unwrap();
        add_memory(&conn, "note", "alpha beta", None).unwrap();

        // 特殊语法不应导致 MATCH 报错
        assert_eq!(search_memories(&conn, "深色", 10).unwrap().len(), 1);
        assert_eq!(search_memories(&conn, "\"深色\" OR *", 10).unwrap().len(), 1);
        assert_eq!(search_memories(&conn, "alpha:beta", 10).unwrap().len(), 1);
        assert!(search_memories(&conn, "  ", 10).unwrap().is_empty());
        assert!(search_memories(&conn, "\"*", 10).unwrap().is_empty());
    }

    #[test]
    fn reopening_task_clears_completed_at() {
        let conn = setup();
        let task = create_task(&conn, "t").unwrap();
        update_task_status(&conn, &task.id, "completed").unwrap();
        assert!(get_task(&conn, &task.id).unwrap().unwrap().completed_at.is_some());
        update_task_status(&conn, &task.id, "running").unwrap();
        assert!(
            get_task(&conn, &task.id).unwrap().unwrap().completed_at.is_none(),
            "重新运行时不应保留旧的完成时间"
        );
    }

    #[test]
    fn list_limits_are_clamped() {
        let conn = setup();
        create_task(&conn, "t").unwrap();
        assert_eq!(list_tasks(&conn, -5).unwrap().len(), 1);
        assert_eq!(list_events(&conn, None, -5).unwrap().len(), 0);
    }
}
