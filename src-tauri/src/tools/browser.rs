//! browser.* 工具（设计文档 #26）：基于 chromiumoxide（CDP）驱动本机 Chrome/Edge
//!
//! 浏览器会话复用：browser.open 启动后存入 AppState，后续工具复用同一页面。

use super::registry::{param_bool, param_f64, param_str};
use crate::state::{AppState, BrowserSession};
use chromiumoxide::Page;
use futures::StreamExt;
use serde_json::{json, Value};

fn current_page(state: &AppState) -> Result<Page, String> {
    let guard = state.browser.lock().unwrap();
    guard
        .as_ref()
        .map(|s| s.page.clone())
        .ok_or_else(|| "浏览器未打开，请先调用 browser.open".into())
}

/// 简易 URL 编码（搜索引擎查询参数用）
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// 执行 JS 并取回字符串结果（失败返回空串）
async fn eval_string(page: &Page, script: &str) -> String {
    page.evaluate_expression(script)
        .await
        .ok()
        .and_then(|r| r.into_value::<String>().ok())
        .unwrap_or_default()
}

pub async fn open(state: &AppState, input: &Value) -> Result<Value, String> {
    let url = param_str(input, "url").map(String::from);
    if state.browser.lock().unwrap().is_some() {
        if let Some(u) = url {
            let mut input2 = serde_json::Map::new();
            input2.insert("url".into(), Value::String(u));
            return goto(state, &Value::Object(input2)).await;
        }
        return Ok(json!({ "ok": true, "reused": true }));
    }

    let config = chromiumoxide::BrowserConfig::builder()
        .with_head()
        .build()
        .map_err(|e| format!("浏览器配置错误: {e}"))?;
    let (browser, mut handler) = chromiumoxide::Browser::launch(config)
        .await
        .map_err(|e| format!("启动浏览器失败: {e}（请确认本机已安装 Chrome 或 Edge）"))?;
    // Handler 必须持续轮询，否则 CDP 事件不会分发
    tokio::spawn(async move {
        while handler.next().await.is_some() {}
    });

    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("创建标签页失败: {e}"))?;
    *state.browser.lock().unwrap() = Some(BrowserSession { browser, page });

    if let Some(u) = url {
        let mut input2 = serde_json::Map::new();
        input2.insert("url".into(), Value::String(u));
        return goto(state, &Value::Object(input2)).await;
    }
    Ok(json!({ "ok": true, "created": true }))
}

pub async fn goto(state: &AppState, input: &Value) -> Result<Value, String> {
    let url = param_str(input, "url")
        .ok_or("缺少参数 url")?
        .to_string();
    let page = current_page(state)?;
    page.goto(url.as_str())
        .await
        .map_err(|e| format!("导航失败: {e}"))?;
    let _ = page.wait_for_navigation().await;
    let title = eval_string(&page, "document.title").await;
    Ok(json!({ "url": url, "title": title }))
}

pub async fn back(state: &AppState) -> Result<Value, String> {
    let page = current_page(state)?;
    let _ = page.evaluate_expression("history.back()").await;
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    Ok(json!({ "ok": true, "action": "back" }))
}

pub async fn forward(state: &AppState) -> Result<Value, String> {
    let page = current_page(state)?;
    let _ = page.evaluate_expression("history.forward()").await;
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    Ok(json!({ "ok": true, "action": "forward" }))
}

pub async fn search(state: &AppState, input: &Value) -> Result<Value, String> {
    let query = param_str(input, "query")
        .ok_or("缺少参数 query")?
        .to_string();
    let mut input2 = serde_json::Map::new();
    input2.insert(
        "url".into(),
        Value::String(format!(
            "https://www.bing.com/search?q={}",
            url_encode(&query)
        )),
    );
    let mut result = goto(state, &Value::Object(input2)).await?;
    result["query"] = Value::String(query);
    Ok(result)
}

pub async fn click(state: &AppState, input: &Value) -> Result<Value, String> {
    let page = current_page(state)?;
    if let Some(selector) = param_str(input, "selector") {
        page.find_element(selector)
            .await
            .map_err(|e| format!("未找到元素 {selector}: {e}"))?
            .click()
            .await
            .map_err(|e| format!("点击失败: {e}"))?;
        return Ok(json!({ "clicked": true, "by": "selector", "selector": selector }));
    }
    let Some(text) = param_str(input, "text") else {
        return Err("需要 selector 或 text 之一".into());
    };
    let script = format!(
        "(() => {{ const els=[...document.querySelectorAll('a,button,[role=button],input[type=submit],input[type=button],summary')]; \
         const el=els.find(e=>(e.innerText||e.value||'').trim().includes({})); \
         if(!el) return 'Z_NOT_FOUND'; el.click(); return 'Z_OK'; }})()",
        serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
    );
    let result = eval_string(&page, &script).await;
    if result == "Z_NOT_FOUND" || result.is_empty() {
        return Err(format!("未找到文本包含「{text}」的可点击元素"));
    }
    Ok(json!({ "clicked": true, "by": "text", "text": text }))
}

