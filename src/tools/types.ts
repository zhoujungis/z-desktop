/** 前端工具元数据（工具 schema 单一来源在 Rust tool_definitions，此处仅做 UI 展示辅助） */
export interface ToolMeta {
  /** Rust 注册的工具名，如 filesystem.read */
  tool: string;
  /** 中文显示名 */
  label: string;
  /** 展示图标 */
  icon: string;
  /** 分组中文名 */
  group: string;
}
