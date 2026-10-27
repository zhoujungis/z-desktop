//! Tauri 命令层：前端与 Rust Core 的 IPC 入口

use crate::database::models::{
    self, AppSettings, EventRecord, Memory, Message, Project, Task, TaskStep,
};
use crate::state::AppState;
use crate::tools::registry::{tool_definitions, ToolDef};
use serde_json::Value;
use tauri::State;

// ---------- 工具 / Agent ----------

#[tauri::command]
pub fn list_tools() -> Vec<ToolDef> {
    tool_definitions()
}

#[tauri::command]
pub async fn tool_execute(
    state: State<'_, AppState>,
    task_id: Option<String>,
    tool: String,
    input: Value,
    approval_request_id: Option<String>,
) -> Result<Value, String> {
    crate::agent::request_tool_execution(
        state.inner(),
        task_id.as_deref(),
        &tool,
        input,
        approval_request_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn resolve_approval(
    state: State<'_, AppState>,
    request_id: String,
    decision: String,
    always_allow: Option<bool>,
) -> Result<Value, String> {
    crate::agent::resolve_approval(
        state.inner(),
        &request_id,
        &decision,
        always_allow.unwrap_or(false),
    )
}

#[tauri::command]
pub async fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<Value, String> {
    crate::agent::cancel_task(state.inner(), &task_id)
}

// ---------- 任务 ----------

#[tauri::command]
pub async fn create_task(state: State<'_, AppState>, title: String) -> Result<Task, String> {
    let task = {
        let conn = state.db.lock().unwrap();
        models::create_task(&conn, &title)?
    };
    state.emit_task_updated(&task);
    Ok(task)
}

#[tauri::command]
pub async fn get_task(state: State<'_, AppState>, id: String) -> Result<Option<Task>, String> {
    let conn = state.db.lock().unwrap();
    models::get_task(&conn, &id)
}

#[tauri::command]
pub async fn list_tasks(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<Task>, String> {
    let conn = state.db.lock().unwrap();
    models::list_tasks(&conn, limit.unwrap_or(50))
}

#[tauri::command]
pub async fn update_task_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> Result<Task, String> {
    {
        let conn = state.db.lock().unwrap();
        models::update_task_status(&conn, &id, &status)?;
    }
    let task = {
        let conn = state.db.lock().unwrap();
        models::get_task(&conn, &id)?.ok_or_else(|| format!("任务 {id} 不存在"))?
    };
    state.emit_task_updated(&task);
    Ok(task)
}

#[tauri::command]
pub async fn list_steps(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Vec<TaskStep>, String> {
    let conn = state.db.lock().unwrap();
    models::list_steps(&conn, &task_id)
}

#[tauri::command]
pub async fn list_events(
    state: State<'_, AppState>,
    task_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<EventRecord>, String> {
    let conn = state.db.lock().unwrap();
    models::list_events(&conn, task_id.as_deref(), limit.unwrap_or(200))
}

// ---------- 消息 / 记忆 ----------

#[tauri::command]
pub async fn save_message(
    state: State<'_, AppState>,
    task_id: Option<String>,
    role: String,
    content: String,
) -> Result<Message, String> {
    let conn = state.db.lock().unwrap();
    models::save_message(&conn, task_id.as_deref(), &role, &content)
}

#[tauri::command]
pub async fn list_messages(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Vec<Message>, String> {
    let conn = state.db.lock().unwrap();
    models::list_messages(&conn, &task_id)
}

#[tauri::command]
pub async fn add_memory(
    state: State<'_, AppState>,
    memory_type: String,
    content: String,
    source: Option<String>,
) -> Result<Memory, String> {
    let conn = state.db.lock().unwrap();
    models::add_memory(&conn, &memory_type, &content, source.as_deref())
}

#[tauri::command]
pub async fn search_memories(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<Memory>, String> {
    let conn = state.db.lock().unwrap();
    models::search_memories(&conn, &query, limit.unwrap_or(10))
}

// ---------- 项目 ----------

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    let conn = state.db.lock().unwrap();
    models::list_projects(&conn)
}

#[tauri::command]
pub async fn add_project(
    state: State<'_, AppState>,
    name: String,
    path: String,
    description: Option<String>,
    technology: Option<String>,
    git_url: Option<String>,
) -> Result<Project, String> {
    let conn = state.db.lock().unwrap();
    models::add_project(
        &conn,
        &name,
        &path,
        description.as_deref(),
        technology.as_deref(),
        git_url.as_deref(),
    )
}

#[tauri::command]
pub async fn set_active_project(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<(), String> {
    let conn = state.db.lock().unwrap();
    models::set_active_project(&conn, id.as_deref())
}

#[tauri::command]
pub async fn get_active_project(state: State<'_, AppState>) -> Result<Option<Project>, String> {
    let conn = state.db.lock().unwrap();
    models::get_active_project(&conn)
}

// ---------- 设置 ----------

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings_snapshot())
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    {
        let conn = state.db.lock().unwrap();
        models::save_settings(&conn, &settings)?;
    }
    *state.settings.lock().unwrap() = settings;
    Ok(())
}
