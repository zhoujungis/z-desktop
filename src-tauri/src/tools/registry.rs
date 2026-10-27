//! 工具注册表（设计文档 #16 Tool Registry）
//!
//! 所有工具统一注册，前端 Agent Loop 通过 `list_tools` 拉取定义，
//! 通过 `tool_execute` 执行（权限校验在 Rust 侧强制进行）。

use crate::state::AppState;
use serde::Serialize;

/// 工具定义（与设计文档 #16 ToolDefinition 对应）
#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: serde_json::Value,
    #[serde(rename = "riskLevel")]
    pub risk_level: &'static str,
}

/// 工具入参读取辅助
pub fn param_str<'a>(input: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(|v| v.as_str())
}

pub fn param_u64(input: &serde_json::Value, key: &str) -> Option<u64> {
    input.get(key).and_then(|v| v.as_u64())
}

pub fn param_i64(input: &serde_json::Value, key: &str) -> Option<i64> {
    input.get(key).and_then(|v| v.as_i64())
}

pub fn param_bool(input: &serde_json::Value, key: &str) -> Option<bool> {
    input.get(key).and_then(|v| v.as_bool())
}

pub fn param_f64(input: &serde_json::Value, key: &str) -> Option<f64> {
    input.get(key).and_then(|v| v.as_f64())
}

