//! Windows UI Automation 底层封装（设计文档 #22 / #23）
//!
//! 定位优先级：AutomationId > Name > ControlType > 父子关系 > UI Tree > Vision > 坐标

use serde_json::{json, Value};
use uiautomation::core::{UIAutomation, UIElement, UIMatcher};
use uiautomation::types::{ControlType, Point};

/// 初始化 UIAutomation COM 实例
pub fn automation() -> Result<UIAutomation, String> {
    UIAutomation::new().map_err(|e| format!("初始化 UIAutomation 失败: {e}"))
}

/// 元素信息序列化（Agent 观察用）
pub fn element_info(e: &UIElement) -> Value {
    let rect = e.get_bounding_rectangle().ok();
    let clickable = e.get_clickable_point().ok().flatten();
    json!({
        "name": e.get_name().unwrap_or_default(),
        "automationId": e.get_automation_id().unwrap_or_default(),
        "controlType": e.get_control_type().ok().map(|c| format!("{c:?}")).unwrap_or_default(),
        "classname": e.get_classname().unwrap_or_default(),
        "pid": e.get_process_id().unwrap_or(0),
        "rect": rect.map(|r| json!({
            "left": r.get_left(),
            "top": r.get_top(),
            "right": r.get_right(),
            "bottom": r.get_bottom(),
        })),
        "clickablePoint": clickable.map(|p| json!({"x": p.get_x(), "y": p.get_y()})),
    })
}

/// 按标题查找顶层窗口
pub fn find_window(auto: &UIAutomation, title: &str) -> Result<UIElement, String> {
    auto.create_matcher()
        .depth(2)
        .control_type(ControlType::Window)
        .contains_name(title)
        .timeout(3000)
        .find_first()
        .map_err(|_| format!("未找到标题包含「{title}」的窗口"))
}

/// 解析定位根：指定窗口标题则用该窗口，否则用桌面根
pub fn resolve_root(auto: &UIAutomation, window: Option<&str>) -> Result<UIElement, String> {
    match window {
        Some(t) if !t.trim().is_empty() => find_window(auto, t),
        _ => auto
            .get_root_element()
            .map_err(|e| format!("获取桌面根元素失败: {e}")),
    }
}

/// ControlType 字符串 → 枚举
pub fn parse_control_type(name: &str) -> Result<ControlType, String> {
    let ct = match name {
        "Button" => ControlType::Button,
        "Calendar" => ControlType::Calendar,
        "CheckBox" => ControlType::CheckBox,
        "ComboBox" => ControlType::ComboBox,
        "Custom" => ControlType::Custom,
        "DataGrid" => ControlType::DataGrid,
        "DataItem" => ControlType::DataItem,
        "Document" => ControlType::Document,
        "Edit" => ControlType::Edit,
        "Group" => ControlType::Group,
        "Hyperlink" => ControlType::Hyperlink,
        "Image" => ControlType::Image,
        "List" => ControlType::List,
        "ListItem" => ControlType::ListItem,
        "Menu" => ControlType::Menu,
        "MenuBar" => ControlType::MenuBar,
        "MenuItem" => ControlType::MenuItem,
        "Pane" => ControlType::Pane,
        "ProgressBar" => ControlType::ProgressBar,
        "RadioButton" => ControlType::RadioButton,
        "ScrollBar" => ControlType::ScrollBar,
        "Slider" => ControlType::Slider,
        "Spinner" => ControlType::Spinner,
        "SplitButton" => ControlType::SplitButton,
        "StatusBar" => ControlType::StatusBar,
        "Tab" => ControlType::Tab,
        "TabItem" => ControlType::TabItem,
        "Table" => ControlType::Table,
        "Text" => ControlType::Text,
        "TitleBar" => ControlType::TitleBar,
        "ToolBar" => ControlType::ToolBar,
        "ToolTip" => ControlType::ToolTip,
        "Tree" => ControlType::Tree,
        "TreeItem" => ControlType::TreeItem,
        "Window" => ControlType::Window,
        other => return Err(format!("不支持的 ControlType: {other}")),
    };
    Ok(ct)
}

/// 构建带过滤条件的查找器（AutomationId > Name > ControlType）
pub fn build_matcher(
    auto: &UIAutomation,
    window: Option<&str>,
    automation_id: Option<&str>,
    name: Option<&str>,
    control_type: Option<&str>,
    depth: u32,
) -> Result<UIMatcher, String> {
    let root = resolve_root(auto, window)?;
    let mut matcher = auto.create_matcher().from_ref(&root).depth(depth).timeout(4000);

    if let Some(aid) = automation_id {
        let aid = aid.to_string();
        matcher = matcher.filter_fn(Box::new(move |e: &UIElement| {
            Ok(e.get_automation_id().unwrap_or_default() == aid)
        }));
    }
    if let Some(n) = name {
        matcher = matcher.contains_name(n);
    }
    if let Some(ct) = control_type {
        matcher = matcher.control_type(parse_control_type(ct)?);
    }
    Ok(matcher)
}

/// 按条件查找元素（window + automation_id / name / control_type）
pub fn find_element(
    auto: &UIAutomation,
    window: Option<&str>,
    automation_id: Option<&str>,
    name: Option<&str>,
    control_type: Option<&str>,
) -> Result<UIElement, String> {
    if automation_id.is_none() && name.is_none() && control_type.is_none() {
        return Err("至少需要 name / automation_id / control_type 之一".into());
    }
    build_matcher(auto, window, automation_id, name, control_type, 8)?
        .find_first()
        .map_err(|e| format!("未找到匹配的控件: {e}"))
}

/// 控件中心点：优先 clickable_point，fallback 到矩形中心（坐标仅最后手段）
pub fn center_point(e: &UIElement) -> Option<Point> {
    e.get_clickable_point().ok().flatten().or_else(|| {
        e.get_bounding_rectangle().ok().map(|r| {
            Point::new(
                (r.get_left() + r.get_right()) / 2,
                (r.get_top() + r.get_bottom()) / 2,
            )
        })
    })
}
