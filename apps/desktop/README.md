# 预留：桌面应用入口

此目录为设计文档 #7 中预留的多应用入口位置（未来可拆分 desktop / tray / assistant 等子应用）。

当前唯一应用位于项目根：`src/`（Vue 3 前端）+ `src-tauri/`（Rust Core），由根目录 `package.json` 与 `src-tauri/tauri.conf.json` 驱动。

```bash
npm run tauri dev    # 开发模式
npm run tauri build  # 构建 Windows 安装包（NSIS / MSI）
```
