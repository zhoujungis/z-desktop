import { api, type ToolExecResult } from "@/services/api";

/**
 * 执行一次工具调用（Rust 侧负责权限判定、审批与 activity 事件）。
 * 返回 tool_execute 的完整响应。
 */
export async function executeTool(
  taskId: string | null,
  tool: string,
  input: Record<string, unknown>,
  approvalRequestId?: string,
): Promise<ToolExecResult> {
  return api.toolExecute({ taskId, tool, input, approvalRequestId });
}
