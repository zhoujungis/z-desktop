import type { UnlistenFn } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import * as loop from "@/agent/loop";
import type { AgentStateName } from "@/agent/types";
import { api, type ActivityPayload } from "@/services/api";
import {
  cancelApprovalWait,
  initApprovalListeners,
  onActivity,
  onApprovalRequested,
} from "@/services/events";
import { useTaskStore } from "./task";

export const AGENT_STATE_LABEL: Record<AgentStateName, string> = {
  IDLE: "空闲",
  THINKING: "思考中",
  PLANNING: "规划中",
  EXECUTING: "执行中",
  WAITING_APPROVAL: "等待审批",
  OBSERVING: "观察结果",
  VERIFYING: "验证中",
  COMPLETED: "已完成",
  ERROR: "出错",
  CANCELLED: "已取消",
  RECOVERING: "自动恢复中",
};

const TERMINAL_STATES: AgentStateName[] = ["IDLE", "COMPLETED", "ERROR", "CANCELLED"];
const MAX_ACTIVITIES = 300;

export interface PendingApprovalInfo {
  requestId: string;
  taskId: string | null;
  tool: string;
  input: unknown;
  riskLevel: string;
  summary: string;
}

let unlisten: UnlistenFn[] = [];

export const useAgentStore = defineStore("agent", {
  state: () => ({
    state: "IDLE" as AgentStateName,
    currentTaskId: null as string | null,
    pendingApproval: null as PendingApprovalInfo | null,
    activities: [] as ActivityPayload[],
    error: null as string | null,
    /** LLM 流式输出的实时思考文本（Streaming，#005） */
    thinkingText: "",
  }),
  getters: {
    isRunning: (s): boolean => !TERMINAL_STATES.includes(s.state),
    stateLabel: (s): string => AGENT_STATE_LABEL[s.state],
  },
  actions: {
    setState(s: AgentStateName) {
      this.state = s;
      if (s !== "THINKING") this.thinkingText = "";
    },
    appendThinking(delta: string) {
      // 只保留尾部 2000 字符，避免长输出撑爆 UI
      this.thinkingText = (this.thinkingText + delta).slice(-2000);
    },
    /** 注册全局事件监听（App 挂载时调用一次） */
    async init() {
      if (unlisten.length > 0) return;
      await initApprovalListeners().catch(() => undefined);
      const offActivity = await onActivity((p) => this.pushActivity(p));
      const offApproval = await onApprovalRequested((p) => {
        this.setPendingApproval({
          requestId: p.request_id,
          taskId: p.task_id,
          tool: p.tool,
          input: p.input,
          riskLevel: p.risk_level,
          summary: p.summary,
        });
      });
      unlisten = [offActivity, offApproval];
    },
    initSafe() {
      this.init().catch(() => undefined);
    },
    dispose() {
      for (const off of unlisten) off();
      unlisten = [];
    },
    pushActivity(p: ActivityPayload) {
      this.activities.push(p);
      if (this.activities.length > MAX_ACTIVITIES) {
        this.activities.splice(0, this.activities.length - MAX_ACTIVITIES);
      }
    },
    clearActivities() {
      this.activities = [];
    },
    setPendingApproval(a: PendingApprovalInfo) {
      const cur = this.pendingApproval;
      if (cur && cur.requestId === a.requestId) {
        // 事件里的 summary 更完整；保留已有详细摘要
        const keepOldSummary = cur.summary !== `请求执行 ${cur.tool}` && a.summary === `请求执行 ${a.tool}`;
        this.pendingApproval = { ...cur, ...a, summary: keepOldSummary ? cur.summary : a.summary };
      } else {
        this.pendingApproval = a;
      }
    },
    clearPendingApproval() {
      this.pendingApproval = null;
    },
    clearError() {
      this.error = null;
    },
    /** 创建任务并启动 Agent Loop，返回任务 id */
    async startTask(prompt: string): Promise<string | null> {
      const trimmed = prompt.trim();
      if (!trimmed || this.isRunning) return null;
      this.error = null;
      const taskStore = useTaskStore();
      try {
        const title = trimmed.length > 60 ? `${trimmed.slice(0, 60)}…` : trimmed;
        const task = await api.createTask(title);
        this.currentTaskId = task.id;
        this.setState("THINKING");
        taskStore.tasks.unshift(task);
        void loop.runTask(task.id, trimmed);
        return task.id;
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        this.setState("ERROR");
        return null;
      }
    },
    /** 用户随时停止（设计文档 #40） */
    async stop() {
      if (!this.currentTaskId) return;
      loop.requestStop();
      if (this.pendingApproval) {
        cancelApprovalWait(this.pendingApproval.requestId);
        this.clearPendingApproval();
      }
      try {
        await api.cancelTask(this.currentTaskId);
      } catch {
        // 取消失败不阻塞 UI，Loop 会在下一次循环检测到停止标志
      }
    },
    /** ApprovalCard 按钮回调；返回是否提交成功 */
    async resolveApproval(decision: "allow" | "deny", alwaysAllow = false): Promise<boolean> {
      const cur = this.pendingApproval;
      if (!cur) return false;
      try {
        await api.resolveApproval(cur.requestId, decision, alwaysAllow);
        return true;
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        return false;
      }
    },
  },
});
