//! 端到端集成测试：走与前端相同的调用路径（agent::request_tool_execution → registry::execute）
//!
//! 覆盖范围：
//! - 工具注册表完整性（44 个工具定义）
//! - 文件系统工具全链路（真实临时目录读写）
//! - Shell 真实执行（输出 / 退出码 / 超时 / 取消 / 工作目录）
//! - 进程查看
//! - 安全层：CRITICAL 拦截、审批流（allow once / deny / 参数校验）
//! - 数据库：任务/步骤/事件/记忆 FTS5 检索
//! - Windows 专属工具在非 Windows 平台的友好降级
//!
//! 注：FTS5 unicode61 分词器对连续中文按整句成词，检索测试使用含空格分词的文本。

use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use z_desktop_lib::agent;
use z_desktop_lib::database::migrations;
use z_desktop_lib::database::models as m;
use z_desktop_lib::security::sandbox::{check_path, PathOp};
use z_desktop_lib::state::AppState;
use z_desktop_lib::tools::registry;

// ---------- 测试环境 ----------

fn setup() -> (Arc<AppState>, TempDir) {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrations::run(&conn).unwrap();
    let dir = tempfile::tempdir().unwrap();
    (Arc::new(AppState::new(conn, dir.path().to_path_buf())), dir)
}

async fn run_tool(state: &AppState, tool: &str, input: Value) -> Result<Value, String> {
    registry::execute(state, None, tool, input).await
}

async fn run_agent_tool(
    state: &AppState,
    task_id: Option<&str>,
    tool: &str,
    input: Value,
) -> Result<Value, String> {
    agent::request_tool_execution(state, task_id, tool, input, None).await
}

// ---------- 1. 工具注册表 ----------

#[test]
fn tool_definitions_complete_and_valid() {
    let defs = registry::tool_definitions();
    assert_eq!(defs.len(), 44, "工具数量应为 44");

    let mut names: Vec<&str> = defs.iter().map(|d| d.name).collect();
    names.sort();
    let before_dedup = names.len();
    names.dedup();
    assert_eq!(names.len(), before_dedup, "工具名不应重复");

    for d in &defs {
        assert!(!d.description.trim().is_empty(), "{} 缺少描述", d.name);
        assert!(d.input_schema.is_object(), "{} 的 input_schema 应为对象", d.name);
        assert!(
            ["safe", "low", "medium", "high", "critical"].contains(&d.risk_level),
            "{} 风险等级非法: {}",
            d.name,
            d.risk_level
        );
    }

    // 9 大能力类别均有代表工具（设计文档 P0 18 项能力）
    let expected = [
        "filesystem.list",
        "filesystem.read",
        "filesystem.write",
        "filesystem.search",
        "filesystem.copy",
        "filesystem.move",
        "filesystem.rename",
        "filesystem.mkdir",
        "filesystem.delete",
        "filesystem.stat",
        "shell.exec",
        "process.list",
        "process.inspect",
        "process.start",
        "process.kill",
        "window.list",
        "window.focus",
        "window.close",
        "ui.inspect",
        "ui.find",
        "ui.click",
        "ui.type",
        "ui.select",
        "ui.getValue",
        "ui.getText",
        "browser.open",
        "browser.goto",
        "browser.search",
        "browser.click",
        "browser.type",
        "browser.extract",
        "screenshot.full",
        "screenshot.window",
        "screenshot.region",
        "clipboard.read",
        "clipboard.write",
    ];
    for name in expected {
        assert!(
            defs.iter().any(|d| d.name == name),
            "缺少关键工具: {name}"
        );
    }
}

// ---------- 2. 文件系统全链路 ----------

