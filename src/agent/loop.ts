import { api, type ToolExecResult } from "@/services/api";
import { waitForApprovalResolved } from "@/services/events";
import { createLlmClient, type ChatMessage } from "@/services/llm";
import { useAgentStore } from "@/stores/agent";
import { useSettingsStore } from "@/stores/settings";
import { buildContext } from "./context";
import { executeTool } from "./executor";
import { buildPlannerMessages, toLlmTools } from "./planner";
import { verify } from "./verifier";

// ---------- Stop 控制（设计文档 #40） ----------

let stopRequested = false;

/** 请求停止当前 Loop（store.stop() 调用），下一次循环检测到即退出 */
export function requestStop(): void {
  stopRequested = true;
}

// ---------- 观察结果 ----------

const OBSERVATION_LIMIT = 4000;

function truncate(s: string, max = OBSERVATION_LIMIT): string {
  return s.length <= max ? s : `${s.slice(0, max)}…（已截断）`;
}

/** 把工具执行结果压缩为 LLM 可读的观察字符串 */
function observationOf(r: ToolExecResult): string {
  const obj: Record<string, unknown> = { status: r.status };
  if (r.status === "executed") obj.result = r.result ?? null;
  else if (r.status === "failed") obj.error = r.error ?? "未知错误";
  else if (r.status === "blocked") obj.reason = r.reason ?? "已被安全策略拦截";
  return truncate(JSON.stringify(obj));
}

async function safeUpdateStatus(taskId: string, status: Parameters<typeof api.updateTaskStatus>[1]) {
  try {
    await api.updateTaskStatus(taskId, status);
  } catch {
    // 状态更新失败不中断收尾流程
  }
}

// ---------- Agent Loop 主流程 ----------

export async function runTask(taskId: string, userPrompt: string): Promise<void> {
  const agent = useAgentStore();
  const settingsStore = useSettingsStore();
  stopRequested = false;

  if (!settingsStore.loaded) await settingsStore.loadOrDefault();
  const settings = settingsStore.settings;

  try {
    agent.setState("THINKING");
    await safeUpdateStatus(taskId, "running");

    // 1. 拉取工具定义并转换为 LLM 格式
    const toolDefs = await api.listTools();
    const llmTools = toLlmTools(toolDefs);

    // 2. 组装上下文与 system prompt
    agent.setState("PLANNING");
    const ctx = await buildContext(userPrompt);
    const llm = createLlmClient(settings);
    const messages: ChatMessage[] = buildPlannerMessages(userPrompt, ctx, llmTools);
    await api.saveMessage(taskId, "user", userPrompt);

    const maxSteps = Math.max(1, Math.min(200, Math.floor(settings.max_steps) || 30));
    let llmRetried = false;
    let lastText: string | null = null;
    let cancelled = false;
    let verifyFailures = 0;

    for (let step = 0; step < maxSteps; step++) {
      if (stopRequested) {
        cancelled = true;
        break;
      }
      agent.setState("THINKING");

      // 3. 调用 LLM（流式输出思考文本；失败自动重试一次）
      let res;
      try {
        agent.thinkingText = "";
        res = await llm.chatStream(messages, llmTools, (d) => agent.appendThinking(d));
      } catch (e) {
        if (llmRetried || stopRequested) throw e;
        llmRetried = true;
        agent.setState("RECOVERING");
        messages.push({
          role: "user",
          content: `上一次 LLM 调用失败（${e instanceof Error ? e.message : String(e)}），请从当前进度继续。`,
        });
        continue;
      }
      llmRetried = false;
      if (res.text) lastText = res.text;

      // 4a. 返回工具调用：逐个执行并回填观察结果
      if (res.toolCalls.length > 0) {
        messages.push({ role: "assistant", content: res.text ?? "", toolCalls: res.toolCalls });
        let interrupted = false;
        for (const call of res.toolCalls) {
          if (stopRequested) {
            cancelled = true;
            interrupted = true;
            break;
          }
          agent.setState("EXECUTING");
          let result = await executeTool(taskId, call.name, call.arguments);

          // 审批流：等待用户决定后重发或以「拒绝」作为观察结果
          if (result.status === "approval_required" && result.requestId) {
            agent.setPendingApproval({
              requestId: result.requestId,
              taskId,
              tool: call.name,
              input: call.arguments,
              riskLevel: result.riskLevel ?? "medium",
              summary: `请求执行 ${call.name}`,
            });
            agent.setState("WAITING_APPROVAL");
            const decision = await waitForApprovalResolved(result.requestId);
            agent.clearPendingApproval();
            if (stopRequested) {
              cancelled = true;
              interrupted = true;
              break;
            }
            if (decision === "allow") {
              agent.setState("EXECUTING");
              result = await executeTool(taskId, call.name, call.arguments, result.requestId);
            } else {
              result = { status: "failed", error: "用户拒绝该操作" };
            }
          }

          if (result.status === "cancelled") {
            cancelled = true;
            interrupted = true;
            break;
          }

          agent.setState("OBSERVING");
          messages.push({
            role: "tool",
            name: call.name,
            toolCallId: call.id,
            content: observationOf(result),
          });
        }
        if (interrupted) break;
        continue;
      }

      // 4b. 纯文本输出 → 验证
      const summary = res.text ?? "";
      agent.setState("VERIFYING");
      const verdict = await verify(llm, userPrompt, summary);
      if (verdict.passed) {
        await api.saveMessage(taskId, "assistant", summary);
        await safeUpdateStatus(taskId, "completed");
        agent.setState("COMPLETED");
        return;
      }
      verifyFailures += 1;
      // 连续两次验证不通过时收敛：保留最终输出并标注验证意见，
      // 避免把剩余步数全部耗在「输出 → 驳回」循环里。
      if (verifyFailures >= 2) {
        const withNote = summary
          ? `${summary}\n\n（自动验证未通过：${verdict.reason}）`
          : `自动验证未通过：${verdict.reason}`;
        await api.saveMessage(taskId, "assistant", withNote);
        await safeUpdateStatus(taskId, "completed");
        agent.setState("COMPLETED");
        return;
      }
      // 验证未通过：带着原因继续循环
      messages.push({ role: "assistant", content: summary });
      messages.push({
        role: "user",
        content: `验证未通过：${verdict.reason}\n请继续完成任务，完成后再次给出总结。`,
      });
    }

    // 5. 循环结束：用户停止 / 步数用尽
    if (lastText) await api.saveMessage(taskId, "assistant", lastText);
    if (cancelled || stopRequested) {
      await safeUpdateStatus(taskId, "cancelled");
      agent.setState("CANCELLED");
    } else {
      await safeUpdateStatus(taskId, "failed");
      agent.setState("ERROR");
      agent.error = "已达到最大步数上限，任务未完成。可在「设置」中调大 max_steps 后重试。";
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    agent.error = msg;
    agent.setState("ERROR");
    await safeUpdateStatus(taskId, "failed");
    try {
      await api.saveMessage(taskId, "assistant", `任务执行出错：${msg}`);
    } catch {
      // 消息保存失败忽略
    }
  }
}
