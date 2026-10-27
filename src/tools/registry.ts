import { browserTools } from "./browser";
import { clipboardTools } from "./clipboard";
import { filesystemTools } from "./filesystem";
import { processTools } from "./process";
import { screenshotTools } from "./screenshot";
import { shellTools } from "./shell";
import { uiTools } from "./ui";
import { windowTools } from "./window";
import type { ToolMeta } from "./types";

export type { ToolMeta } from "./types";

/** 全部工具元数据（与 Rust tool_definitions 的 44 个工具一一对应） */
export const ALL_TOOL_META: ToolMeta[] = [
  ...filesystemTools,
  ...shellTools,
  ...processTools,
  ...windowTools,
  ...uiTools,
  ...browserTools,
  ...screenshotTools,
  ...clipboardTools,
];

const BY_NAME = new Map(ALL_TOOL_META.map((m) => [m.tool, m]));

/** 按工具名查询元数据；未登记的工具回退为通用元数据 */
export function toolMetaOf(tool: string | null | undefined): ToolMeta {
  if (tool) {
    const hit = BY_NAME.get(tool);
    if (hit) return hit;
  }
  return { tool: tool ?? "", label: tool || "未知工具", icon: "🔧", group: "其他" };
}