#[tokio::test]
async fn filesystem_full_lifecycle() {
    let (state, dir) = setup();
    let base = dir.path().to_string_lossy().to_string();

    // mkdir + write（含自动创建父目录）
    run_tool(&state, "filesystem.mkdir", json!({ "path": format!("{base}/docs") }))
        .await
        .expect("mkdir 失败");
    let nested = format!("{base}/docs/nested/report.txt");
    run_tool(
        &state,
        "filesystem.write",
        json!({ "path": nested, "content": "Z Desktop 集成测试内容 hello" }),
    )
    .await
    .expect("write 失败");

    // read 内容一致
    let read = run_tool(&state, "filesystem.read", json!({ "path": nested }))
        .await
        .expect("read 失败");
    assert_eq!(read["content"].as_str().unwrap(), "Z Desktop 集成测试内容 hello");

    // stat
    let stat = run_tool(&state, "filesystem.stat", json!({ "path": nested }))
        .await
        .expect("stat 失败");
    assert_eq!(stat["isDir"].as_bool(), Some(false), "report.txt 应为文件");
    assert_eq!(stat["exists"].as_bool(), Some(true));

    // search 命中
    let search = run_tool(
        &state,
        "filesystem.search",
        json!({ "path": base, "pattern": "*.txt" }),
    )
    .await
    .expect("search 失败");
    let arr = search["matches"].as_array().expect("search 应返回 matches 列表");
    assert!(
        arr.iter().any(|v| v.to_string().contains("report.txt")),
        "*.txt 应命中 report.txt: {search}"
    );
    assert_eq!(search["count"].as_u64(), Some(arr.len() as u64));

    // copy → copy 目标可读
    let copied = format!("{base}/copy-report.txt");
    run_tool(
        &state,
        "filesystem.copy",
        json!({ "from": nested, "to": copied }),
    )
    .await
    .expect("copy 失败");
    let r2 = run_tool(&state, "filesystem.read", json!({ "path": copied }))
        .await
        .expect("读取副本失败");
    assert_eq!(r2["content"].as_str().unwrap(), "Z Desktop 集成测试内容 hello");

    // rename
    run_tool(
        &state,
        "filesystem.rename",
        json!({ "path": copied, "new_name": "renamed.txt" }),
    )
    .await
    .expect("rename 失败");
    let renamed = format!("{base}/renamed.txt");
    assert!(run_tool(&state, "filesystem.stat", json!({ "path": renamed })).await.is_ok());

    // move
    let moved = format!("{base}/docs/moved.txt");
    run_tool(
        &state,
        "filesystem.move",
        json!({ "from": renamed, "to": moved }),
    )
    .await
    .expect("move 失败");
    assert!(run_tool(&state, "filesystem.stat", json!({ "path": moved })).await.is_ok());
    assert!(run_tool(&state, "filesystem.stat", json!({ "path": renamed })).await.is_err());

    // list 包含目录内容
    let list = run_tool(&state, "filesystem.list", json!({ "path": base, "depth": 3 }))
        .await
        .expect("list 失败");
    let list_str = list.to_string();
    assert!(list_str.contains("docs"), "list 应包含 docs 目录");

    // delete → 之后不可读
    run_tool(&state, "filesystem.delete", json!({ "path": moved }))
        .await
        .expect("delete 失败");
    assert!(run_tool(&state, "filesystem.read", json!({ "path": moved })).await.is_err());
}

#[tokio::test]
async fn filesystem_rejects_protected_paths() {
    let (state, _dir) = setup();
    // Linux 保护目录
    let err = run_tool(
        &state,
        "filesystem.write",
        json!({ "path": "/usr/share/evil.txt", "content": "x" }),
    )
    .await;
    assert!(err.is_err(), "写 /usr 应被沙箱拒绝");
    // Windows 保护路径（跨平台判定）
    let err2 = run_tool(
        &state,
        "filesystem.write",
        json!({ "path": "C:\\Windows\\evil.txt", "content": "x" }),
    )
    .await;
    assert!(err2.is_err(), "写 C:\\Windows 应被沙箱拒绝");
    // 读系统目录允许（Read 不限制保护目录）
    assert!(check_path("/etc/hostname", PathOp::Read).is_ok());
}

