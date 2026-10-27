//! Windows 底层能力封装（UI Automation / 进程 / 窗口）

#[cfg(windows)]
pub mod automation;
#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod window;

/// 非 Windows 平台提供空实现（工具层返回"仅支持 Windows"）
#[cfg(not(windows))]
pub mod automation {}
#[cfg(not(windows))]
pub mod process {}
#[cfg(not(windows))]
pub mod window {}