pub async fn type_text(state: &AppState, input: &Value) -> Result<Value, String> {
    let text = param_str(input, "text")
        .ok_or("缺少参数 text")?
        .to_string();
    let submit = param_bool(input, "submit").unwrap_or(false);
    let page = current_page(state)?;
    let selector = param_str(input, "selector").unwrap_or("input,textarea");
    let el = page
        .find_element(selector)
        .await
        .map_err(|e| format!("未找到输入框 {selector}: {e}"))?;
    el.click()
        .await
        .map_err(|e| format!("聚焦输入框失败: {e}"))?;
    el.type_str(&text)
        .await
        .map_err(|e| format!("输入失败: {e}"))?;
    if submit {
        el.press_key("Enter")
            .await
            .map_err(|e| format!("回车提交失败: {e}"))?;
        let _ = page.wait_for_navigation().await;
    }
    Ok(json!({ "typed": true, "submitted": submit }))
}

pub async fn scroll(state: &AppState, input: &Value) -> Result<Value, String> {
    let dy = param_f64(input, "dy").unwrap_or(600.0);
    let page = current_page(state)?;
    let script = format!("window.scrollBy(0, {dy})");
    let _ = page.evaluate_expression(script.as_str()).await;
    Ok(json!({ "scrolled": true, "dy": dy }))
}

pub async fn extract(state: &AppState, input: &Value) -> Result<Value, String> {
    let page = current_page(state)?;
    let title = eval_string(&page, "document.title").await;
    match param_str(input, "selector") {
        Some(selector) => {
            let el = page
                .find_element(selector)
                .await
                .map_err(|e| format!("未找到元素 {selector}: {e}"))?;
            let text = el
                .inner_text()
                .await
                .map_err(|e| format!("提取文本失败: {e}"))?
                .unwrap_or_default();
            Ok(json!({ "selector": selector, "text": truncate(&text), "title": title }))
        }
        None => {
            let text = eval_string(&page, "document.body ? document.body.innerText : ''").await;
            Ok(json!({ "text": truncate(&text), "title": title }))
        }
    }
}

pub async fn screenshot(state: &AppState, input: &Value) -> Result<Value, String> {
    let page = current_page(state)?;
    let png = page
        .screenshot(chromiumoxide::page::ScreenshotParams::default())
        .await
        .map_err(|e| format!("页面截图失败: {e}"))?;
    save_png(state, &png, "page", input)
}

pub async fn download(_state: &AppState, input: &Value) -> Result<Value, String> {
    let url = param_str(input, "url")
        .ok_or("缺少参数 url")?
        .to_string();
    let path = param_str(input, "path")
        .ok_or("缺少参数 path")?
        .to_string();
    crate::security::sandbox::check_path(&path, crate::security::sandbox::PathOp::Mutate)?;

    let resp = tauri_plugin_http::reqwest::get(&url)
        .await
        .map_err(|e| format!("下载请求失败: {e}"))?;
    let status = resp.status().as_u16();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取下载数据失败: {e}"))?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err("下载文件超过 10MB 上限".into());
    }
    std::fs::write(&path, &bytes).map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(json!({
        "url": url,
        "path": path,
        "size": bytes.len(),
        "status": status,
    }))
}

fn truncate(s: &str) -> String {
    const MAX: usize = 20_000;
    if s.len() <= MAX {
        return s.to_string();
    }
    let mut end = MAX;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…（已截断）", &s[..end])
}

fn save_png(
    state: &AppState,
    png: &[u8],
    kind: &str,
    input: &Value,
) -> Result<Value, String> {
    let dir = state.data_dir.join("screenshots");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建截图目录失败: {e}"))?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S%3f");
    let path = dir.join(format!("{kind}-{ts}.png"));
    std::fs::write(&path, png).map_err(|e| format!("写入截图失败: {e}"))?;
    let with_base64 = param_bool(input, "base64").unwrap_or(false);
    Ok(json!({
        "path": path.display().to_string(),
        "size": png.len(),
        "base64": if with_base64 {
            use base64::Engine as _;
            Some(base64::engine::general_purpose::STANDARD.encode(png))
        } else {
            None
        },
    }))
}