// ---------- 3. Shell 真实执行 ----------

#[tokio::test]
async fn shell_exec_basic_output_and_exit_code() {
    let (state, _dir) = setup();
    let out = run_tool(&state, "shell.exec", json!({ "command": "echo z-desktop-ok" }))
        .await
        .expect("shell.exec 失败");
    assert_eq!(out["exitCode"].as_i64(), Some(0));
    assert!(out["stdout"].as_str().unwrap().contains("z-desktop-ok"));
    assert_eq!(out["timedOut"].as_bool(), Some(false));
    assert_eq!(out["cancelled"].as_bool(), Some(false));
    assert!(out["pid"].as_u64().is_some());
}

#[tokio::test]
async fn shell_exec_reports_nonzero_exit() {
    let (state, _dir) = setup();
    let out = run_tool(
        &state,
        "shell.exec",
        json!({ "command": "definitely-not-exist-cmd-xyz 2>/dev/null; exit 7" }),
    )
    .await
    .expect("shell.exec 失败");
    assert_eq!(out["exitCode"].as_i64(), Some(7));
    assert!(out["stderr"].as_str().unwrap().len() < 100_000);
}

#[tokio::test]
async fn shell_exec_timeout_kills_process() {
    let (state, _dir) = setup();
    let out = run_tool(
        &state,
        "shell.exec",
        json!({ "command": "sleep 5", "timeout": 500 }),
    )
    .await
    .expect("shell.exec 失败");
    assert_eq!(out["timedOut"].as_bool(), Some(true), "应标记超时");
    assert!(out["duration"].as_u64().unwrap() < 4_000, "应在超时点终止而非等满 5s");
}

#[tokio::test]
async fn shell_exec_user_cancel_takes_effect() {
    let (state, _dir) = setup();
    // 模拟用户点击 Stop（设计文档 #40）：先置取消标志，再以该 task_id 发起命令
    state.cancelled_tasks.lock().unwrap().insert("task-cancel".into());
    let out = registry::execute(&state, Some("task-cancel"), "shell.exec", json!({ "command": "sleep 30" }))
        .await
        .expect("shell.exec 失败");
    // poll_cancel 置位取消标志后 wait_loop 终止进程
    assert_eq!(out["cancelled"].as_bool(), Some(true), "取消后应返回 cancelled=true");
    assert!(out["duration"].as_u64().unwrap() < 10_000, "取消应远早于命令自然结束");
}

#[tokio::test]
async fn shell_exec_respects_cwd() {
    let (state, dir) = setup();
    let cwd = dir.path().to_string_lossy().to_string();
    let out = run_tool(
        &state,
        "shell.exec",
        json!({ "command": "pwd", "cwd": cwd }),
    )
    .await
    .expect("shell.exec 失败");
    assert_eq!(out["exitCode"].as_i64(), Some(0));
    assert!(
        out["stdout"].as_str().unwrap().contains(dir.path().file_name().unwrap().to_str().unwrap()),
        "pwd 输出应位于临时目录内: {}",
        out["stdout"]
    );
}

#[tokio::test]
async fn shell_exec_empty_command_rejected() {
    let (state, _dir) = setup();
    assert!(run_tool(&state, "shell.exec", json!({ "command": "   " })).await.is_err());
    assert!(run_tool(&state, "shell.exec", json!({})).await.is_err());
}

// ---------- 4. 进程 ----------

#[tokio::test]
async fn process_list_returns_processes() {
    let (state, _dir) = setup();
    let out = run_tool(&state, "process.list", json!({ "limit": 50 }))
        .await
        .expect("process.list 失败");
    let procs = out["processes"].as_array().expect("应有 processes 数组");
    assert!(!procs.is_empty(), "系统中至少有一个进程");
    let first = &procs[0];
    assert!(first["pid"].as_u64().is_some(), "进程应有 pid");
    assert!(first["name"].as_str().is_some(), "进程应有 name");
}

