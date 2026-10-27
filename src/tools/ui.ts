import type { ToolMeta } from "./types";

export const uiTools: ToolMeta[] = [
  { tool: "ui.inspect", label: "检查界面", icon: "🧭", group: "界面自动化" },
  { tool: "ui.find", label: "查找控件", icon: "🔍", group: "界面自动化" },
  { tool: "ui.click", label: "点击控件", icon: "👆", group: "界面自动化" },
  { tool: "ui.type", label: "输入文本", icon: "⌨️", group: "界面自动化" },
  { tool: "ui.select", label: "选择选项", icon: "☑️", group: "界面自动化" },
  { tool: "ui.getValue", label: "读取控件值", icon: "📥", group: "界面自动化" },
  { tool: "ui.getText", label: "读取控件文本", icon: "📖", group: "界面自动化" },
];
