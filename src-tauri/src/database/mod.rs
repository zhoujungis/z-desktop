//! 数据库模块：SQLite 初始化、迁移与模型定义（设计文档 #27 / #28）

pub mod migrations;
pub mod models;

use rusqlite::Connection;
use std::path::Path;

/// 打开（或创建）数据库并执行迁移
pub fn init_db(app_data_dir: &Path) -> Result<Connection, String> {
    std::fs::create_dir_all(app_data_dir)
        .map_err(|e| format!("无法创建数据目录 {}: {e}", app_data_dir.display()))?;
    let db_path = app_data_dir.join("zdesktop.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("无法打开数据库 {}: {e}", db_path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| e.to_string())?;
    migrations::run(&conn)?;
    Ok(conn)
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_db_in_memory() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        // 关键表必须存在
        for table in [
            "tasks",
            "task_steps",
            "events",
            "memories",
            "messages",
            "projects",
            "settings",
            "approval_rules",
            "memories_fts",
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(exists, "表 {table} 应存在");
        }
    }
}
