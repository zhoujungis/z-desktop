import type { ToolMeta } from "./types";

export const windowTools: ToolMeta[] = [
  { tool: "window.list", label: "列出窗口", icon: "🪟", group: "窗口" },
  { tool: "window.find", label: "查找窗口", icon: "🔍", group: "窗口" },
  { tool: "window.focus", label: "聚焦窗口", icon: "🎯", group: "窗口" },
  { tool: "window.minimize", label: "最小化", icon: "🔽", group: "窗口" },
  { tool: "window.maximize", label: "最大化", icon: "🔼", group: "窗口" },
  { tool: "window.close", label: "关闭窗口", icon: "❌", group: "窗口" },
];
