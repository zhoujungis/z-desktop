import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ActivityPayload, ApprovalPayload, Task } from "./api";

/** 订阅实时活动流 */
export function onActivity(handler: (payload: ActivityPayload) => void): Promise<UnlistenFn> {
  return listen<ActivityPayload>("activity", (e) => handler(e.payload));
}

/** 订阅任务状态变更 */
export function onTaskUpdated(handler: (task: Task) => void): Promise<UnlistenFn> {
  return listen<Task>("task-updated", (e) => handler(e.payload));
}

/** 订阅审批请求 */
export function onApprovalRequested(handler: (payload: ApprovalPayload) => void): Promise<UnlistenFn> {
  return listen<ApprovalPayload>("approval-requested", (e) => handler(e.payload));
}

// ---------- 审批结果 → Promise（Agent Loop 等待用户决定） ----------

export type ApprovalDecision = "allow" | "deny";

type Waiter = (decision: ApprovalDecision) => void;
const waiters = new Map<string, Waiter[]>();
let approvalListener: Promise<void> | null = null;

/** 预注册 "approval-resolved" 监听（agent store init 时调用，避免错过事件） */
export function initApprovalListeners(): Promise<void> {
  if (!approvalListener) {
    approvalListener = listen<{ requestId: string; decision: string }>(
      "approval-resolved",
      (e) => {
        const list = waiters.get(e.payload.requestId);
        if (!list) return;
        waiters.delete(e.payload.requestId);
        const decision: ApprovalDecision = e.payload.decision === "allow" ? "allow" : "deny";
        for (const w of list) w(decision);
      },
    )
      .then(() => undefined)
      .catch((e) => {
        // 注册失败时允许下次重试，否则等待中的审批会永远挂起
        approvalListener = null;
        throw e;
      });
  }
  return approvalListener;
}

/** 等待指定审批请求被用户决定 */
export async function waitForApprovalResolved(requestId: string): Promise<ApprovalDecision> {
  await initApprovalListeners();
  return new Promise<ApprovalDecision>((resolve) => {
    const list = waiters.get(requestId) ?? [];
    list.push(resolve);
    waiters.set(requestId, list);
  });
}

/** 手动唤醒等待者（用户 Stop 任务时按 deny 处理，让 Loop 退出） */
export function cancelApprovalWait(requestId: string): void {
  const list = waiters.get(requestId);
  if (!list) return;
  waiters.delete(requestId);
  for (const w of list) w("deny");
}
