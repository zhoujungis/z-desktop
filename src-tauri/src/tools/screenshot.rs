//! screenshot.* 工具（设计文档 #25）：全屏 / 区域 / 窗口截图，保存到应用数据目录

use super::registry::{param_i64, param_str, param_u64};
use screenshots::Screen;
use serde_json::{json, Value};
use std::path::Path;

fn save_png(
    img: screenshots::image::RgbaImage,
    data_dir: &Path,
    kind: &str,
) -> Result<Value, String> {
    let dir = data_dir.join("screenshots");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建截图目录失败: {e}"))?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S%3f");
    let path = dir.join(format!("{kind}-{ts}.png"));
    img.save(&path).map_err(|e| format!("写入截图失败: {e}"))?;
    Ok(json!({
        "path": path.display().to_string(),
        "width": img.width(),
        "height": img.height(),
    }))
}

pub fn full(input: &Value, data_dir: &Path) -> Result<Value, String> {
    let idx = param_u64(input, "screen").unwrap_or(0) as usize;
    let screens = Screen::all().map_err(|e| format!("获取屏幕信息失败: {e}"))?;
    let screen = screens
        .get(idx)
        .ok_or_else(|| format!("屏幕编号 {idx} 不存在（共 {} 块屏幕）", screens.len()))?;
    let img = screen.capture().map_err(|e| format!("全屏截图失败: {e}"))?;
    save_png(img, data_dir, "full")
}

pub fn region(input: &Value, data_dir: &Path) -> Result<Value, String> {
    // 多显示器时左侧/上方的屏幕坐标为负，必须用有符号整数
    let x = param_i64(input, "x").ok_or("缺少参数 x")?;
    let y = param_i64(input, "y").ok_or("缺少参数 y")?;
    let x = i32::try_from(x).map_err(|_| "x 超出屏幕坐标范围".to_string())?;
    let y = i32::try_from(y).map_err(|_| "y 超出屏幕坐标范围".to_string())?;
    let width = param_u64(input, "width").ok_or("缺少参数 width")?;
    let height = param_u64(input, "height").ok_or("缺少参数 height")?;
    if width == 0 || height == 0 {
        return Err("截图宽高必须大于 0".into());
    }
    let width = u32::try_from(width).map_err(|_| "width 超出范围".to_string())?;
    let height = u32::try_from(height).map_err(|_| "height 超出范围".to_string())?;
    let screen = Screen::from_point(x, y).map_err(|e| format!("定位屏幕失败: {e}"))?;
    let img = screen
        .capture_area(x, y, width, height)
        .map_err(|e| format!("区域截图失败: {e}"))?;
    save_png(img, data_dir, "region")
}

pub fn window(input: &Value, data_dir: &Path) -> Result<Value, String> {
    let title = param_str(input, "title").ok_or("缺少参数 title")?;
    let (x, y, width, height) = crate::tools::window::find_window_rect(title)?;
    let cx = x + width / 2;
    let cy = y + height / 2;
    let screen = Screen::from_point(cx, cy).map_err(|e| format!("定位屏幕失败: {e}"))?;
    let img = screen
        .capture_area(x, y, width as u32, height as u32)
        .map_err(|e| format!("窗口截图失败: {e}"))?;
    save_png(img, data_dir, "window")
}
