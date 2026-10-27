# Agent 设计

> 对应《Z_Desktop_v0.1 开发设计文档》#17 / #37-#40 节。

## 1. Agent 状态机（设计文档 #38）

```
IDLE → THINKING → PLANNING → EXECUTING → OBSERVING → VERIFYING ─┐
         ↑                                        │              │
         └──────────── 未达成，继续规划 ←──────────┘              │
                                      达成 → COMPLETED ←─────────┘

EXECUTING ── 需审批 ──→ WAITING_APPROVAL ── Allow ──→ EXECUTING
                              │ Deny
                              └──→ 回到 THINKING（换方案）或终止

异常 → ERROR → RECOVERING（重试一次）→ EXECUTING
用户 [Stop] → 任意状态 → （ cancelled ）→ IDLE
```

状态由前端 `stores/agent.ts` 持有，主循环在 `src/agent/loop.ts`。

## 2. Agent Loop（src/agent/loop.ts）

每轮迭代：

1. **THINKING**：组装上下文（系统提示 + 任务 + 历史 + 可用工具清单），调用 LLM。
2. **PLANNING**：解析 LLM 返回的工具调用（LLM tool calling），得到 `{tool, input}` 序列。
3. **EXECUTING**：逐个调用 `invoke("tool_execute", { tool, input, taskId, approvalRequestId })`。
   - Rust 返回五种 status：`executed` / `approval_required` / `blocked` / `failed` / `cancelled`。
   - `approval_required`：前端显示 ApprovalCard，用 Promise 挂起等待 `approval-resolved` 事件。
4. **OBSERVING**：把工具输出追加到消息历史与 Task Steps，推送 Activity。
5. **VERIFYING**：再调一次 LLM 判断目标是否已达成。

### 保护机制

| 机制 | 说明 |
|---|---|
| max_steps | 默认 30 步上限（可配置 1–200），防止死循环 |
| LLM 失败重试 | 网络等临时错误重试一次 |
| Stop 检查 | 每步循环前检查取消标志；审批等待同时支持 deny 唤醒与取消唤醒 |
| CRITICAL 拦截 | Rust 侧直接返回 `blocked`，Loop 记录并继续/终止 |

## 3. 规划（planner.ts）

- 系统提示词要求 LLM：先理解任务 → 输出**一个**下一步工具调用（ReAct 风格单步规划），附简短理由。
- 使用 OpenAI function calling 协议为基准，`services/llm.ts` 负责协议转换：
  - **OpenAI 兼容**（OpenAI / OpenRouter / Custom / Local）：原生 `tools` 字段。
  - **Anthropic**：`tools` + `tool_use` content block，双向转换。
  - **Gemini**：`functionDeclarations` + `functionCall`，双向转换。

## 4. 上下文（context.ts）

按优先级组装，超限时从低优先级截断：

1. 系统提示（角色 + 能力边界 + 安全约束）
2. 任务目标
3. 最近工具调用与结果（裁剪至 4KB/条）
4. Memory 检索结果（FTS5 相关记忆）
5. 当前活跃项目信息

## 5. 验证（verifier.ts）

- 将「任务目标 + 最终输出」交给 LLM，要求以 `yes` / `no` 开头并附一句原因。
- 解析时只看回答开头的判定词（兼容 `是` / `否` / `通过` / `未通过`），避免 `no, but ... yes` 被误判为通过。
- 未达成时携带 reason 回到 THINKING，使下一轮规划有纠偏依据；连续两次未通过则保留最终输出、附加验证意见后收敛为 `completed`，避免耗尽全部步数。
- 达成后 Loop 写入 `completed` 状态并落库。

## 6. 审批流（Rust agent/mod.rs）

```
tool_execute ─→ permission.decide()
  ├─ auto allow ─→ 执行
  └─ 需审批 ─→ 生成 requestId
        → pending_approvals.insert(requestId, ApprovalState{request, decision:None})
        → emit("approval-requested")
        → 前端 ApprovalCard 显示 [Deny] [Allow Once] [Always]
  resolve_approval(requestId, decision, always_allow)
        → 写回 decision → emit("approval-resolved")
        → always_allow=true 且风险≤MEDIUM → 落库 approval_rules
        → 唤醒等待的 Loop → same_request 校验 → 执行
```

## 7. 取消（cancel_task）

- 置 `cancelled_tasks` 标志（Loop 每步检查）。
- 取出该任务的 `running_pids`，逐个杀进程树（Windows：`taskkill /PID x /T /F`）。
- Shell 工具内部以 100ms 轮询取消标志，命中后 kill 子进程并返回 `cancelled`。
- 审批等待中的任务：deny 唤醒 + 状态标记取消。

## 8. 记忆（Memory）

- `add_memory` / `search_memories` 命令落库 memories 表。
- FTS5 虚拟表 + 触发器自动同步（见 [database.md](database.md)）。
- Loop 每轮规划前检索相关记忆注入上下文。
