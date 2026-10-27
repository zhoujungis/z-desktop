import type { ToolMeta } from "./types";

export const filesystemTools: ToolMeta[] = [
  { tool: "filesystem.list", label: "列出文件", icon: "📂", group: "文件系统" },
  { tool: "filesystem.read", label: "读取文件", icon: "📄", group: "文件系统" },
  { tool: "filesystem.write", label: "写入文件", icon: "✏️", group: "文件系统" },
  { tool: "filesystem.search", label: "搜索文件", icon: "🔍", group: "文件系统" },
  { tool: "filesystem.copy", label: "复制", icon: "📑", group: "文件系统" },
  { tool: "filesystem.move", label: "移动", icon: "📦", group: "文件系统" },
  { tool: "filesystem.rename", label: "重命名", icon: "🏷️", group: "文件系统" },
  { tool: "filesystem.mkdir", label: "新建目录", icon: "📁", group: "文件系统" },
  { tool: "filesystem.delete", label: "删除", icon: "🗑️", group: "文件系统" },
  { tool: "filesystem.stat", label: "文件信息", icon: "ℹ️", group: "文件系统" },
];
