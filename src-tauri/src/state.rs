//! 应用全局状态（Agent / Tools / Permission / Task 共享）

use crate::database::models::AppSettings;
use crate::security::risk::RiskLevel;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

/// 待审批请求（设计文档 #31 Approval UI）
#[derive(Debug, Clone, Serialize)]
pub struct PendingApproval {
    pub request_id: String,
    pub task_id: Option<String>,
    pub tool: String,
    pub input: serde_json::Value,
    pub risk_level: String,
    pub summary: String,
}

/// 审批请求状态：request 展示给用户，decision 记录用户决定
#[derive(Debug, Clone, Serialize)]
pub struct ApprovalState {
    pub request: PendingApproval,
    /// None = 待决定；"allow" / "deny"
    pub decision: Option<String>,
}

/// 浏览器会话（browser.* 工具复用同一个 Chrome/Edge 实例）
pub struct BrowserSession {
    #[allow(dead_code)] // 保留 browser 句柄用于后续多标签页扩展
    pub browser: chromiumoxide::Browser,
    pub page: chromiumoxide::Page,
}

pub struct AppState {
    pub db: Mutex<Connection>,
    pub settings: Mutex<AppSettings>,
    pub pending_approvals: Mutex<HashMap<String, ApprovalState>>,
    /// task_id -> 该任务启动的进程 PID（Stop 时终止，设计文档 #40）
    pub running_pids: Mutex<HashMap<String, Vec<u32>>>,
    pub cancelled_tasks: Mutex<HashSet<String>>,
    pub browser: Mutex<Option<BrowserSession>>,
    pub app_handle: Mutex<Option<AppHandle>>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(db: Connection, data_dir: PathBuf) -> Self {
        let settings = crate::database::models::load_settings(&db).unwrap_or_default();
        Self {
            db: Mutex::new(db),
            settings: Mutex::new(settings),
            pending_approvals: Mutex::new(HashMap::new()),
            running_pids: Mutex::new(HashMap::new()),
            cancelled_tasks: Mutex::new(HashSet::new()),
            browser: Mutex::new(None),
            app_handle: Mutex::new(None),
            data_dir,
        }
    }

    pub fn settings_snapshot(&self) -> AppSettings {
        self.settings.lock().unwrap().clone()
    }

    pub fn is_cancelled(&self, task_id: &str) -> bool {
        self.cancelled_tasks.lock().unwrap().contains(task_id)
    }

    /// 记录并广播 Activity 事件（设计文档 #13：用户必须能够知道 AI 当前在做什么）
    pub fn emit_activity(
        &self,
        task_id: Option<&str>,
        event_type: &str,
        tool: Option<&str>,
        summary: &str,
        detail: Option<&str>,
    ) {
        // 持久化到 events 表
        {
            let conn = self.db.lock().unwrap();
            let _ = crate::database::models::append_event(&conn, task_id, event_type, detail);
        }
        let payload = serde_json::json!({
            "taskId": task_id,
            "time": crate::database::now_ms(),
            "type": event_type,
            "tool": tool,
            "summary": summary,
            "detail": detail,
        });
        if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
            let _ = handle.emit("activity", payload);
        }
    }

    /// 广播任务状态变更
    pub fn emit_task_updated(&self, task: &crate::database::models::Task) {
        if let Ok(payload) = serde_json::to_value(task) {
            if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
                let _ = handle.emit("task-updated", payload);
            }
        }
    }

    /// 广播审批请求
    pub fn emit_approval_requested(&self, request: &PendingApproval) {
        if let Ok(payload) = serde_json::to_value(request) {
            if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
                let _ = handle.emit("approval-requested", payload);
            }
        }
    }

    /// 广播审批结果
    pub fn emit_approval_resolved(&self, request_id: &str, decision: &str) {
        if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
            let _ = handle.emit(
                "approval-resolved",
                serde_json::json!({ "requestId": request_id, "decision": decision }),
            );
        }
    }
}

/// 工具风险等级注册表（与设计文档 #16/#30 对应）
pub fn tool_risk(tool: &str) -> RiskLevel {
    match tool {
        // SAFE：读取类
        "filesystem.list" | "filesystem.read" | "filesystem.search" | "filesystem.stat"
        | "process.list" | "process.inspect"
        | "window.list" | "window.find"
        | "ui.inspect" | "ui.find" | "ui.getValue" | "ui.getText"
        | "browser.goto" | "browser.back" | "browser.forward" | "browser.search"
        | "browser.scroll" | "browser.extract" | "browser.screenshot"
        | "screenshot.full" | "screenshot.window" | "screenshot.region"
        | "clipboard.read" => RiskLevel::Safe,
        // LOW：创建 / 复制 / 重命名 / 打开应用
        "filesystem.mkdir" | "filesystem.copy" | "filesystem.rename"
        | "process.start"
        | "window.focus" | "window.minimize" | "window.maximize"
        | "browser.open"
        | "clipboard.write" => RiskLevel::Low,
        // MEDIUM：修改 / 操作类
        "filesystem.write" | "filesystem.move" | "window.close"
        | "ui.click" | "ui.type" | "ui.select"
        | "browser.click" | "browser.type" | "browser.download"
        | "shell.exec" => RiskLevel::Medium,
        // HIGH：删除 / 终止进程
        "filesystem.delete" | "process.kill" => RiskLevel::High,
        _ => RiskLevel::Medium,
    }
}