// ---------- 5. Windows 专属工具降级（非 Windows 断言，Windows 上跑真实逻辑由手测覆盖） ----------

#[cfg(not(windows))]
#[tokio::test]
async fn windows_only_tools_degrade_gracefully() {
    let (state, _dir) = setup();
    for tool in ["window.list", "ui.inspect", "ui.click"] {
        let err = run_tool(&state, tool, json!({})).await;
        assert!(err.is_err(), "{tool} 在非 Windows 平台应返回错误提示");
        assert!(
            err.unwrap_err().contains("Windows"),
            "{tool} 的错误信息应说明仅支持 Windows"
        );
    }
}

// ---------- 6. 安全层与审批流 ----------

#[tokio::test]
async fn critical_commands_are_blocked() {
    let (state, _dir) = setup();
    let out = run_agent_tool(&state, Some("t1"), "shell.exec", json!({ "command": "format c:" }))
        .await
        .expect("request_tool_execution 不应 Err");
    assert_eq!(out["status"].as_str(), Some("blocked"), "CRITICAL 命令应默认禁止");
    assert_eq!(out["riskLevel"].as_str(), Some("critical"));

    let out2 = run_agent_tool(&state, Some("t1"), "shell.exec", json!({ "command": "shutdown /s /t 0" }))
        .await
        .expect("request_tool_execution 不应 Err");
    assert_eq!(out2["status"].as_str(), Some("blocked"));
}

#[tokio::test]
async fn safe_tool_auto_executes_without_approval() {
    let (state, dir) = setup();
    let p = dir.path().join("a.txt");
    std::fs::write(&p, "hi").unwrap();
    let out = run_agent_tool(
        &state,
        Some("t2"),
        "filesystem.read",
        json!({ "path": p.to_string_lossy() }),
    )
    .await
    .expect("不应 Err");
    assert_eq!(out["status"].as_str(), Some("executed"), "SAFE 工具默认自动执行");
    assert!(out["result"].is_object() || out["result"].is_string());
}

#[tokio::test]
async fn medium_tool_requires_approval_then_allow_once() {
    let (state, dir) = setup();
    // 默认设置：auto_approve_medium = false → 需审批
    let out = run_agent_tool(
        &state,
        Some("t3"),
        "filesystem.write",
        json!({ "path": dir.path().join("appr.txt"), "content": "x" }),
    )
    .await
    .expect("不应 Err");
    assert_eq!(out["status"].as_str(), Some("approval_required"), "MEDIUM 默认需审批");
    let rid = out["requestId"].as_str().expect("应返回 requestId").to_string();

    // 批准前 pending_approvals 中存在该请求
    assert!(state.pending_approvals.lock().unwrap().contains_key(&rid));

    // Allow Once
    let resolved = agent::resolve_approval(&state, &rid, "allow", false).expect("审批应成功");
    assert_eq!(resolved["decision"].as_str(), Some("allow"));

    // 带同一 requestId + 同一参数重放 → 消费后执行
    let out2 = agent::request_tool_execution(
        &state,
        Some("t3"),
        "filesystem.write",
        json!({ "path": dir.path().join("appr.txt"), "content": "x" }),
        Some(&rid),
    )
    .await
    .expect("不应 Err");
    assert_eq!(out2["status"].as_str(), Some("executed"), "批准后应执行");
    assert!(!state.pending_approvals.lock().unwrap().contains_key(&rid), "一次性审批应被消费");
    assert!(dir.path().join("appr.txt").exists(), "文件应已写入");
}

#[tokio::test]
async fn approval_replay_with_changed_input_is_rejected() {
    let (state, dir) = setup();
    let input_a = json!({ "path": dir.path().join("a.txt"), "content": "A" });
    let out = run_agent_tool(&state, Some("t4"), "filesystem.write", input_a)
        .await
        .expect("不应 Err");
    let rid = out["requestId"].as_str().unwrap().to_string();
    agent::resolve_approval(&state, &rid, "allow", false).unwrap();

    // 用批准 A 的 requestId 执行 B（参数已变化）→ 拒绝（防"批准 A 执行 B"）
    let input_b = json!({ "path": dir.path().join("b.txt"), "content": "B" });
    let result = agent::request_tool_execution(&state, Some("t4"), "filesystem.write", input_b, Some(&rid)).await;
    assert!(result.is_err(), "参数变化的重放应被拒绝");
    assert!(!dir.path().join("b.txt").exists());
}

