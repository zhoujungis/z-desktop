# 安全设计

> 对应《Z_Desktop_v0.1 开发设计文档》#19 / #30 / #31 节。实现位于 `src-tauri/src/security/` 与 `src-tauri/src/agent/mod.rs`。

## 1. 五级风险模型（设计文档 #30）

| 等级 | 默认行为 | 示例 |
|---|---|---|
| SAFE | 自动允许（可配置） | 读文件、列目录、查进程、截图、读剪贴板 |
| LOW | 可配置自动 | 复制/重命名文件、启动应用、写剪贴板 |
| MEDIUM | 需审批（可配置自动） | 写文件、移动文件、关窗口、UI 点击、未知 Shell 命令 |
| HIGH | 每次确认 | 删除文件/目录、杀进程、装软件、注册表写 |
| CRITICAL | **默认禁止** | 格式化、关机、改 HKLM 注册表、关 Defender、fork 炸弹等 |

**规则**：
- CRITICAL 直接 `blocked`，不出审批卡（连"允许一次"都不可）。
- HIGH 忽略一切 auto-approve / Always-allow，每次人工确认。
- "Always allow" 仅对 ≤MEDIUM 生效。

## 2. 命令风险分级（shell.exec 专用，设计文档 #19）

`security/risk.rs::classify_command()` 对命令文本做模式匹配（大小写不敏感）：

- **SAFE 白名单**：`dir`、`ls`、`cat`、`pwd`、`node --version`、`npm ls`、`git status` 等只读命令 → 可配置自动执行。
- **HIGH 黑名单**：`remove-item`、`rd /s`、`del /f`、`reg add/delete`、`taskkill /f`、`git push` / `git reset --hard` / `git clean`、`npm publish`、`pip install`、`curl | sh`、计划任务创建等 → 每次确认。
- **CRITICAL 黑名单**：`format`、`shutdown`、`diskpart`、`bcdedit`、`rm -rf /`、`dd if=`、fork 炸弹等 → 默认禁止。
- 其余命令 → MEDIUM，首次确认，可"Always"。

**防绕过**：命令先按 `&&`、`||`、`;`、`|`、换行拆成独立片段，逐段分级取最高风险；
只有所有片段都是只读命令时才判定 SAFE。因此 `echo ok && rm -rf ./build`、
`cd D:\ && del x`、`dir | Remove-Item ...` 都不会再被 SAFE 白名单“带过”自动执行。
重定向（`>`）也会让命令退出 SAFE 判定。

`shell.exec` 的最终风险以命令分级为准（未知命令默认 MEDIUM）；`shell_safe_auto_exec`
开关关闭后，即使是只读命令也需要审批。

## 3. 权限判定流程（permission.rs）

```
decide(conn, settings, tool, tool_risk, input_json)
  1. shell.exec → classify_command 升级风险
  2. CRITICAL → blocked
  3. 命中 approval_rules 且 ≤MEDIUM → 自动允许
  4. 按设置 auto_approve_safe / low / medium 决定
  5. 否则 requires_approval = true
```

设置项存 `settings` 表（`AppSettings`，SettingsView 可改）：
`auto_approve_safe`（默认 true）、`auto_approve_low`、`auto_approve_medium`、`llm_provider`、`api_key`、`model` 等。

## 4. 路径沙箱（sandbox.rs）

- **保护目录（拒绝写/删）**：`C:\Windows`、`C:\Program Files*`、`/etc`、`/usr`、`/bin`、`/sbin`、`/boot`、`/sys`、`/proc` 等。
- **读**：`check_path(path, PathOp::Read)` — 允许除敏感区域外任意读取。
- **写**：`check_path(path, PathOp::Mutate)` — 拒绝保护目录。
- 路径会先解析 `..`、补全相对路径，并尽可能解析符号链接（保留最后一层不存在的目标），
  因此通过链接或相对路径写入保护目录同样会被拦截；同时保留 Windows 上 `/etc`、`/usr`
  这类 Unix 风格写路径的保护语义。
- 所有 filesystem.* 写操作、shell cwd、browser.download 落盘路径均经校验后才执行。

## 5. 审批 UI 与数据流（设计文档 #31）

```
Rust: pending_approvals.insert(requestId, {tool, input, task_id, risk})
      emit("approval-requested")
前端 ApprovalCard: [Deny] [Allow Once] [Always]
      invoke("resolve_approval", {requestId, decision, alwaysAllow})
Rust: 写回 decision → emit("approval-resolved") → 唤醒等待中的 Loop
      Always + ≤MEDIUM → approval_rules 表落库（tool + 完整 input 精确匹配）
```

防滥用校验：审批唤醒后必须 `same_request`（tool 与 input 完全一致）才复用裁决，防止"批准 A 执行 B"。

## 6. 取消与进程安全

- 所有可杀的子进程 PID 登记在 `AppState.running_pids`（按 task_id 分组）。
- `cancel_task(task_id)`：置取消标志 + 杀该任务全部进程树。
- Windows 杀进程树：`taskkill /PID <pid> /T /F`；Unix：`kill -9`。
- Shell/浏览器等长任务以 100ms 轮询取消标志。

## 7. 边界与未来（v0.1 已知取舍）

- API Key 明文存本地 SQLite `settings` 表（v0.2 计划：OS keychain）。
- Always-allow 规则为精确匹配（tool + input JSON），不做通配；粒度偏细是刻意的保守设计。
- 浏览器自动化限定 CDP，不接受远程调试端口；下载上限 10MB。
- LLM 输出中若夹带命令注入文本，最终仍需通过本层风险分级，LLM 无特权。
