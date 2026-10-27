//! clipboard.* 工具（设计文档 #24）

use super::registry::param_str;
use serde_json::{json, Value};

pub fn read() -> Result<Value, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("访问剪贴板失败: {e}"))?;
    let text = cb.get_text().map_err(|e| format!("读取剪贴板失败: {e}"))?;
    Ok(json!({ "text": text }))
}

pub fn write(input: &Value) -> Result<Value, String> {
    let text = param_str(input, "text").ok_or("缺少参数 text")?;
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("访问剪贴板失败: {e}"))?;
    cb.set_text(text)
        .map_err(|e| format!("写入剪贴板失败: {e}"))?;
    Ok(json!({ "written": true, "length": text.chars().count() }))
}
