import { describe, expect, it } from "vitest";
import { buildGeminiBody, safeParseArgs, sseData, type ChatMessage, type LlmTool } from "./llm";

// ---------- sseData ----------

/** 构造 SSE Response（Node 18+ 全局 ReadableStream） */
function sseResponse(chunks: string[]): Response {
  const encoder = new TextEncoder();
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      for (const c of chunks) controller.enqueue(encoder.encode(c));
      controller.close();
    },
  });
  return new Response(stream);
}

async function collect(resp: Response): Promise<Record<string, unknown>[]> {
  const out: Record<string, unknown>[] = [];
  for await (const obj of sseData(resp)) out.push(obj);
  return out;
}

describe("sseData 流式解析", () => {
  it("解析多行 data 并跳过 [DONE]", async () => {
    const resp = sseResponse([
      'data: {"a":1}\ndata: {"b":2}\n\ndata: [DONE]\ndata: {"c":3}\n',
    ]);
    const objs = await collect(resp);
    expect(objs).toEqual([{ a: 1 }, { b: 2 }, { c: 3 }]);
  });

  it("处理跨 chunk 分割的行", async () => {
    const resp = sseResponse([
      'data: {"x":"hel',
      'lo"}\ndata: {"y":',
      "2}\n",
    ]);
    const objs = await collect(resp);
    expect(objs).toEqual([{ x: "hello" }, { y: 2 }]);
  });

  it("处理 \\r\\n 换行与无尾部换行", async () => {
    const resp = sseResponse(['data: {"a":1}\r\n', 'data: {"b":2}']);
    const objs = await collect(resp);
    expect(objs).toEqual([{ a: 1 }, { b: 2 }]);
  });

  it("忽略非 data 行（event:/注释）与非法 JSON", async () => {
    const resp = sseResponse([
      ": keepalive\nevent: delta\ndata: not-json\ndata: {\"ok\":true}\n",
    ]);
    const objs = await collect(resp);
    expect(objs).toEqual([{ ok: true }]);
  });

  it("无 body 时抛错", async () => {
    const fake = { body: null } as unknown as Response;
    await expect(collect(fake)).rejects.toThrow("流式");
  });
});

// ---------- safeParseArgs ----------

describe("safeParseArgs", () => {
  it("正常 JSON 对象", () => {
    expect(safeParseArgs('{"path":"/tmp/a"}')).toEqual({ path: "/tmp/a" });
  });
  it("空值返回空对象", () => {
    expect(safeParseArgs("")).toEqual({});
    expect(safeParseArgs(null)).toEqual({});
    expect(safeParseArgs(undefined)).toEqual({});
  });
  it("非法 JSON 与数组返回空对象", () => {
    expect(safeParseArgs("not-json{")).toEqual({});
    expect(safeParseArgs("[1,2]")).toEqual({});
  });
});

// ---------- buildGeminiBody ----------

const tools: LlmTool[] = [
  { name: "filesystem.read", description: "读文件", parameters: { type: "object", properties: {} } },
];

describe("buildGeminiBody 协议转换", () => {
  it("system 消息进入 systemInstruction", () => {
    const msgs: ChatMessage[] = [
      { role: "system", content: "你是助手" },
      { role: "user", content: "你好" },
    ];
    const body = buildGeminiBody(msgs, []);
    expect(body.systemInstruction).toEqual({ parts: [{ text: "你是助手" }] });
    expect(body.contents).toEqual([{ role: "user", parts: [{ text: "你好" }] }]);
  });

  it("assistant 的 toolCalls 转为 functionCall，tool 结果转为 functionResponse", () => {
    const msgs: ChatMessage[] = [
      {
        role: "assistant",
        content: "我来读取",
        toolCalls: [{ id: "call_1", name: "filesystem.read", arguments: { path: "/a" } }],
      },
      { role: "tool", content: "文件内容", toolCallId: "call_1", name: "filesystem.read" },
    ];
    const body = buildGeminiBody(msgs, []);
    const contents = body.contents as Array<{ role: string; parts: Record<string, unknown>[] }>;
    expect(contents[0].role).toBe("model");
    expect(contents[0].parts).toEqual([
      { text: "我来读取" },
      { functionCall: { name: "filesystem.read", args: { path: "/a" } } },
    ]);
    expect(contents[1].role).toBe("user");
    expect(contents[1].parts[0]).toEqual({
      functionResponse: { name: "filesystem.read", response: { result: "文件内容" } },
    });
  });

  it("工具声明转为 functionDeclarations", () => {
    const body = buildGeminiBody([], tools);
    expect(body.tools).toEqual([
      {
        functionDeclarations: [
          { name: "filesystem.read", description: "读文件", parameters: { type: "object", properties: {} } },
        ],
      },
    ]);
  });

  it("多段 system 合并", () => {
    const msgs: ChatMessage[] = [
      { role: "system", content: "A" },
      { role: "system", content: "B" },
    ];
    const body = buildGeminiBody(msgs, []);
    expect(body.systemInstruction).toEqual({ parts: [{ text: "A\n\nB" }] });
    expect(body.contents).toEqual([]);
  });
});
