# Z Desktop 架构设计

> 对应《Z_Desktop_v0.1 开发设计文档》#2 / #7 / #14-#16 节。

## 1. 总体架构

Z Desktop 是一个 AI 原生桌面工作台（AI Computer Workspace），采用 **Tauri 2 混合架构**：

```
┌──────────────────────────────────────────────────────┐
│                  Frontend (WebView)                   │
│   Vue 3 + TypeScript + Pinia + Tailwind CSS          │
│   ┌─────────┐ ┌──────────────────┐ ┌──────────────┐  │
│   │ Workspace│ │  Main Workspace  │ │  Activity    │  │
│   │ 导航栏   │ │  Home/Task/Files │ │  Panel       │  │
│   └─────────┘ └──────────────────┘ └──────────────┘  │
│              底部 Chat Input（全局对话入口）            │
│   Agent Loop（规划/执行/观察/验证）+ LLM Provider      │
└──────────────────────┬───────────────────────────────┘
                       │ Tauri IPC（invoke + event）
┌──────────────────────┴───────────────────────────────┐
│                  Rust Core (src-tauri)                │
│  ┌────────────┐ ┌────────────┐ ┌──────────────────┐  │
│  │ commands   │ │ agent 引擎  │ │ tools 工具系统    │  │
│  │ 20 个命令   │ │ 审批/取消   │ │ 44 个工具实现     │  │
│  └────────────┘ └────────────┘ └──────────────────┘  │
│  ┌────────────┐ ┌────────────┐ ┌──────────────────┐  │
│  │ security   │ │ database   │ │ windows 底层封装  │  │
│  │ 风险/沙箱   │ │ SQLite+FTS5│ │ UIA/窗口/进程     │  │
│  └────────────┘ └────────────┘ └──────────────────┘  │
└──────────────────────────────────────────────────────┘
```

**核心设计原则：Rust 侧强制权限校验。** 前端 Agent Loop 负责规划与编排，但任何工具调用最终都必须经过 Rust 侧 `request_tool_execution()` → 权限判定 → 执行，前端无法绕过安全层。

## 2. 技术栈

| 层 | 技术 |
|---|---|
| 前端框架 | Vue 3（Composition API + `<script setup>`） |
| 语言 | TypeScript（严格模式） |
| 构建 | Vite 6 |
| 状态 | Pinia |
| 路由 | Vue Router（Hash 模式，兼容 Tauri file 协议） |
| 样式 | Tailwind CSS 3 |
| 桌面框架 | Tauri 2（Rust 后端 + 系统 WebView） |
| 后端语言 | Rust（edition 2021） |
| 数据库 | SQLite（rusqlite bundled，内含 FTS5） |
| 浏览器自动化 | chromiumoxide（CDP 协议） |
| Windows 自动化 | uiautomation（COM UIA） |
| 系统信息 | sysinfo、arboard（剪贴板）、screenshots（截屏） |

## 3. 进程与通信模型

- **单窗口单进程对**：`z-desktop.exe`（Rust 宿主）+ WebView 前端。
- **命令通道（请求/响应）**：前端通过 `invoke()` 调用 20 个 Tauri 命令（见 [tools.md](tools.md) §7）。
- **事件通道（后端 → 前端推送）**：
  - `activity` — Agent 每一步工具调用的实时流水（时间戳 + tool + 参数 + 结果摘要）
  - `task-updated` — 任务状态变更（创建/状态流转），驱动 TaskList 与 StepTimeline 刷新
  - `approval-requested` — 审批请求推送，驱动 ApprovalCard 弹出
  - `approval-resolved` — 审批裁决结果，唤醒等待中的 Agent Loop

## 4. 一次任务执行的完整数据流

```
用户输入（Chat Input）
  → agent store.startTask()
  → POST create_task 命令（Rust 写入 tasks 表）
  → Agent Loop（前端）状态机循环：
      THINKING → 调 LLM 生成计划
      PLANNING → 产出工具调用序列
      EXECUTING → invoke("tool_execute", {tool, input, taskId, approvalRequestId})
                    ↓ Rust: permission.decide()
                    ├─ SAFE/已批准 → 直接执行 → {status:"executed", output}
                    ├─ 需审批 → pending_approvals 入队 → emit approval-requested
                    │            → 前端 Promise 等待 approval-resolved
                    ├─ CRITICAL → {status:"blocked"}
                    └─ 异常 → {status:"failed"}
      OBSERVING → 记录结果、更新 Activity / Task Steps
      VERIFYING → LLM 校验目标是否达成
      → 达成 → COMPLETED；未达成 → 回到 THINKING（max_steps 上限保护）
      → 任一环节可被用户 [Stop] 打断（cancel_task 杀进程树 + 置取消标志）
```

## 5. 模块边界

### 5.1 前端（src/）

| 目录 | 职责 |
|---|---|
| `services/` | `tauri.ts`（IPC 封装）、`api.ts`（20 个命令 + 类型）、`events.ts`（事件监听/审批等待）、`llm.ts`（多 Provider 适配） |
| `stores/` | `agent`（Loop 状态机/活动流/审批）、`task`、`workspace`、`settings` |
| `agent/` | `loop.ts`（状态机主循环）、`planner.ts`（LLM 规划）、`executor.ts`（工具调用）、`verifier.ts`（结果校验）、`context.ts`（上下文组装） |
| `components/` | `chat/`、`task/`、`activity/`、`approval/`、`files/`、`common/` |
| `pages/` | HomeView、TaskView、FilesView、ProjectsView、SettingsView |

### 5.2 Rust（src-tauri/src/）

| 目录 | 职责 |
|---|---|
| `commands.rs` | 20 个 `#[tauri::command]`，薄层转发 |
| `agent/mod.rs` | `request_tool_execution`（执行入口/审批队列）、`resolve_approval`、`cancel_task` |
| `tools/` | `registry.rs`（44 个工具定义与统一执行入口）+ 8 个实现模块 |
| `security/` | `risk.rs`（五级风险 + 命令模式库）、`permission.rs`（判定引擎）、`sandbox.rs`（路径保护） |
| `database/` | `migrations.rs`（启动建表）、`models.rs`（CRUD）、`mod.rs`（连接管理，WAL 模式） |
| `windows/` | `automation.rs`（UIA 底层）、`window.rs`、`process.rs`（Windows 专用） |
| `state.rs` | `AppState`：DB 连接、审批队列、运行中 PID 表、浏览器会话、AppHandle |

## 6. 关键设计决策

1. **Agent Loop 放前端、安全放后端**：规划需要频繁调 LLM，放前端便于流式 UI；但工具执行收口在 Rust，安全不可绕过。
2. **审批复用校验**：`approval-resolved` 唤醒时校验 `same_request`（tool + input 完全一致），防止"批准 A 后执行 B"。
3. **Always-allow 上限 Medium**：审批规则表只对 ≤MEDIUM 风险生效，HIGH 每次确认（设计文档 #19）。
4. **取消即杀进程树**：`cancel_task` 置取消标志 + 遍历 `running_pids` 执行 `taskkill /T /F`（Windows），Shell 轮询取消标志实现即时中断。
5. **阻塞调用全部隔离**：文件/注册表/UIA/截图等同步实现统一经 `spawn_blocking` 执行，不阻塞 async runtime。
6. **Windows 条件编译**：`windows/` 模块与 window/ui 工具在非 Windows 平台编译为返回友好提示的桩，保证跨平台可构建。

## 7. 目录结构

见项目根 [README.md](../README.md) §4，与设计文档 #7 一致。
