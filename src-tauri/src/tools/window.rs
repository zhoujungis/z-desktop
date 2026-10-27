//! window.* 工具：基于 Windows UI Automation 枚举与控制顶层窗口

use super::registry::param_str;
use serde_json::{json, Value};

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::windows::automation::{automation, find_window};
    use crate::windows::window as win;
    use uiautomation::types::WindowVisualState;

    pub fn list(_input: &Value) -> Result<Value, String> {
        let windows = win::list_windows()?;
        Ok(json!({ "windows": windows, "count": windows.len() }))
    }

    pub fn find(input: &Value) -> Result<Value, String> {
        let title = param_str(input, "title").ok_or("缺少参数 title")?;
        let auto = automation()?;
        let w = find_window(&auto, title)?;
        Ok(json!({
            "found": true,
            "window": crate::windows::automation::element_info(&w),
        }))
    }

    pub fn focus(input: &Value) -> Result<Value, String> {
        let title = param_str(input, "title").ok_or("缺少参数 title")?;
        let auto = automation()?;
        let w = find_window(&auto, title)?;
        w.set_focus().map_err(|e| format!("聚焦窗口失败: {e}"))?;
        Ok(json!({
            "focused": true,
            "window": crate::windows::automation::element_info(&w),
        }))
    }

    pub fn minimize(input: &Value) -> Result<Value, String> {
        let title = param_str(input, "title").ok_or("缺少参数 title")?;
        win::set_visual_state(title, WindowVisualState::Minimized)
    }

    pub fn maximize(input: &Value) -> Result<Value, String> {
        let title = param_str(input, "title").ok_or("缺少参数 title")?;
        win::set_visual_state(title, WindowVisualState::Maximized)
    }

    pub fn close(input: &Value) -> Result<Value, String> {
        let title = param_str(input, "title").ok_or("缺少参数 title")?;
        win::close_window(title)
    }

    pub fn find_window_rect(title: &str) -> Result<(i32, i32, i32, i32), String> {
        win::find_window_rect(title)
    }
}

#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
mod imp {
    use serde_json::Value;

    pub fn find_window_rect(_title: &str) -> Result<(i32, i32, i32, i32), String> {
        Err("window.* 工具仅支持 Windows".into())
    }
}

#[cfg(not(windows))]
pub use imp::find_window_rect;

#[cfg(not(windows))]
macro_rules! windows_only {
    ($name:ident) => {
        pub fn $name(_input: &Value) -> Result<Value, String> {
            Err("window.* 工具仅支持 Windows（需要 UI Automation）".into())
        }
    };
}

#[cfg(not(windows))]
windows_only!(list);
#[cfg(not(windows))]
windows_only!(find);
#[cfg(not(windows))]
windows_only!(focus);
#[cfg(not(windows))]
windows_only!(minimize);
#[cfg(not(windows))]
windows_only!(maximize);
#[cfg(not(windows))]
windows_only!(close);
