import { describe, expect, it } from "vitest";
import { buildPlannerMessages, toLlmTools } from "./planner";
import type { ToolDef } from "@/services/api";
import type { AgentContext } from "./types";

function def(overrides: Partial<ToolDef> = {}): ToolDef {
  return {
    name: "filesystem.read",
    description: "读取文件",
    input_schema: { type: "object", properties: { path: { type: "string" } } },
    risk_level: "safe",
    ...overrides,
  };
}

function ctx(overrides: Partial<AgentContext> = {}): AgentContext {
  return {
    projectName: null,
    projectPath: null,
    projectTech: null,
    memories: [],
    now: "2026-10-27 10:00",
    ...overrides,
  };
}

describe("toLlmTools", () => {
  it("转换名称/描述并附加风险等级", () => {
    const out = toLlmTools([def()]);
    expect(out).toHaveLength(1);
    expect(out[0].name).toBe("filesystem.read");
    expect(out[0].description).toContain("读取文件");
    expect(out[0].description).toContain("safe");
    expect(out[0].parameters).toEqual({
      type: "object",
      properties: { path: { type: "string" } },
    });
  });

  it("非对象 schema 回退为空 object schema", () => {
    const out = toLlmTools([def({ input_schema: "bad" })]);
    expect(out[0].parameters).toEqual({ type: "object", properties: {} });
    const out2 = toLlmTools([def({ input_schema: [1, 2] })]);
    expect(out2[0].parameters).toEqual({ type: "object", properties: {} });
  });

  it("兼容 riskLevel / risk_level 两种键名", () => {
    expect(toLlmTools([def({ risk_level: undefined, riskLevel: "high" })])[0].description).toContain("high");
    expect(toLlmTools([def({ risk_level: undefined })])[0].description).toContain("medium");
  });
});

describe("buildPlannerMessages", () => {
  it("包含工具清单、上下文与记忆", () => {
    const c = ctx({
      projectName: "Z Desktop",
      projectPath: "/workspace/z-desktop",
      projectTech: "Rust",
      memories: [
        { id: "1", type: "fact", content: "用户偏好深色主题", source: null, created_at: 0 },
      ],
    });
    const msgs = buildPlannerMessages("整理文件", c, toLlmTools([def()]));
    expect(msgs).toHaveLength(2);
    expect(msgs[0].role).toBe("system");
    expect(msgs[1]).toEqual({ role: "user", content: "整理文件" });

    const sys = msgs[0].content;
    expect(sys).toContain("filesystem.read");
    expect(sys).toContain("Z Desktop");
    expect(sys).toContain("/workspace/z-desktop");
    expect(sys).toContain("Rust");
    expect(sys).toContain("用户偏好深色主题");
    expect(sys).toContain("2026-10-27 10:00");
  });

  it("无记忆时给出占位说明", () => {
    const sys = buildPlannerMessages("hi", ctx(), [])[0].content;
    expect(sys).toContain("暂无相关记忆");
  });
});
