import { invoke } from "@tauri-apps/api/core";

/** 是否运行在 Tauri 桌面环境中 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** invoke 包装：浏览器 dev 模式下给出友好错误 */
export async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error(
      `当前处于浏览器开发模式，无法调用 Rust 命令「${cmd}」。请使用 npm run tauri dev 启动桌面应用。`,
    );
  }
  return invoke<T>(cmd, args);
}
