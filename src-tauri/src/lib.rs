//! Z Desktop 库入口：Tauri Builder 装配（状态、插件、命令）

pub mod agent;
pub mod commands;
pub mod database;
pub mod security;
pub mod state;
pub mod tools;
pub mod windows;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir).ok();
            let conn = database::init_db(&data_dir)?;
            let app_state = state::AppState::new(conn, data_dir);
            *app_state.app_handle.lock().unwrap() = Some(app.handle().clone());
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 工具 / Agent
            commands::list_tools,
            commands::tool_execute,
            commands::resolve_approval,
            commands::cancel_task,
            // 任务
            commands::create_task,
            commands::get_task,
            commands::list_tasks,
            commands::update_task_status,
            commands::list_steps,
            commands::list_events,
            // 消息 / 记忆
            commands::save_message,
            commands::list_messages,
            commands::add_memory,
            commands::search_memories,
            // 项目
            commands::list_projects,
            commands::add_project,
            commands::set_active_project,
            commands::get_active_project,
            // 设置
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("Z Desktop 运行出错");
}
