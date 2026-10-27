# Roadmap

> 对应《Z_Desktop_v0.1 开发设计文档》#50 / #51 节的 30 项开工清单与验收标准。

## v0.1（当前版本）— P0 已交付

**目标**：验证「AI 在授权下安全操作电脑」的最小闭环。

- ✅ 项目脚手架（Vite + Vue3 + TS + Tailwind + Tauri 2 + Rust）
- ✅ SQLite 9 表 + FTS5 全文检索
- ✅ 五级风险模型 + 命令分级 + 路径沙箱 + 审批队列
- ✅ 44 个工具：filesystem(10) / shell(1) / process(4) / window(6) / ui(7) / browser(11) / screenshot(3) / clipboard(2)
- ✅ Agent Loop：THINKING→PLANNING→EXECUTING→OBSERVING→VERIFYING，max_steps/重试/取消
- ✅ 多 Provider LLM：OpenAI / Anthropic / Gemini / OpenRouter / Custom / Local
- ✅ UI 三栏布局 + Home / Task / Files / Projects / Settings
- ✅ Activity 实时面板 + Approval 卡片 + Task Timeline
- ✅ 审批 Always-allow（≤MEDIUM）规则落库

**验收（设计文档 #51）**：读取/创建/移动文件 ✅；执行命令 ✅；启动程序 ✅；查看进程 ✅；操作 GUI ✅（Windows）；浏览网页 ✅；截图 ✅；读剪贴板 ✅；调用 LLM ✅；自动规划/执行/验证 ✅；高风险询问 ✅；随时停止 ✅（取消标志+杀进程树）；Timeline 记录 ✅。

## v0.2 — 安全与体验增强

- API Key 迁移 OS Keychain（Windows Credential Manager）
- 文件操作批量审批（一次批准一组同类操作）
- Shell 沙箱模式（受限 token / Job Object 资源限制）
- 记忆 embedding 化，向量 + FTS 混合检索
- 事件表自动归档
- 多显示器截图选择器与屏幕预览

## v0.3 — 能力扩展

- Vision 定位补全：UI 找不到控件时截图 + 多模态 LLM 定位（坐标 fallback 正式化）
- 浏览器会话持久化（profile/cookie 复用）
- 定时任务 / 工作流（把成功 Task 存为可重放 Workflow）
- MCP 工具接入（对外工具生态）
- 插件系统：第三方工具包注册

## v1.0 — 产品化

- Windows Installer（MSIX/NSIS）+ 自动更新
- 崩溃恢复：任务断点续跑
- 多任务并行与任务队列
- 团队/组织策略（审批策略集中下发）
- 国际化（中/英）

## 风险与依赖

| 风险 | 缓解 |
|---|---|
| UIA 对自绘应用（Qt/游戏）支持差 | Vision 路线（v0.3）兜底 |
| chromiumoxide 依赖本机 Chrome | 检测缺失时提示安装或使用 Edge（同为 CDP） |
| LLM 输出不稳定导致规划失败 | 重试 + 明确 JSON schema + 失败终止而非盲试 |
