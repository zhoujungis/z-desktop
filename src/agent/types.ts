import type { Memory } from "@/services/api";

/** Agent 状态机状态（设计文档 #38） */
export type AgentStateName =
  | "IDLE"
  | "THINKING"
  | "PLANNING"
  | "EXECUTING"
  | "WAITING_APPROVAL"
  | "OBSERVING"
  | "VERIFYING"
  | "COMPLETED"
  | "ERROR"
  | "CANCELLED"
  | "RECOVERING";

/** Agent 运行上下文（planner 组装 system prompt 用） */
export interface AgentContext {
  projectName: string | null;
  projectPath: string | null;
  projectTech: string | null;
  memories: Memory[];
  now: string;
}