#[tokio::test]
async fn deny_prevents_execution() {
    let (state, dir) = setup();
    let input = json!({ "path": dir.path().join("no.txt"), "content": "x" });
    let out = run_agent_tool(&state, Some("t5"), "filesystem.write", input.clone())
        .await
        .expect("不应 Err");
    let rid = out["requestId"].as_str().unwrap().to_string();
    agent::resolve_approval(&state, &rid, "deny", false).unwrap();
    // deny 后重放：审批请求无效（决策不是 allow）
    let result = agent::request_tool_execution(&state, Some("t5"), "filesystem.write", input, Some(&rid)).await;
    assert!(result.is_err(), "deny 后的重放应失败");
    assert!(!dir.path().join("no.txt").exists(), "deny 后不应执行");
}

#[tokio::test]
async fn always_allow_persists_rule_and_skips_approval() {
    let (state, dir) = setup();
    let path = dir.path().join("always.txt");
    // Always-allow 为精确匹配（tool + 完整 input JSON），两次调用必须参数完全一致
    let input = json!({ "path": path, "content": "v1" });
    let out = run_agent_tool(&state, Some("t6"), "filesystem.write", input.clone())
        .await
        .expect("不应 Err");
    assert_eq!(out["status"].as_str(), Some("approval_required"));
    let rid = out["requestId"].as_str().unwrap().to_string();
    agent::resolve_approval(&state, &rid, "allow", true).expect("Always 审批应成功");

    // 同参数再次发起 → 命中 Always-allow，直接执行
    let out2 = run_agent_tool(&state, Some("t6"), "filesystem.write", input)
        .await
        .expect("不应 Err");
    assert_eq!(out2["status"].as_str(), Some("executed"), "命中 Always-allow 应直接执行");
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "v1");

    // 规则精确匹配：参数不同 → 仍需审批
    let out3 = run_agent_tool(
        &state,
        Some("t6"),
        "filesystem.write",
        json!({ "path": dir.path().join("other.txt"), "content": "x" }),
    )
    .await
    .expect("不应 Err");
    assert_eq!(out3["status"].as_str(), Some("approval_required"), "参数不同不应命中规则");
}

#[tokio::test]
async fn cancel_task_blocks_subsequent_steps() {
    let (state, _dir) = setup();
    agent::cancel_task(&state, "t7").expect("cancel_task 应成功");
    let out = run_agent_tool(&state, Some("t7"), "filesystem.read", json!({ "path": "/tmp" }))
        .await
        .expect("不应 Err");
    assert_eq!(out["status"].as_str(), Some("cancelled"), "已取消任务的后续步骤应立即返回 cancelled");
}

// ---------- 7. 数据库层 ----------

