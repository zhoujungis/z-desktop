//! 权限系统（设计文档 #30 / #19 / #31）

use super::risk::RiskLevel;
use crate::database::models::AppSettings;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// 权限判定结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub risk_level: String,
    /// 是否需要用户审批
    pub requires_approval: bool,
    /// 是否直接禁止（CRITICAL 默认禁止）
    pub blocked: bool,
    pub reason: String,
}

/// 查询 "Always allow" 规则（设计文档 #31 Approval UI 的 Always 按钮）
fn find_allow_rule(conn: &Connection, tool: &str, input_json: &str) -> bool {
    // 精确匹配 tool + 完整输入；简单可靠
    conn.query_row(
        "SELECT COUNT(*) FROM approval_rules WHERE tool = ?1 AND pattern = ?2 AND decision = 'allow'",
        rusqlite::params![tool, input_json],
        |r| r.get::<_, i64>(0),
    )
    .map(|c| c > 0)
    .unwrap_or(false)
}

/// 综合权限判定：
/// - CRITICAL：默认禁止（设计文档 #30）
/// - SAFE：自动允许
/// - LOW / MEDIUM / HIGH：根据设置与规则决定是否需要审批
/// - shell.exec 使用命令风险分级（设计文档 #19）
pub fn decide(
    conn: &Connection,
    settings: &AppSettings,
    tool: &str,
    tool_risk: RiskLevel,
    input_json: &str,
) -> PermissionDecision {
    let effective_risk = if tool == "shell.exec" {
        let command = serde_json::from_str::<serde_json::Value>(input_json)
            .ok()
            .and_then(|v| v.get("command").and_then(|c| c.as_str()).map(String::from))
            .unwrap_or_default();
        // 命令分级是 shell.exec 的权威结果：classify_command 内部对未知命令
        // 默认返回 MEDIUM，因此不会因为绕过工具级下限而降低安全性。
        super::risk::classify_command(&command)
    } else {
        tool_risk
    };

    if effective_risk == RiskLevel::Critical {
        return PermissionDecision {
            risk_level: effective_risk.as_str().into(),
            requires_approval: false,
            blocked: true,
            reason: "危险系统操作默认禁止（设计文档 #30）".into(),
        };
    }

    // "Always allow" 规则只对 MEDIUM 及以下生效（高风险每次确认）
    if effective_risk <= RiskLevel::Medium && find_allow_rule(conn, tool, input_json) {
        return PermissionDecision {
            risk_level: effective_risk.as_str().into(),
            requires_approval: false,
            blocked: false,
            reason: "命中 Always-allow 规则".into(),
        };
    }

    let auto = match effective_risk {
        // shell.exec 的只读命令额外受 shell_safe_auto_exec 开关控制
        RiskLevel::Safe if tool == "shell.exec" => {
            settings.auto_approve_safe && settings.shell_safe_auto_exec
        }
        RiskLevel::Safe => settings.auto_approve_safe,
        RiskLevel::Low => settings.auto_approve_low,
        RiskLevel::Medium => settings.auto_approve_medium,
        _ => false,
    };

    PermissionDecision {
        risk_level: effective_risk.as_str().into(),
        requires_approval: !auto,
        blocked: false,
        reason: if auto {
            "已按设置自动批准".into()
        } else if effective_risk == RiskLevel::Safe && tool == "shell.exec" {
            "只读 Shell 命令，但「安全命令自动执行」已关闭".into()
        } else {
            "需要用户审批".into()
        },
    }
}

/// 将审批结果写入规则库（Allow Always）
pub fn save_allow_rule(conn: &Connection, tool: &str, input_json: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO approval_rules (id, tool, pattern, decision, created_at)
         VALUES (?1, ?2, ?3, 'allow', ?4)",
        rusqlite::params![
            crate::database::new_id(),
            tool,
            input_json,
            crate::database::now_ms()
        ],
    )
    .map_err(|e| e.to_string())?;
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
    fn safe_auto_allowed() {
        let conn = setup();
        let settings = AppSettings::default();
        let d = decide(&conn, &settings, "filesystem.read", RiskLevel::Safe, "{}");
        assert!(!d.requires_approval && !d.blocked);
    }

    #[test]
    fn medium_requires_approval_by_default() {
        let conn = setup();
        let settings = AppSettings::default();
        let d = decide(
            &conn,
            &settings,
            "shell.exec",
            RiskLevel::Medium,
            r#"{"command":"npm test"}"#,
        );
        assert!(d.requires_approval);
        assert_eq!(d.risk_level, "medium");
    }

    #[test]
    fn shell_critical_blocked() {
        let conn = setup();
        let settings = AppSettings::default();
        let d = decide(
            &conn,
            &settings,
            "shell.exec",
            RiskLevel::Medium,
            r#"{"command":"format C:"}"#,
        );
        assert!(d.blocked);
        assert_eq!(d.risk_level, "critical");
    }

    #[test]
    fn high_always_requires_approval_even_with_rules() {
        let conn = setup();
        let settings = AppSettings::default();
        save_allow_rule(&conn, "filesystem.delete", r#"{"path":"D:/a.txt"}"#).unwrap();
        let d = decide(
            &conn,
            &settings,
            "filesystem.delete",
            RiskLevel::High,
            r#"{"path":"D:/a.txt"}"#,
        );
        assert!(d.requires_approval);
    }

    #[test]
    fn allow_rule_skips_medium() {
        let conn = setup();
        let settings = AppSettings::default();
        let input = r#"{"command":"npm test"}"#;
        save_allow_rule(&conn, "shell.exec", input).unwrap();
        let d = decide(&conn, &settings, "shell.exec", RiskLevel::Medium, input);
        assert!(!d.requires_approval);
    }

    #[test]
    fn shell_safe_auto_exec_switch_is_effective() {
        let conn = setup();
        let settings = AppSettings {
            shell_safe_auto_exec: true,
            ..AppSettings::default()
        };
        let safe_input = r#"{"command":"git status"}"#;
        let d = decide(
            &conn,
            &settings,
            "shell.exec",
            RiskLevel::Medium,
            safe_input,
        );
        assert!(!d.requires_approval, "默认应自动执行只读命令");
        assert_eq!(d.risk_level, "safe");

        let settings = AppSettings {
            shell_safe_auto_exec: false,
            ..AppSettings::default()
        };
        let d = decide(
            &conn,
            &settings,
            "shell.exec",
            RiskLevel::Medium,
            safe_input,
        );
        assert!(d.requires_approval, "关闭开关后只读命令也应审批");
        // 非 shell 的 SAFE 工具不受该开关影响
        let d2 = decide(&conn, &settings, "filesystem.read", RiskLevel::Safe, "{}");
        assert!(!d2.requires_approval);
    }

    #[test]
    fn chained_command_cannot_borrow_safe_status() {
        let conn = setup();
        let settings = AppSettings::default();
        let input = r#"{"command":"echo ok && rm -rf ./build"}"#;
        let d = decide(&conn, &settings, "shell.exec", RiskLevel::Medium, input);
        assert_eq!(d.risk_level, "high");
        assert!(d.requires_approval, "链式危险命令必须审批");
    }
}
