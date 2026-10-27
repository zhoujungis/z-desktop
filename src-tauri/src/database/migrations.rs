//! 数据库迁移（设计文档 #28）

use rusqlite::Connection;

/// (版本号, SQL)。SQL 与 /database/migrations/*.sql 保持一致。
pub const MIGRATIONS: &[(&str, &str)] = &[(
    "0001_init",
    include_str!("../../../database/migrations/0001_init.sql"),
)];

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
            // 迁移与版本记录必须在同一事务内：中途失败时不会留下半成品 schema。
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| format!("开启迁移事务失败: {e}"))?;
            tx.execute_batch(sql)
                .map_err(|e| format!("迁移 {version} 失败: {e}"))?;
            tx.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![version, crate::database::now_ms()],
            )
            .map_err(|e| e.to_string())?;
            tx.commit()
                .map_err(|e| format!("提交迁移 {version} 失败: {e}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_recorded_and_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        run(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "迁移版本只应记录一次");
    }
}
