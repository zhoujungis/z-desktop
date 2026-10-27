//! 数据库迁移（设计文档 #28）

use rusqlite::Connection;

/// (版本号, SQL)。SQL 与 /database/migrations/*.sql 保持一致。
pub const MIGRATIONS: &[(&str, &str)] = &[("0001_init", include_str!("../../../database/migrations/0001_init.sql"))];

pub fn run(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )
    .map_err(|e| e.to_string())?;

    for (version, sql) in MIGRATIONS {
        let applied: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM schema_migrations WHERE version = ?1",
                [version],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !applied {
            conn.execute_batch(sql).map_err(|e| format!("迁移 {version} 失败: {e}"))?;
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![version, crate::database::now_ms()],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
