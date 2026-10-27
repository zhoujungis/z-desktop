import { toolRisk, type ToolDef } from "@/services/api";
import type { ChatMessage, LlmTool } from "@/services/llm";
import type { AgentContext } from "./types";

/** 把 Rust ToolDef 转换为各 Provider 通用的 LLM 工具格式 */
export function toLlmTools(defs: ToolDef[]): LlmTool[] {
  return defs.map((d) => ({
    name: d.name,
    description: `${d.description}（风险等级: ${toolRisk(d)}）`,
    parameters: isSchemaObject(d.input_schema) ? d.input_schema : { type: "object", properties: {} },
  }));
}

function isSchemaObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** 组装 system prompt + 用户任务（设计文档 #37） */
export function buildPlannerMessages(
  userPrompt: string,
  ctx: AgentContext,
  tools: LlmTool[],
): ChatMessage[] {
  const toolLines = tools.map((t) => `- ${t.name}: ${t.description}`).join("\n");
  const memoryLines =
    ctx.memories.length > 0
      ? ctx.memories.map((m) => `- [${m.type}] ${m.content}`).join("\n")
      : "（暂无相关记忆）";

  const system = [
    "你是 Z Desktop 桌面 Agent，可以直接操作用户的电脑：文件系统、Shell 命令、进程、窗口、GUI 自动化、浏览器、截图与剪贴板。",
    "",
    "## 可用工具",
    toolLines,
    "",
    "## 当前上下文",
    `- 当前时间：${ctx.now}`,
    `- 活跃项目：${ctx.projectName ?? "（未设置）"}${ctx.projectPath ? `，路径：${ctx.projectPath}` : ""}`,
    ctx.projectTech ? `- 项目技术栈：${ctx.projectTech}` : null,
    `- 相关记忆：`,
    memoryLines,
    "",
    "## 工作要求",
    "1. 每一步只调用一个工具，根据返回的观察结果决定下一步。",
    "2. 完成任务后不要再调用工具，直接用中文输出最终总结。",
    "3. 需要用户审批的工具会返回 approval_required，此时不要重复发起相同调用，等待用户决定。",
    "4. 工具返回 failed 时，分析错误原因并尝试修正；同一操作最多重试一次。",
    "5. 文件与 Shell 操作优先以活跃项目路径作为工作目录。",
    "6. Shell 命令请单条执行，不要用 && / ; / | 串联多个操作；危险命令会被安全层拦截。",
  ]
    .filter((line): line is string => line !== null)
    .join("\n");

  return [
    { role: "system", content: system },
    { role: "user", content: userPrompt },
  ];
}
