//! ui.* 工具：Windows UI Automation 桌面控件操作
//!
//! 定位优先级（设计文档 #23）：AutomationId > Name > ControlType > 父子关系 > UI Tree，
//! 坐标点击仅作为最后 fallback。

use super::registry::{param_bool, param_str, param_u64};
use serde_json::{json, Value};

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::windows::automation::{
        automation, center_point, element_info, find_element, resolve_root,
    };
    use uiautomation::inputs::{Keyboard, Mouse};
    use uiautomation::patterns::{
        UIInvokePattern, UISelectionItemPattern, UITextPattern, UIValuePattern,
    };

    pub fn inspect(input: &Value) -> Result<Value, String> {
        let auto = automation()?;
        let root = resolve_root(&auto, param_str(input, "window"))?;
        let depth = param_u64(input, "depth").unwrap_or(4).min(10) as u32;
        let elements = auto
            .create_matcher()
            .from_ref(&root)
            .depth(depth)
            .timeout(5000)
            .find_all()
            .map_err(|e| format!("遍历 UI 树失败: {e}"))?;
        let list: Vec<Value> = elements.iter().map(element_info).collect();
        Ok(json!({ "elements": list, "count": list.len() }))
    }

    pub fn find(input: &Value) -> Result<Value, String> {
        let auto = automation()?;
        let e = locate(&auto, input)?;
        Ok(json!({ "found": true, "element": element_info(&e) }))
    }

    /// 从工具入参提取定位条件
    fn locate(
        auto: &uiautomation::core::UIAutomation,
        input: &Value,
    ) -> Result<uiautomation::core::UIElement, String> {
        find_element(
            auto,
            param_str(input, "window"),
            param_str(input, "automation_id"),
            param_str(input, "name"),
            param_str(input, "control_type"),
        )
    }

    /// 点击控件：优先 Invoke 模式，失败时坐标点击（最后 fallback）
    pub fn click(input: &Value) -> Result<Value, String> {
        let auto = automation()?;
        let e = locate(&auto, input)?;
        let info = element_info(&e);
        if let Ok(invoke) = e.get_pattern::<UIInvokePattern>() {
            if invoke.invoke().is_ok() {
                return Ok(json!({ "clicked": true, "method": "invoke", "element": info }));
            }
        }
        let Some(point) = center_point(&e) else {
            return Err("控件不可点击（无 Invoke 模式也无可用坐标）".into());
        };
        Mouse::new()
            .click(&point)
            .map_err(|e| format!("坐标点击失败: {e}"))?;
        Ok(json!({ "clicked": true, "method": "coordinate", "element": info }))
    }

    /// 输入文本：聚焦控件后模拟键盘输入；clear 时先尝试 Value 模式清空
    pub fn type_text(input: &Value) -> Result<Value, String> {
        let text = param_str(input, "text").ok_or("缺少参数 text")?.to_string();
        let clear = param_bool(input, "clear").unwrap_or(false);
        let auto = automation()?;
        let e = locate(&auto, input)?;
        let info = element_info(&e);
        e.set_focus()
            .map_err(|err| format!("聚焦控件失败: {err}"))?;
        if clear {
            if let Ok(vp) = e.get_pattern::<UIValuePattern>() {
                let _ = vp.set_value("");
            }
        }
        Keyboard::new()
            .send_text(&text)
            .map_err(|err| format!("键盘输入失败: {err}"))?;
        Ok(json!({ "typed": true, "length": text.chars().count(), "element": info }))
    }

    /// 选中列表项 / 下拉项：优先 SelectionItem 模式，失败时 Invoke / 点击
    pub fn select(input: &Value) -> Result<Value, String> {
        let auto = automation()?;
        let e = locate(&auto, input)?;
        let info = element_info(&e);
        if let Ok(sel) = e.get_pattern::<UISelectionItemPattern>() {
            if sel.select().is_ok() {
                return Ok(json!({ "selected": true, "method": "selectionItem", "element": info }));
            }
        }
        if let Ok(invoke) = e.get_pattern::<UIInvokePattern>() {
            if invoke.invoke().is_ok() {
                return Ok(json!({ "selected": true, "method": "invoke", "element": info }));
            }
        }
        let Some(point) = center_point(&e) else {
            return Err("无法选中该控件（无 Select/Invoke 模式也无可用坐标）".into());
        };
        Mouse::new()
            .click(&point)
            .map_err(|err| format!("坐标点击失败: {err}"))?;
        Ok(json!({ "selected": true, "method": "coordinate", "element": info }))
    }

    pub fn get_value(input: &Value) -> Result<Value, String> {
        let auto = automation()?;
        let e = locate(&auto, input)?;
        let value = e
            .get_pattern::<UIValuePattern>()
            .and_then(|v| v.get_value())
            .unwrap_or_else(|_| e.get_name().unwrap_or_default());
        Ok(json!({ "value": value, "element": element_info(&e) }))
    }

    pub fn get_text(input: &Value) -> Result<Value, String> {
        let auto = automation()?;
        let e = locate(&auto, input)?;
        // 优先 Value 模式，其次 Text 模式（Document 等），最后 Name
        let mut text = e
            .get_pattern::<UIValuePattern>()
            .and_then(|v| v.get_value())
            .ok();
        if text.as_ref().map_or(true, |s| s.is_empty()) {
            text = e
                .get_pattern::<UITextPattern>()
                .and_then(|t| t.get_document_range())
                .and_then(|r| r.get_text(-1))
                .ok();
        }
        if text.as_ref().map_or(true, |s| s.is_empty()) {
            text = Some(e.get_name().unwrap_or_default());
        }
        let text = text.unwrap_or_default();
        Ok(json!({
            "text": truncate(&text),
            "length": text.chars().count(),
            "element": element_info(&e),
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
}

#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
macro_rules! ui_windows_only {
    ($name:ident) => {
        pub fn $name(_input: &Value) -> Result<Value, String> {
            Err("ui.* 工具仅支持 Windows（需要 UI Automation）".into())
        }
    };
}

#[cfg(not(windows))]
ui_windows_only!(inspect);
#[cfg(not(windows))]
ui_windows_only!(find);
#[cfg(not(windows))]
ui_windows_only!(click);
#[cfg(not(windows))]
ui_windows_only!(type_text);
#[cfg(not(windows))]
ui_windows_only!(select);
#[cfg(not(windows))]
ui_windows_only!(get_value);
#[cfg(not(windows))]
ui_windows_only!(get_text);
