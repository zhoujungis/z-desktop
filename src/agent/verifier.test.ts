import { describe, expect, it, vi } from "vitest";
import { parseVerdict, verify } from "./verifier";
import type { LlmClient } from "@/services/llm";

describe("parseVerdict", () => {
  it("accepts a clear yes in English and Chinese", () => {
    expect(parseVerdict("yes，任务已完成")).toBe(true);
    expect(parseVerdict("Yes. All files were moved.")).toBe(true);
    expect(parseVerdict("是，已经完成")).toBe(true);
    expect(parseVerdict("通过：输出符合目标")).toBe(true);
  });

  it("rejects a clear no", () => {
    expect(parseVerdict("no，还缺少最后一步")).toBe(false);
    expect(parseVerdict("No. The task is incomplete.")).toBe(false);
    expect(parseVerdict("否，未完成")).toBe(false);
    expect(parseVerdict("未通过：文件不存在")).toBe(false);
  });

  it("does not flip to pass when no appears first", () => {
    expect(parseVerdict("no, but it could be yes later")).toBe(false);
    expect(parseVerdict("No — although Yes was mentioned, verification failed")).toBe(false);
  });

  it("ignores leading markdown decoration", () => {
    expect(parseVerdict("**Yes**, done")).toBe(true);
    expect(parseVerdict("`no`")).toBe(false);
  });

  it("treats empty output as not passed", () => {
    expect(parseVerdict("")).toBe(false);
    expect(parseVerdict(null)).toBe(false);
    expect(parseVerdict(undefined)).toBe(false);
  });
});

describe("verify", () => {
  it("returns the verifier verdict", async () => {
    const llm: LlmClient = {
      chat: vi.fn(async () => ({ text: "yes, done", toolCalls: [] })),
      chatStream: vi.fn(async () => ({ text: null, toolCalls: [] })),
    };
    const result = await verify(llm, "整理文件", "已整理完毕");
    expect(result.passed).toBe(true);
    expect(llm.chat).toHaveBeenCalledOnce();
  });

  it("fails open when the verifier call errors", async () => {
    const llm: LlmClient = {
      chat: vi.fn(async () => {
        throw new Error("network down");
      }),
      chatStream: vi.fn(async () => ({ text: null, toolCalls: [] })),
    };
    const result = await verify(llm, "task", "output");
    expect(result.passed).toBe(true);
    expect(result.reason).toContain("network down");
  });
});