fn schema(properties: serde_json::Value, required: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

fn s(desc: &str) -> serde_json::Value {
    serde_json::json!({ "type": "string", "description": desc })
}

fn b(desc: &str) -> serde_json::Value {
    serde_json::json!({ "type": "boolean", "description": desc })
}

fn n(desc: &str) -> serde_json::Value {
    serde_json::json!({ "type": "number", "description": desc })
}

/// 全部工具定义（设计文档 #17-#26）
pub fn tool_definitions() -> Vec<ToolDef> {
    use crate::security::risk::RiskLevel;
    let (safe, low, medium, high) = (
        RiskLevel::Safe.as_str(),
        RiskLevel::Low.as_str(),
        RiskLevel::Medium.as_str(),
        RiskLevel::High.as_str(),
    );
    vec![
        // ---------- filesystem.* ----------
        ToolDef {
            name: "filesystem.list",
            description: "列出目录内容",
            input_schema: schema(
                serde_json::json!({ "path": s("目录绝对路径"), "depth": n("递归深度，默认 1") }),
                &["path"],
            ),
            risk_level: safe,
        },
        ToolDef {
            name: "filesystem.read",
            description: "读取文件内容（文本或 base64）",
            input_schema: schema(
                serde_json::json!({ "path": s("文件绝对路径"), "max_bytes": n("最大读取字节数，默认 2MB") }),
                &["path"],
            ),
            risk_level: safe,
        },
        ToolDef {
            name: "filesystem.write",
            description: "写入文件（自动创建父目录）",
            input_schema: schema(
                serde_json::json!({ "path": s("文件绝对路径"), "content": s("写入内容") }),
                &["path", "content"],
            ),
            risk_level: medium,
        },
        ToolDef {
            name: "filesystem.search",
            description: "按文件名通配符递归搜索文件，如 *.pdf",
            input_schema: schema(
                serde_json::json!({ "path": s("搜索根目录"), "pattern": s("通配符，如 *.pdf"), "max_results": n("最大结果数，默认 200") }),
                &["path", "pattern"],
            ),
            risk_level: safe,
        },
        ToolDef {
            name: "filesystem.copy",
            description: "复制文件或目录",
            input_schema: schema(
                serde_json::json!({ "from": s("源路径"), "to": s("目标路径"), "overwrite": b("是否覆盖，默认 false") }),
                &["from", "to"],
            ),
            risk_level: low,
        },
        ToolDef {
            name: "filesystem.move",
            description: "移动文件或目录",
            input_schema: schema(
                serde_json::json!({ "from": s("源路径"), "to": s("目标路径"), "overwrite": b("是否覆盖，默认 false") }),
                &["from", "to"],
            ),
            risk_level: medium,
        },
        ToolDef {
            name: "filesystem.rename",
            description: "重命名文件或目录",
            input_schema: schema(
                serde_json::json!({ "path": s("原路径"), "new_name": s("新名称（仅文件名）") }),
                &["path", "new_name"],
            ),
            risk_level: low,
        },
        ToolDef {
            name: "filesystem.mkdir",
            description: "创建目录（递归）",
            input_schema: schema(serde_json::json!({ "path": s("目录路径") }), &["path"]),
            risk_level: low,
        },
        ToolDef {
            name: "filesystem.delete",
            description: "删除文件或目录（高风险）",
            input_schema: schema(
                serde_json::json!({ "path": s("目标路径"), "recursive": b("目录是否递归删除") }),
                &["path"],
            ),
            risk_level: high,
        },
        ToolDef {
            name: "filesystem.stat",
            description: "查看文件/目录元信息",
            input_schema: schema(serde_json::json!({ "path": s("路径") }), &["path"]),
            risk_level: safe,
        },
        // ---------- shell.* ----------
        ToolDef {
            name: "shell.exec",
            description:
                "执行 Shell 命令（PowerShell/CMD/WSL/Git Bash），返回 exitCode/stdout/stderr",
            input_schema: schema(
                serde_json::json!({ "command": s("命令"), "cwd": s("工作目录"), "timeout": n("超时毫秒，默认 120000"), "shell": s("powershell | cmd | wsl | gitbash，默认 powershell") }),
                &["command"],
            ),
            risk_level: medium,
        },
        // ---------- process.* ----------
        ToolDef {
            name: "process.list",
            description: "查看系统进程",
            input_schema: schema(
                serde_json::json!({ "filter": s("按名称过滤"), "limit": n("最大数量，默认 100") }),
                &[],
            ),
            risk_level: safe,
        },
        ToolDef {
            name: "process.start",
            description: "启动应用/进程",
            input_schema: schema(
                serde_json::json!({ "command": s("可执行文件或应用名"), "args": serde_json::json!({"type":"array","items":{"type":"string"},"description":"启动参数"}), "cwd": s("工作目录") }),
                &["command"],
            ),
            risk_level: low,
        },
        ToolDef {
            name: "process.kill",
            description: "终止进程（高风险）",
            input_schema: schema(serde_json::json!({ "pid": n("进程 ID") }), &["pid"]),
            risk_level: high,
        },
        ToolDef {
            name: "process.inspect",
            description: "查看进程详情",
            input_schema: schema(serde_json::json!({ "pid": n("进程 ID") }), &["pid"]),
            risk_level: safe,
        },
        // ---------- window.* ----------
        ToolDef {
            name: "window.list",
            description: "列出可见窗口",
            input_schema: schema(serde_json::json!({}), &[]),
            risk_level: safe,
        },
        ToolDef {
            name: "window.find",
            description: "按标题查找窗口",
            input_schema: schema(
                serde_json::json!({ "title": s("窗口标题（包含匹配）") }),
                &["title"],
            ),
            risk_level: safe,
        },
        ToolDef {
            name: "window.focus",
            description: "聚焦窗口",
            input_schema: schema(serde_json::json!({ "title": s("窗口标题") }), &["title"]),
            risk_level: low,
        },
        ToolDef {
            name: "window.minimize",
            description: "最小化窗口",
            input_schema: schema(serde_json::json!({ "title": s("窗口标题") }), &["title"]),
            risk_level: low,
        },
        ToolDef {
            name: "window.maximize",
            description: "最大化窗口",
            input_schema: schema(serde_json::json!({ "title": s("窗口标题") }), &["title"]),
            risk_level: low,
        },
        ToolDef {
            name: "window.close",
            description: "关闭窗口",
            input_schema: schema(serde_json::json!({ "title": s("窗口标题") }), &["title"]),
            risk_level: medium,
        },
        // ---------- ui.*（Windows UI Automation，设计文档 #22/#23）----------
        ToolDef {
            name: "ui.inspect",
            description: "查看窗口 UI 树（AutomationId/Name/ControlType）",
            input_schema: schema(
                serde_json::json!({ "window": s("窗口标题"), "depth": n("遍历深度，默认 4") }),
                &[],
            ),
            risk_level: safe,
        },
        ToolDef {
            name: "ui.find",
            description: "按 Name/AutomationId/ControlType 查找控件",
            input_schema: schema(
                serde_json::json!({ "window": s("窗口标题（可选）"), "name": s("控件 Name（可选）"), "automation_id": s("AutomationId（可选，最高优先级）"), "control_type": s("ControlType，如 Button（可选）") }),
                &[],
            ),
            risk_level: safe,
        },
        ToolDef {
            name: "ui.click",
            description: "点击控件",
            input_schema: schema(
                serde_json::json!({ "window": s("窗口标题（可选）"), "name": s("控件 Name"), "automation_id": s("AutomationId（可选）"), "control_type": s("ControlType（可选）") }),
                &["name"],
            ),
            risk_level: medium,
        },
        ToolDef {
            name: "ui.type",
            description: "向控件输入文本",
            input_schema: schema(
                serde_json::json!({ "window": s("窗口标题（可选）"), "name": s("控件 Name"), "text": s("要输入的文本"), "clear": b("是否先清空，默认 false") }),
                &["name", "text"],
            ),
            risk_level: medium,
        },
        ToolDef {
            name: "ui.select",
            description: "选中列表项/下拉项",
            input_schema: schema(
                serde_json::json!({ "window": s("窗口标题（可选）"), "name": s("项 Name") }),
                &["name"],
            ),
            risk_level: medium,
        },
        ToolDef {
            name: "ui.getValue",
            description: "读取控件值",
            input_schema: schema(
                serde_json::json!({ "window": s("窗口标题（可选）"), "name": s("控件 Name") }),
                &["name"],
            ),
            risk_level: safe,
        },
        ToolDef {
            name: "ui.getText",
            description: "读取控件文本",
            input_schema: schema(
                serde_json::json!({ "window": s("窗口标题（可选）"), "name": s("控件 Name（可选，缺省读窗口）") }),
                &[],
            ),
            risk_level: safe,
        },
        // ---------- browser.*（设计文档 #26）----------
        ToolDef {
            name: "browser.open",
            description: "打开/启动浏览器（复用已有会话）",
            input_schema: schema(serde_json::json!({ "url": s("起始 URL，可选") }), &[]),
            risk_level: low,
        },
        ToolDef {
            name: "browser.goto",
            description: "导航到 URL",
            input_schema: schema(serde_json::json!({ "url": s("目标 URL") }), &["url"]),
            risk_level: safe,
        },
        ToolDef {
            name: "browser.back",
            description: "后退",
            input_schema: schema(serde_json::json!({}), &[]),
            risk_level: safe,
        },
        ToolDef {
            name: "browser.forward",
            description: "前进",
            input_schema: schema(serde_json::json!({}), &[]),
            risk_level: safe,
        },
        ToolDef {
            name: "browser.search",
            description: "搜索引擎搜索",
            input_schema: schema(serde_json::json!({ "query": s("搜索关键词") }), &["query"]),
            risk_level: safe,
        },
        ToolDef {
            name: "browser.click",
            description: "点击页面元素（CSS 选择器或按文本匹配）",
            input_schema: schema(
                serde_json::json!({ "selector": s("CSS 选择器（可选）"), "text": s("按文本匹配（可选）") }),
                &[],
            ),
            risk_level: medium,
        },
        ToolDef {
            name: "browser.type",
            description: "向输入框输入文本",
            input_schema: schema(
                serde_json::json!({ "selector": s("CSS 选择器（可选，默认第一个输入框）"), "text": s("文本"), "submit": b("是否回车提交") }),
                &["text"],
            ),
            risk_level: medium,
        },
        ToolDef {
            name: "browser.scroll",
            description: "滚动页面",
            input_schema: schema(serde_json::json!({ "dy": n("纵向像素，默认 600") }), &[]),
            risk_level: safe,
        },
        ToolDef {
            name: "browser.extract",
            description: "提取页面文本/元素内容",
            input_schema: schema(
                serde_json::json!({ "selector": s("CSS 选择器（可选，默认整页文本）") }),
                &[],
            ),
            risk_level: safe,
        },
        ToolDef {
            name: "browser.screenshot",
            description: "页面截图",
            input_schema: schema(serde_json::json!({}), &[]),
            risk_level: safe,
        },
        ToolDef {
            name: "browser.download",
            description: "下载文件（基于页面 fetch，上限 10MB）",
            input_schema: schema(
                serde_json::json!({ "url": s("下载 URL"), "path": s("保存路径") }),
                &["url", "path"],
            ),
            risk_level: medium,
        },
        // ---------- screenshot.* ----------
        ToolDef {
            name: "screenshot.full",
            description: "全屏截图",
            input_schema: schema(serde_json::json!({ "screen": n("屏幕编号，默认 0") }), &[]),
            risk_level: safe,
        },
        ToolDef {
            name: "screenshot.window",
            description: "窗口截图",
            input_schema: schema(serde_json::json!({ "title": s("窗口标题") }), &["title"]),
            risk_level: safe,
        },
        ToolDef {
            name: "screenshot.region",
            description: "区域截图",
            input_schema: schema(
                serde_json::json!({ "x": n("X"), "y": n("Y"), "width": n("宽"), "height": n("高") }),
                &["x", "y", "width", "height"],
            ),
            risk_level: safe,
        },
        // ---------- clipboard.* ----------
        ToolDef {
            name: "clipboard.read",
            description: "读取剪贴板文本",
            input_schema: schema(serde_json::json!({}), &[]),
            risk_level: safe,
        },
        ToolDef {
            name: "clipboard.write",
            description: "写入剪贴板文本",
            input_schema: schema(serde_json::json!({ "text": s("文本") }), &["text"]),
            risk_level: low,
        },
    ]
}

/// 工具执行统一入口。调用方（Tauri 命令层）需先完成权限判定（executor）。
/// `input` 为 owned：同步工具统一放到阻塞线程执行，闭包需要 'static。
pub async fn execute(
    state: &AppState,
    task_id: Option<&str>,
    tool: &str,
    input: serde_json::Value,
) -> Result<serde_json::Value, String> {
    match tool {
        // filesystem
        "filesystem.list" => run_blocking(move || super::filesystem::list(&input)).await,
        "filesystem.read" => run_blocking(move || super::filesystem::read(&input)).await,
        "filesystem.write" => run_blocking(move || super::filesystem::write(&input)).await,
        "filesystem.search" => run_blocking(move || super::filesystem::search(&input)).await,
        "filesystem.copy" => run_blocking(move || super::filesystem::copy(&input)).await,
        "filesystem.move" => run_blocking(move || super::filesystem::move_path(&input)).await,
        "filesystem.rename" => run_blocking(move || super::filesystem::rename(&input)).await,
        "filesystem.mkdir" => run_blocking(move || super::filesystem::mkdir(&input)).await,
        "filesystem.delete" => run_blocking(move || super::filesystem::delete(&input)).await,
        "filesystem.stat" => run_blocking(move || super::filesystem::stat(&input)).await,
        // shell（支持超时与取消）
        "shell.exec" => super::shell::exec(state, task_id, &input).await,
        // process
        "process.list" => run_blocking(move || super::process::list(&input)).await,
        "process.start" => super::process::start(state, task_id, &input),
        "process.kill" => run_blocking(move || super::process::kill(&input)).await,
        "process.inspect" => run_blocking(move || super::process::inspect(&input)).await,
        // window / ui（Windows COM 调用放阻塞线程）
        "window.list" => run_blocking(move || super::window::list(&input)).await,
        "window.find" => run_blocking(move || super::window::find(&input)).await,
        "window.focus" => run_blocking(move || super::window::focus(&input)).await,
        "window.minimize" => run_blocking(move || super::window::minimize(&input)).await,
        "window.maximize" => run_blocking(move || super::window::maximize(&input)).await,
        "window.close" => run_blocking(move || super::window::close(&input)).await,
        "ui.inspect" => run_blocking(move || super::ui::inspect(&input)).await,
        "ui.find" => run_blocking(move || super::ui::find(&input)).await,
        "ui.click" => run_blocking(move || super::ui::click(&input)).await,
        "ui.type" => run_blocking(move || super::ui::type_text(&input)).await,
        "ui.select" => run_blocking(move || super::ui::select(&input)).await,
        "ui.getValue" => run_blocking(move || super::ui::get_value(&input)).await,
        "ui.getText" => run_blocking(move || super::ui::get_text(&input)).await,
        // browser
        "browser.open" => super::browser::open(state, &input).await,
        "browser.goto" => super::browser::goto(state, &input).await,
        "browser.back" => super::browser::back(state).await,
        "browser.forward" => super::browser::forward(state).await,
        "browser.search" => super::browser::search(state, &input).await,
        "browser.click" => super::browser::click(state, &input).await,
        "browser.type" => super::browser::type_text(state, &input).await,
        "browser.scroll" => super::browser::scroll(state, &input).await,
        "browser.extract" => super::browser::extract(state, &input).await,
        "browser.screenshot" => super::browser::screenshot(state, &input).await,
        "browser.download" => super::browser::download(state, &input).await,
        // screenshot
        "screenshot.full" => {
            run_blocking({
                let dir = state.data_dir.clone();
                move || super::screenshot::full(&input, &dir)
            })
            .await
        }
        "screenshot.window" => {
            run_blocking({
                let dir = state.data_dir.clone();
                move || super::screenshot::window(&input, &dir)
            })
            .await
        }
        "screenshot.region" => {
            run_blocking({
                let dir = state.data_dir.clone();
                move || super::screenshot::region(&input, &dir)
            })
            .await
        }
        // clipboard
        "clipboard.read" => run_blocking(super::clipboard::read).await,
        "clipboard.write" => run_blocking(move || super::clipboard::write(&input)).await,
        _ => Err(format!("未知工具: {tool}")),
    }
}

/// 把阻塞调用放到独立线程，避免卡住 async runtime
pub async fn run_blocking<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("工具线程异常: {e}"))?
}