#[test]
fn database_task_step_event_crud_and_fts() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrations::run(&conn).unwrap();

    // 任务 + 步骤 + 事件
    let task = m::create_task(&conn, "整理下载目录").unwrap();
    assert_eq!(task.status, "pending");
    m::update_task_status(&conn, &task.id, "running").unwrap();
    let got = m::get_task(&conn, &task.id).unwrap().unwrap();
    assert_eq!(got.status, "running");

    m::append_step(&conn, &task.id, Some("filesystem.list"), Some("{}"), Some("ok"), "completed").unwrap();
    m::append_step(&conn, &task.id, Some("filesystem.move"), Some("{}"), Some("ok"), "completed").unwrap();
    let steps = m::list_steps(&conn, &task.id).unwrap();
    assert_eq!(steps.len(), 2, "应有两条步骤记录");
    assert_eq!(steps[0].step_index, 0, "step_index 应自动递增");
    assert_eq!(steps[1].step_index, 1);

    m::append_event(&conn, Some(&task.id), "tool_completed", Some("r1")).unwrap();
    m::append_event(&conn, None, "app_started", None).unwrap();
    assert_eq!(m::list_events(&conn, Some(&task.id), 100).unwrap().len(), 1);
    assert_eq!(m::list_events(&conn, None, 100).unwrap().len(), 2);

    // 消息
    m::save_message(&conn, Some(&task.id), "user", "帮我整理文件").unwrap();
    m::save_message(&conn, Some(&task.id), "assistant", "好的，开始执行").unwrap();
    assert_eq!(m::list_messages(&conn, &task.id).unwrap().len(), 2);

    // 项目
    let proj = m::add_project(&conn, "Z Desktop", "/workspace/z-desktop", None, Some("Rust"), None).unwrap();
    m::set_active_project(&conn, Some(&proj.id)).unwrap();
    assert_eq!(m::get_active_project(&conn).unwrap().unwrap().id, proj.id);

    // 记忆 + FTS5 检索（unicode61 按空格分词，中文以空格分词写入）
    m::add_memory(&conn, "preference", "用户 偏好 深色 主题", None).unwrap();
    m::add_memory(&conn, "fact", "项目 使用 vite 构建 前端", None).unwrap();
    let hits = m::search_memories(&conn, "深色", 10).unwrap();
    assert_eq!(hits.len(), 1, "FTS5 应命中 '深色'");
    assert!(hits[0].content.contains("深色"));
    let hits2 = m::search_memories(&conn, "vite", 10).unwrap();
    assert_eq!(hits2.len(), 1, "FTS5 应命中 'vite'");
    let miss = m::search_memories(&conn, "不存在的词xyz", 10).unwrap();
    assert!(miss.is_empty());

    // 设置持久化
    let mut s = m::load_settings(&conn).unwrap();
    s.provider = "anthropic".into();
    s.auto_approve_low = true;
    m::save_settings(&conn, &s).unwrap();
    let s2 = m::load_settings(&conn).unwrap();
    assert_eq!(s2.provider, "anthropic");
    assert!(s2.auto_approve_low);

    // 任务终态时间
    m::update_task_status(&conn, &task.id, "completed").unwrap();
    let done = m::get_task(&conn, &task.id).unwrap().unwrap();
    assert!(done.completed_at.is_some(), "completed 应记录 completed_at");
}

#[test]
fn database_fts_stays_in_sync_after_update() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrations::run(&conn).unwrap();
    let mem = m::add_memory(&conn, "note", "关键词 alpha beta", None).unwrap();
    assert_eq!(m::search_memories(&conn, "alpha", 10).unwrap().len(), 1);
    // 更新后旧词应消失（触发器同步）
    conn.execute(
        "UPDATE memories SET content = ?1 WHERE id = ?2",
        rusqlite::params!["关键词 gamma delta", mem.id],
    )
    .unwrap();
    assert!(m::search_memories(&conn, "alpha", 10).unwrap().is_empty(), "旧内容应从 FTS 索引移除");
    assert_eq!(m::search_memories(&conn, "gamma", 10).unwrap().len(), 1, "新内容应进入 FTS 索引");
}

// ---------- 8. 沙箱纯函数 ----------

#[test]
fn sandbox_path_normalization_and_protection() {
    // `..` 逃逸必须被规范化后识别
    assert!(check_path("/etc/../etc/evil", PathOp::Mutate).is_err());
    assert!(check_path("/tmp/normal/file.txt", PathOp::Mutate).is_ok());
    assert!(check_path("", PathOp::Read).is_err(), "空路径应拒绝");
    // 大小写不敏感（Windows 风格）
    assert!(check_path("c:\\windows\\system32\\x", PathOp::Mutate).is_err());
}
