//! Windows 顶层窗口枚举与控制（基于 UI Automation WindowPattern）

use serde_json::{json, Value};
use uiautomation::core::UIElement;
use uiautomation::patterns::UIWindowPattern;
use uiautomation::types::WindowVisualState;

use super::automation::{automation, element_info, find_window};

fn window_info(e: &UIElement) -> Value {
    element_info(e)
}

pub fn list_windows() -> Result<Vec<Value>, String> {
    let auto = automation()?;
    let windows = auto
        .create_matcher()
        .depth(2)
        .control_type(uiautomation::types::ControlType::Window)
        .find_all()
        .map_err(|e| format!("枚举窗口失败: {e}"))?;
    Ok(windows.iter().map(window_info).collect())
}

pub fn set_visual_state(title: &str, state: WindowVisualState) -> Result<Value, String> {
    let auto = automation()?;
    let w = find_window(&auto, title)?;
    let pattern = w
        .get_pattern::<UIWindowPattern>()
        .map_err(|e| format!("窗口不支持视觉状态控制: {e}"))?;
    pattern
        .set_window_visual_state(state)
        .map_err(|e| format!("设置窗口状态失败: {e}"))?;
    Ok(json!({ "ok": true, "window": window_info(&w) }))
}

pub fn close_window(title: &str) -> Result<Value, String> {
    let auto = automation()?;
    let w = find_window(&auto, title)?;
    let pattern = w
        .get_pattern::<UIWindowPattern>()
        .map_err(|e| format!("窗口不支持关闭操作: {e}"))?;
    pattern.close().map_err(|e| format!("关闭窗口失败: {e}"))?;
    Ok(json!({ "closed": true, "window": window_info(&w) }))
}

/// 供 screenshot.window 使用：查找窗口并返回 (left, top, width, height)
pub fn find_window_rect(title: &str) -> Result<(i32, i32, i32, i32), String> {
    let auto = automation()?;
    let w = find_window(&auto, title)?;
    let r = w
        .get_bounding_rectangle()
        .map_err(|e| format!("获取窗口位置失败: {e}"))?;
    Ok((
        r.get_left(),
        r.get_top(),
        (r.get_right() - r.get_left()).max(1),
        (r.get_bottom() - r.get_top()).max(1),
    ))
}
