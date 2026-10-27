import type { ToolMeta } from "./types";

export const processTools: ToolMeta[] = [
  { tool: "process.list", label: "查看进程", icon: "📊", group: "进程" },
  { tool: "process.inspect", label: "进程详情", icon: "🔎", group: "进程" },
  { tool: "process.start", label: "启动应用", icon: "🚀", group: "进程" },
  { tool: "process.kill", label: "终止进程", icon: "⛔", group: "进程" },
];
