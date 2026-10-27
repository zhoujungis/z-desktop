//! Agent 引擎核心（Rust 侧，设计文档 #27-#29 / #40）
//!
//! Rust 负责权限判定、审批流、工具执行与任务停止；
//! LLM 规划与 Agent Loop 由前端驱动（见 src/agent/loop.ts）。

use crate::security::permission;
use crate::state::{AppState, ApprovalState, PendingApproval};
use serde_json::{json, Value};
use std::time::Instant;

/// 审批输入校验时的关键字段（防止"批准 A 执行 B"）
fn same_request(stored: &PendingApproval, tool: &str, input: &Value) -> bool {
    stored.tool == tool && stored.input == *input
}

fn summarize(tool: &str, input: &Value) -> String {
    match tool {
        "shell.exec" => {
            let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
            truncate_text(&format!("执行命令: {cmd}"), 140)
        }
        "filesystem.read" | "filesystem.list" | "filesystem.stat" => {
            let p = input.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            truncate_text(&format!("读取 {p}"), 140)
        }
        "filesystem.write" => {
            let p = input.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            truncate_text(&format!("写入文件 {p}"), 140)
        }
        "filesystem.delete" => {
            let p = input.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            truncate_text(&format!("删除 {p}"), 140)
        }
        "filesystem.move" | "filesystem.copy" | "filesystem.rename" => {
            let f = input
                .get("from")
                .or_else(|| input.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let t = input
                .get("to")
                .or_else(|| input.get("new_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            truncate_text(&format!("{tool}: {f} → {t}"), 140)
        }
        "process.start" => {
            let c = input.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            format!("启动应用 {c}")
        }
        "process.kill" => {
            let pid = input.get("pid").cloned().unwrap_or(Value::Null);
            format!("终止进程 {pid}")
        }
        "ui.click" | "ui.type" | "ui.select" => {
            let n = input.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{tool}: {n}")
        }
        _ => tool.to_string(),
    }
}

fn truncate_text(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}…")
}

const STEP_INPUT_LIMIT: usize = 4_000;
const STEP_OUTPUT_LIMIT: usize = 8_000;

/// 记录一次工具步骤；返回 step id 供完成时更新状态。
fn record_step(
    state: &AppState,
    task_id: Option<&str>,
    tool: &str,
    input: &Value,
    status: &str,
    output: Option<&str>,
) -> Option<String> {
    let task_id = task_id?;
    let input_text = truncate_text(&input.to_string(), STEP_INPUT_LIMIT);
    let output_text = output.map(|o| truncate_text(o, STEP_OUTPUT_LIMIT));
    let conn = state.db.lock().unwrap();
    crate::database::models::append_step(
        &conn,
        task_id,
        Some(tool),
        Some(&input_text),
        output_text.as_deref(),
        status,
    )
    .ok()
    .map(|step| step.id)
}

fn finish_step(state: &AppState, step_id: Option<&str>, status: &str, output: Option<&str>) {
    let Some(step_id) = step_id else {
        return;
    };
    let output_text = output.map(|o| truncate_text(o, STEP_OUTPUT_LIMIT));
    let conn = state.db.lock().unwrap();
    let _ =
        crate::database::models::update_step_status(&conn, step_id, status, output_text.as_deref());
}

/// 工具执行请求（Agent Loop 的每一步都经过这里）
///
/// 返回：
/// - {status:"executed", result, durationMs}
/// - {status:"failed", error, durationMs}（工具错误作为观察结果返回给 LLM）
/// - {status:"blocked", reason, riskLevel}
/// - {status:"approval_required", requestId, riskLevel, reason}
/// - {status:"cancelled"}
pub async fn request_tool_execution(
    state: &AppState,
    task_id: Option<&str>,
    tool: &str,
    input: Value,
    approval_request_id: Option<&str>,
) -> Result<Value, String> {
    // 0. 取消检查（设计文档 #40）
    if let Some(tid) = task_id {
        if state.is_cancelled(tid) {
            return Ok(json!({ "status": "cancelled" }));
        }
    }

    // 1. 复用已批准的请求：校验工具与参数一致后消费
    if let Some(rid) = approval_request_id {
        let valid = {
            let mut map = state.pending_approvals.lock().unwrap();
            match map.get(rid) {
                Some(st)
                    if st.decision.as_deref() == Some("allow")
                        && same_request(&st.request, tool, &input) =>
                {
                    map.remove(rid);
                    true
                }
                _ => false,
            }
        };
        if !valid {
            return Err("审批请求无效（不存在、未批准或参数已变化）".into());
        }
    } else {
        // 2. 权限判定（设计文档 #19 / #30）
        let settings = state.settings_snapshot();
        let decision = {
            let conn = state.db.lock().unwrap();
            permission::decide(
                &conn,
                &settings,
                tool,
                crate::state::tool_risk(tool),
                &input.to_string(),
            )
        };
        if decision.blocked {
            record_step(
                state,
                task_id,
                tool,
                &input,
                "blocked",
                Some(decision.reason.as_str()),
            );
            state.emit_activity(
                task_id,
                "tool_blocked",
                Some(tool),
                &format!("已拦截 {tool}：{}", decision.reason),
                Some(&input.to_string()),
            );
            return Ok(json!({
                "status": "blocked",
                "reason": decision.reason,
                "riskLevel": decision.risk_level,
            }));
        }
        if decision.requires_approval {
            let request_id = crate::database::new_id();
            let request = PendingApproval {
                request_id: request_id.clone(),
                task_id: task_id.map(String::from),
                tool: tool.to_string(),
                input: input.clone(),
                risk_level: decision.risk_level.clone(),
                summary: summarize(tool, &input),
            };
            // 先入队再广播：否则用户可能在事件到达后立刻点击审批，
            // 而 resolve_approval 还找不到该 requestId。
            state.pending_approvals.lock().unwrap().insert(
                request_id.clone(),
                ApprovalState {
                    request,
                    decision: None,
                },
            );
            let pending = state
                .pending_approvals
                .lock()
                .unwrap()
                .get(&request_id)
                .map(|st| st.request.clone());
            if let Some(request) = pending {
                state.emit_approval_requested(&request);
            }
            return Ok(json!({
                "status": "approval_required",
                "requestId": request_id,
                "riskLevel": decision.risk_level,
                "reason": decision.reason,
            }));
        }
    }

    // 3. 执行工具
    let started = Instant::now();
    let step_id = record_step(state, task_id, tool, &input, "running", None);
    state.emit_activity(
        task_id,
        "tool_started",
        Some(tool),
        &format!("执行 {tool}"),
        Some(&input.to_string()),
    );
    let result = crate::tools::registry::execute(state, task_id, tool, input).await;
    let duration = started.elapsed().as_millis() as u64;

    match result {
        Ok(value) => {
            let value_text = value.to_string();
            let cancelled = value
                .get("cancelled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let step_status = if cancelled { "cancelled" } else { "completed" };
            finish_step(state, step_id.as_deref(), step_status, Some(&value_text));
            state.emit_activity(
                task_id,
                "tool_completed",
                Some(tool),
                &format!("{tool} 完成（{duration}ms）"),
                Some(&truncate_text(&value_text, 2000)),
            );
            Ok(json!({ "status": "executed", "result": value, "durationMs": duration }))
        }
        Err(e) => {
            finish_step(state, step_id.as_deref(), "failed", Some(&e));
            state.emit_activity(
                task_id,
                "tool_failed",
                Some(tool),
                &format!("{tool} 失败: {e}"),
                None,
            );
            Ok(json!({ "status": "failed", "error": e, "durationMs": duration }))
        }
    }
}

/// 用户审批决定（设计文档 #31：Deny / Allow Once / Always）
pub fn resolve_approval(
    state: &AppState,
    request_id: &str,
    decision: &str,
    always_allow: bool,
) -> Result<Value, String> {
    if decision != "allow" && decision != "deny" {
        return Err(format!("未知审批决定: {decision}"));
    }
    let (request, save_rule) = {
        let mut map = state.pending_approvals.lock().unwrap();
        let Some(st) = map.get_mut(request_id) else {
            return Err(format!("审批请求 {request_id} 不存在或已处理"));
        };
        if st.decision.is_some() {
            return Err(format!("审批请求 {request_id} 已处理"));
        }
        st.decision = Some(decision.to_string());
        let request = st.request.clone();
        // 拒绝的请求不会再有重放，立即移除，避免审批表无限增长。
        if decision == "deny" {
            map.remove(request_id);
        }
        (request, decision == "allow" && always_allow)
    };
    if save_rule {
        // Always-allow 规则只对 MEDIUM 及以下生效（permission::decide 内部约束）
        let conn = state.db.lock().unwrap();
        permission::save_allow_rule(&conn, &request.tool, &request.input.to_string())?;
    }
    state.emit_approval_resolved(request_id, decision);
    Ok(json!({ "resolved": true, "decision": decision }))
}

/// 用户停止任务（设计文档 #40）：置取消标志 + 终止该任务启动的进程树
pub fn cancel_task(state: &AppState, task_id: &str) -> Result<Value, String> {
    state
        .cancelled_tasks
        .lock()
        .unwrap()
        .insert(task_id.to_string());

    // 清理该任务尚未处理的审批请求，并通知等待方，避免审批卡片残留 / 表泄漏
    let stale: Vec<String> = {
        let mut map = state.pending_approvals.lock().unwrap();
        let ids: Vec<String> = map
            .iter()
            .filter(|(_, st)| st.request.task_id.as_deref() == Some(task_id))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &ids {
            map.remove(id);
        }
        ids
    };
    for id in stale {
        state.emit_approval_resolved(&id, "deny");
    }

    let pids = state
        .running_pids
        .lock()
        .unwrap()
        .remove(task_id)
        .unwrap_or_default();
    let mut killed = 0;
    for pid in pids {
        kill_tree(pid);
        killed += 1;
    }
    state.emit_activity(
        Some(task_id),
        "task_cancel_requested",
        None,
        if killed > 0 {
            format!("用户请求停止任务（终止 {killed} 个进程）")
        } else {
            "用户请求停止任务".to_string()
        }
        .as_str(),
        None,
    );
    Ok(json!({ "cancelled": true, "taskId": task_id, "killedProcesses": killed }))
}

fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        crate::windows::process::kill_tree(pid);
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations;
    use crate::state::{AppState, ApprovalState, PendingApproval};

    fn setup() -> AppState {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        AppState::new(conn, std::env::temp_dir())
    }

    fn insert_pending(state: &AppState, request_id: &str, task_id: Option<&str>) {
        let request = PendingApproval {
            request_id: request_id.to_string(),
            task_id: task_id.map(String::from),
            tool: "filesystem.write".into(),
            input: json!({ "path": "D:/x.txt", "content": "x" }),
            risk_level: "medium".into(),
            summary: "写文件".into(),
        };
        state.pending_approvals.lock().unwrap().insert(
            request_id.to_string(),
            ApprovalState {
                request,
                decision: None,
            },
        );
    }

    #[test]
    fn deny_removes_pending_request() {
        let state = setup();
        insert_pending(&state, "r1", Some("t1"));
        resolve_approval(&state, "r1", "deny", false).unwrap();
        assert!(!state.pending_approvals.lock().unwrap().contains_key("r1"));
    }

    #[test]
    fn resolving_twice_is_rejected() {
        let state = setup();
        insert_pending(&state, "r2", Some("t1"));
        resolve_approval(&state, "r2", "allow", false).unwrap();
        assert!(resolve_approval(&state, "r2", "deny", false).is_err());
    }

    #[test]
    fn cancel_clears_task_pending_approvals_only() {
        let state = setup();
        insert_pending(&state, "r3", Some("task-a"));
        insert_pending(&state, "r4", Some("task-b"));
        cancel_task(&state, "task-a").unwrap();
        let map = state.pending_approvals.lock().unwrap();
        assert!(!map.contains_key("r3"), "被取消任务的审批应清理");
        assert!(map.contains_key("r4"), "其他任务的审批应保留");
    }

    #[test]
    fn finish_task_cleans_runtime_state() {
        let state = setup();
        state
            .cancelled_tasks
            .lock()
            .unwrap()
            .insert("task-a".into());
        state
            .running_pids
            .lock()
            .unwrap()
            .insert("task-a".into(), vec![1234]);
        insert_pending(&state, "r5", Some("task-a"));
        insert_pending(&state, "r6", Some("task-b"));

        state.finish_task("task-a");

        assert!(!state.cancelled_tasks.lock().unwrap().contains("task-a"));
        assert!(!state.running_pids.lock().unwrap().contains_key("task-a"));
        let map = state.pending_approvals.lock().unwrap();
        assert!(!map.contains_key("r5"));
        assert!(map.contains_key("r6"), "其他任务不受影响");
    }
}
