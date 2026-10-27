import { api, type Memory } from "@/services/api";
import { useWorkspaceStore } from "@/stores/workspace";
import type { AgentContext } from "./types";

/** 收集 Agent 上下文：活跃项目、相关记忆、当前时间 */
export async function buildContext(userPrompt: string): Promise<AgentContext> {
  const ws = useWorkspaceStore();
  if (!ws.activeProject && ws.projects.length === 0) {
    await ws.loadProjectsSafe();
  }

  // 取任务描述中的关键词检索长期记忆（FTS5）
  const keywords = userPrompt
    .trim()
    .split(/\s+/)
    .filter((w) => w.length > 1)
    .slice(0, 6)
    .join(" ");
  let memories: Memory[] = [];
  if (keywords) {
    try {
      memories = await api.searchMemories(keywords, 5);
    } catch {
      memories = [];
    }
  }

  return {
    projectName: ws.activeProject?.name ?? null,
    projectPath: ws.activeProject?.path ?? null,
    projectTech: ws.activeProject?.technology ?? null,
    memories,
    now: new Date().toLocaleString("zh-CN"),
  };
}
