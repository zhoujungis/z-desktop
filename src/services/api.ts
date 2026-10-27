import { tauriInvoke } from "./tauri";

// ---------- 基础类型（与 Rust models.rs 的 snake_case JSON 对齐） ----------

export type TaskStatus =
  | "pending"
  | "running"
  | "waiting_approval"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type RiskLevel = "safe" | "low" | "medium" | "high" | "critical";

export interface Task {
  id: string;
  title: string;
  status: TaskStatus;
  created_at: number;
  updated_at: number;
  completed_at: number | null;
}

export interface TaskStep {
  id: string;
  task_id: string;
  step_index: number;
  tool: string | null;
  input: string | null;
  output: string | null;
  status: string | null;
  created_at: number;
}

export interface EventRecord {
  id: string;
  task_id: string | null;
  type: string;
  data: string | null;
  created_at: number;
}

export interface Message {
  id: string;
  task_id: string | null;
  role: string;
  content: string;
  created_at: number;
}

export interface Memory {
  id: string;
  type: string;
  content: string;
  source: string | null;
  created_at: number;
}

export interface Project {
  id: string;
  name: string;
  path: string;
  description: string | null;
  technology: string | null;
  git_url: string | null;
  is_active: boolean;
  created_at: number;
}

export interface AppSettings {
  provider: string;
  base_url: string;
  api_key: string;
  model: string;
  auto_approve_safe: boolean;
  auto_approve_low: boolean;
  auto_approve_medium: boolean;
  shell_safe_auto_exec: boolean;
  max_steps: number;
}

/** 工具定义（Rust 序列化为 risk_level，兼容 riskLevel 两种键名） */
export interface ToolDef {
  name: string;
  description: string;
  input_schema: unknown;
  riskLevel?: RiskLevel;
  risk_level?: RiskLevel;
}

export function toolRisk(def: ToolDef): RiskLevel {
  return def.riskLevel ?? def.risk_level ?? "medium";
}

// ---------- 工具执行 / 审批 / 事件 payload ----------

export interface ToolExecResult {
  status: "executed" | "failed" | "blocked" | "approval_required" | "cancelled";
  result?: unknown;
  error?: string;
  durationMs?: number;
  requestId?: string;
  riskLevel?: string;
  reason?: string;
}

/** "approval-requested" 事件 payload（snake_case） */
export interface ApprovalPayload {
  request_id: string;
  task_id: string | null;
  tool: string;
  input: unknown;
  risk_level: string;
  summary: string;
}

/** "activity" 事件 payload（camelCase） */
export interface ActivityPayload {
  taskId: string | null;
  time: number;
  type: string;
  tool: string | null;
  summary: string;
  detail: string | null;
}

// ---------- filesystem 工具返回结构 ----------

export interface FsEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  modified: number | null;
}

export interface FsListResult {
  path: string;
  entries: FsEntry[];
  count: number;
}

export interface FsReadResult {
  path: string;
  binary: boolean;
  size: number;
  truncated: boolean;
  content: string;
}

export interface FsSearchResult {
  path: string;
  pattern: string;
  matches: FsEntry[];
  count: number;
}

// ---------- 内部辅助：把工具执行结果解包为强类型 ----------

function errMsg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

async function runTool<T>(tool: string, input: Record<string, unknown>, taskId?: string): Promise<T> {
  const res = await api.toolExecute({ taskId, tool, input });
  if (res.status === "executed") return res.result as T;
  throw new Error(res.error ?? res.reason ?? `工具 ${tool} 执行失败（${res.status}）`);
}

// ---------- API ----------

export const api = {
  // 工具 / Agent
  listTools: () => tauriInvoke<ToolDef[]>("list_tools"),
  toolExecute: (args: {
    taskId?: string | null;
    tool: string;
    input: Record<string, unknown>;
    approvalRequestId?: string;
  }) => tauriInvoke<ToolExecResult>("tool_execute", args),
  resolveApproval: (requestId: string, decision: "allow" | "deny", alwaysAllow = false) =>
    tauriInvoke<{ resolved: boolean; decision: string }>("resolve_approval", {
      requestId,
      decision,
      alwaysAllow,
    }),
  cancelTask: (taskId: string) =>
    tauriInvoke<{ cancelled: boolean; taskId: string; killedProcesses: number }>("cancel_task", {
      taskId,
    }),

  // 任务
  createTask: (title: string) => tauriInvoke<Task>("create_task", { title }),
  updateTaskStatus: (id: string, status: TaskStatus) =>
    tauriInvoke<Task>("update_task_status", { id, status }),
  getTask: (id: string) => tauriInvoke<Task | null>("get_task", { id }),
  listTasks: (limit = 50) => tauriInvoke<Task[]>("list_tasks", { limit }),
  listSteps: (taskId: string) => tauriInvoke<TaskStep[]>("list_steps", { taskId }),
  listEvents: (taskId?: string | null, limit = 200) =>
    tauriInvoke<EventRecord[]>("list_events", { taskId, limit }),

  // 消息 / 记忆
  saveMessage: (taskId: string | null, role: string, content: string) =>
    tauriInvoke<Message>("save_message", { taskId, role, content }),
  listMessages: (taskId: string) => tauriInvoke<Message[]>("list_messages", { taskId }),
  addMemory: (memoryType: string, content: string, source?: string) =>
    tauriInvoke<Memory>("add_memory", { memoryType, content, source }),
  searchMemories: (query: string, limit = 10) =>
    tauriInvoke<Memory[]>("search_memories", { query, limit }),

  // 项目
  listProjects: () => tauriInvoke<Project[]>("list_projects"),
  addProject: (args: {
    name: string;
    path: string;
    description?: string;
    technology?: string;
    gitUrl?: string;
  }) => tauriInvoke<Project>("add_project", args),
  setActiveProject: (id: string | null) => tauriInvoke<void>("set_active_project", { id }),
  getActiveProject: () => tauriInvoke<Project | null>("get_active_project"),

  // 设置
  getSettings: () => tauriInvoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => tauriInvoke<void>("save_settings", { settings }),

  // filesystem 工具（读取类，safe 自动放行）
  fsList: (path: string, depth = 1) => runTool<FsListResult>("filesystem.list", { path, depth }),
  fsRead: (path: string, maxBytes = 2_000_000) =>
    runTool<FsReadResult>("filesystem.read", { path, max_bytes: maxBytes }),
  fsSearch: (path: string, pattern: string, maxResults = 200) =>
    runTool<FsSearchResult>("filesystem.search", { path, pattern, max_results: maxResults }),
};

export { errMsg };
