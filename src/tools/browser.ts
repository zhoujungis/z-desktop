import type { ToolMeta } from "./types";

export const browserTools: ToolMeta[] = [
  { tool: "browser.open", label: "打开浏览器", icon: "🌐", group: "浏览器" },
  { tool: "browser.goto", label: "访问网址", icon: "➡️", group: "浏览器" },
  { tool: "browser.back", label: "后退", icon: "↩️", group: "浏览器" },
  { tool: "browser.forward", label: "前进", icon: "↪️", group: "浏览器" },
  { tool: "browser.search", label: "网页搜索", icon: "🔎", group: "浏览器" },
  { tool: "browser.click", label: "点击元素", icon: "👆", group: "浏览器" },
  { tool: "browser.type", label: "输入内容", icon: "⌨️", group: "浏览器" },
  { tool: "browser.scroll", label: "滚动页面", icon: "🖱️", group: "浏览器" },
  { tool: "browser.extract", label: "提取内容", icon: "📋", group: "浏览器" },
  { tool: "browser.screenshot", label: "页面截图", icon: "📸", group: "浏览器" },
  { tool: "browser.download", label: "下载文件", icon: "💾", group: "浏览器" },
];
