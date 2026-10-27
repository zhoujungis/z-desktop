# 工具系统

> 对应《Z_Desktop_v0.1 开发设计文档》#18-#26 节。所有工具定义集中在 `src-tauri/src/tools/registry.rs` 的 `tool_definitions()`，共 44 项。

## 1. 统一约定

- **调用方式**：前端 `invoke("tool_execute", { tool, input, taskId?, approvalRequestId? })`。
- **返回**：`{ status: "executed"|"approval_required"|"blocked"|"failed"|"cancelled", output?, requestId? }`。
- **风险等级**：每个工具注册时声明 SAFE/LOW/MEDIUM/HIGH，`shell.exec` 会按命令内容动态升级（见 security.md）。
- **阻塞隔离**：所有同步实现统一经 `spawn_blocking` 执行。

## 2. 文件系统 filesystem.*（10 项）

| 工具 | 风险 | 输入 | 说明 |
|---|---|---|---|
| filesystem.list | safe | path, depth? | 列目录（walkdir 递归） |
| filesystem.read | safe | path, max_bytes? | 读文件（默认 2MB，超限截断标记 truncated） |
| filesystem.write | medium | path, content | 写文件，自动建父目录 |
| filesystem.search | safe | path, pattern, max_results? | 通配符递归搜索，如 `*.pdf` |
| filesystem.copy | low | from, to, overwrite? | 复制文件/目录 |
| filesystem.move | medium | from, to, overwrite? | 移动文件/目录 |
| filesystem.rename | low | path, new_name | 重命名（仅文件名） |
| filesystem.mkdir | low | path | 递归建目录 |
| filesystem.delete | high | path, recursive? | 删除文件/目录 |
| filesystem.stat | safe | path | 元信息（大小/修改时间/类型） |

所有写操作路径经 sandbox 校验（保护目录见 security.md §4）。

## 3. Shell shell.exec（1 项）

| 参数 | 说明 |
|---|---|
| command | 命令内容（必填） |
| cwd | 工作目录（经 sandbox 读校验） |
| timeout | 超时毫秒，默认 120000 |
| shell | `powershell`（默认）/ `cmd` / `wsl` / `gitbash` |

实现要点（`tools/shell.rs`）：

- 独立线程读取 stdout/stderr 管道，UTF-8 边界安全截断（保留尾部 100KB）。
- 等待退出最多 2s 冲刷剩余输出；超时/取消时杀进程树。
- 返回 `{exitCode, stdout, stderr, duration, timedOut, cancelled, pid}`；子进程 PID 登记到 `running_pids` 供取消。
- 非 Windows 环境映射到 `/bin/sh`。

## 4. 进程 process.*（4 项）

| 工具 | 风险 | 说明 |
|---|---|---|
| process.list | safe | 列进程（filter/limit，按内存排序） |
| process.inspect | safe | 单进程详情（exe/cmdline/cwd/父进程/CPU/内存/启动时间） |
| process.start | low | 启动应用（分离模式，返回 pid） |
| process.kill | high | 终止进程 |

基于 sysinfo 0.38，`refresh_processes(ProcessesToUpdate)` 精确刷新。

## 5. 窗口 window.*（6 项，Windows）

| 工具 | 风险 | 说明 |
|---|---|---|
| window.list | safe | 列出可见顶层窗口（标题/pid/矩形） |
| window.find | safe | 按标题查找（包含匹配） |
| window.focus | low | 聚焦 |
| window.minimize / window.maximize | low | 窗口状态 |
| window.close | medium | 关闭窗口 |

底层走 UIA `UIWindowPattern`（`windows/window.rs`）。

## 6. UI 自动化 ui.*（7 项，Windows）

| 工具 | 风险 | 说明 |
|---|---|---|
| ui.inspect | safe | 遍历窗口 UI 树（AutomationId/Name/ControlType/Rect） |
| ui.find | safe | 按条件查找控件，返回元素信息 |
| ui.click | medium | 点击：优先 `UIInvokePattern`，fallback 可点击点坐标 |
| ui.type | medium | 输入文本：聚焦 → 可选 ValuePattern 清空 → `Keyboard::send_text` |
| ui.select | medium | 选中列表项：SelectionItemPattern → Invoke → 坐标 |
| ui.getValue | safe | 读控件值（ValuePattern） |
| ui.getText | safe | 读控件文本（TextPattern 文档范围 → Name 回退） |

**定位优先级（设计文档 #23）**：AutomationId > Name > ControlType > 父子层级 > UI Tree；Vision/坐标仅作最后 fallback（click 的坐标 fallback）。

## 7. 浏览器 browser.*（11 项）

chromiumoxide（CDP）驱动 Chrome/Chromium，`browser.open` 拉起并维持会话（Handler 持续轮询）。

| 工具 | 风险 | 说明 |
|---|---|---|
| browser.open | low | 启动浏览器/复用会话，可选起始 URL |
| browser.goto | safe | 导航并等待加载，返回标题 |
| browser.back / browser.forward | safe | 历史导航 |
| browser.search | safe | Bing 搜索 |
| browser.click | medium | CSS 选择器或按文本匹配 a/button/[role=button] |
| browser.type | medium | 输入框输入，可选回车提交 |
| browser.scroll | safe | 纵向滚动 |
| browser.extract | safe | 提取元素/整页文本 |
| browser.screenshot | safe | 页面截图存档 |
| browser.download | medium | 下载文件（上限 10MB，落盘路径经 sandbox） |

## 8. 截图 screenshot.*（3 项）

| 工具 | 参数 | 说明 |
|---|---|---|
| screenshot.full | screen? | 全屏（多屏按编号） |
| screenshot.window | title | 窗口截图（UIA 取矩形后区域截屏） |
| screenshot.region | x,y,width,height | 区域截图 |

PNG 存至 `{data_dir}/screenshots/`，返回路径，可选 base64。

## 9. 剪贴板 clipboard.*（2 项）

`clipboard.read`（safe）/ `clipboard.write`（low），基于 arboard。

## 10. Tauri 命令层（20 个）

| 类别 | 命令 |
|---|---|
| 工具 | list_tools、tool_execute |
| 审批/取消 | resolve_approval、cancel_task |
| 任务 | create_task、get_task、list_tasks、update_task_status、list_steps、list_events |
| 会话 | save_message、list_messages |
| 记忆 | add_memory、search_memories |
| 项目 | list_projects、add_project、set_active_project、get_active_project |
| 设置 | get_settings、save_settings |

## 11. 新增工具步骤

1. 在对应模块实现 `pub fn/async fn xxx(input: &Value) -> Result<Value, String>`。
2. 在 `registry.rs::execute()` 的 match 中登记分支（阻塞实现包 `run_blocking`）。
3. 在 `tool_definitions()` 追加 `ToolDef`（名称/描述/input_schema/risk_level）。
4. 前端 `src/services/api.ts` 如需专属封装则补充；通用调用直接走 `tool_execute`。
