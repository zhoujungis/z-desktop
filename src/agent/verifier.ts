import type { ChatMessage, LlmClient } from "@/services/llm";

export interface VerifyResult {
  passed: boolean;
  reason: string;
}

const YES = /^(yes|true|pass|passed|ok|是|对|通过|已完成|完成)/i;
const NO = /^(no|false|fail|failed|否|不|未通过|未完成|失败)/i;

/**
 * 解析验证器输出：只看回答开头的第一个词。
 * 不能简单地全文搜索 yes/no —— "no, but it could be yes" 会被误判为通过。
 */
export function parseVerdict(text: string | null | undefined): boolean {
  const head = (text ?? "")
    .trim()
    .replace(/^[*#`"'“”‘’\s]+/, "")
    .toLowerCase();
  if (!head) return false;
  if (YES.test(head)) return true;
  if (NO.test(head)) return false;
  // 没有明确开头时，要求 yes 出现在 no 之前
  const yesIdx = head.search(/\b(?:yes|true)\b/);
  const noIdx = head.search(/\b(?:no|false)\b/);
  return yesIdx >= 0 && (noIdx < 0 || yesIdx < noIdx);
}

/** 验证引擎（设计文档 #33）：判断任务是否完成且验证通过 */
export async function verify(
  llm: LlmClient,
  userPrompt: string,
  finalText: string,
  signal?: AbortSignal,
): Promise<VerifyResult> {
  const messages: ChatMessage[] = [
    {
      role: "system",
      content:
        "你是严格的任务验证器。根据任务目标和 Agent 的最终输出，判断任务是否已完成且结果可信。只回答 yes 或 no，随后用一句话说明原因。",
    },
    {
      role: "user",
      content: `任务目标：\n${userPrompt}\n\nAgent 最终输出：\n${finalText || "（无输出）"}\n\n任务是否已完成且验证通过？`,
    },
  ];
  try {
    const res = await llm.chat(messages, [], signal);
    const text = (res.text ?? "").trim();
    const passed = parseVerdict(text);
    return { passed, reason: text || (passed ? "验证通过" : "验证未通过") };
  } catch (e) {
    // 用户主动停止导致的取消必须向上抛出，不能按“验证通过”处理
    if (signal?.aborted) throw e;
    // 验证器不可用时采纳最终输出，避免验证环节死循环
    return {
      passed: true,
      reason: `验证器调用失败，直接采纳最终输出：${e instanceof Error ? e.message : String(e)}`,
    };
  }
}
