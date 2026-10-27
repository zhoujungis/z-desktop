import { describe, expect, it } from "vitest";
import { ALL_TOOL_META, toolMetaOf } from "./registry";

/** Rust registry.rs::tool_definitions 的权威工具名单（44 个） */
const RUST_TOOLS = [
  "browser.back", "browser.click", "browser.download", "browser.extract",
  "browser.forward", "browser.goto", "browser.open", "browser.screenshot",
  "browser.scroll", "browser.search", "browser.type",
  "clipboard.read", "clipboard.write",
  "filesystem.copy", "filesystem.delete", "filesystem.list", "filesystem.mkdir",
  "filesystem.move", "filesystem.read", "filesystem.rename", "filesystem.search",
  "filesystem.stat", "filesystem.write",
  "process.inspect", "process.kill", "process.list", "process.start",
  "screenshot.full", "screenshot.region", "screenshot.window",
  "shell.exec",
  "ui.click", "ui.find", "ui.getText", "ui.getValue", "ui.inspect", "ui.select", "ui.type",
  "window.close", "window.find", "window.focus", "window.list", "window.maximize", "window.minimize",
];

describe("前端工具元数据与 Rust 注册表一致性", () => {
  it("ALL_TOOL_META 应覆盖全部 44 个 Rust 工具且无多余", () => {
    const frontend = ALL_TOOL_META.map((m) => m.tool).sort();
    const rust = [...RUST_TOOLS].sort();
    expect(frontend).toEqual(rust);
  });

  it("不应有重复工具名", () => {
    const names = ALL_TOOL_META.map((m) => m.tool);
    expect(new Set(names).size).toBe(names.length);
  });

  it("每条元数据字段完整", () => {
    for (const m of ALL_TOOL_META) {
      expect(m.label.trim(), `${m.tool} 缺中文名`).not.toBe("");
      expect(m.icon.trim(), `${m.tool} 缺图标`).not.toBe("");
      expect(m.group.trim(), `${m.tool} 缺分组`).not.toBe("");
    }
  });
});

describe("toolMetaOf", () => {
  it("命中已登记工具", () => {
    const m = toolMetaOf("filesystem.read");
    expect(m.label).toBe("读取文件");
    expect(m.group).toBe("文件系统");
  });

  it("未登记工具回退为通用元数据", () => {
    const m = toolMetaOf("future.tool");
    expect(m.icon).toBe("🔧");
    expect(m.label).toBe("future.tool");
    expect(m.group).toBe("其他");
  });

  it("空值回退", () => {
    expect(toolMetaOf(null).label).toBe("未知工具");
    expect(toolMetaOf(undefined).icon).toBe("🔧");
    expect(toolMetaOf("").group).toBe("其他");
  });
});
