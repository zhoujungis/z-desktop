import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import type { AppSettings } from "./api";
import { isTauri } from "./tauri";

// ---------- 通用类型 ----------

export type ChatRole = "system" | "user" | "assistant" | "tool";

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

export interface ChatMessage {
  role: ChatRole;
  content: string;
  /** role === "tool" 时对应的 tool_call id */
  toolCallId?: string;
  /** role === "tool" 时对应的工具名（Gemini functionResponse 需要） */
  name?: string;
  /** role === "assistant" 携带的工具调用 */
  toolCalls?: ToolCall[];
}

export interface LlmTool {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
}

export interface ChatResult {
  text: string | null;
  toolCalls: ToolCall[];
}

/** 流式增量回调（设计文档 30 项清单 #005 Streaming） */
export type OnDelta = (delta: string) => void;

export interface LlmClient {
  chat(messages: ChatMessage[], tools: LlmTool[], signal?: AbortSignal): Promise<ChatResult>;
  chatStream(
    messages: ChatMessage[],
    tools: LlmTool[],
    onDelta: OnDelta,
    signal?: AbortSignal,
  ): Promise<ChatResult>;
}

// ---------- HTTP 基础 ----------

interface HttpInit {
  method: string;
  headers: Record<string, string>;
  body: string;
  signal?: AbortSignal;
}

async function httpRaw(url: string, init: HttpInit): Promise<Response> {
  if (!isTauri()) {
    throw new Error("浏览器开发模式下无法发起 LLM 请求，请在 Tauri 桌面应用中运行");
  }
  return tauriFetch(url, init);
}

async function httpJson(url: string, init: HttpInit): Promise<unknown> {
  const resp = await httpRaw(url, init);
  const raw = await resp.text();
  let data: unknown = null;
  try {
    data = raw ? (JSON.parse(raw) as unknown) : null;
  } catch {
    // 非 JSON 响应，保留原文用于报错
  }
  if (!resp.ok) {
    const detail =
      data !== null ? JSON.stringify(data).slice(0, 500) : raw.slice(0, 500) || "(空响应)";
    throw new Error(`LLM 请求失败（HTTP ${resp.status}）：${detail}`);
  }
  return data;
}

async function httpStream(url: string, init: HttpInit, label: string): Promise<Response> {
  const resp = await httpRaw(url, init);
  if (!resp.ok) {
    const raw = await resp.text().catch(() => "");
    throw new Error(`${label} 请求失败（HTTP ${resp.status}）：${raw.slice(0, 500) || "(空响应)"}`);
  }
  return resp;
}

/** 解析 SSE 响应中的 data: JSON 行（流式；导出仅供测试） */
export async function* sseData(resp: Response): AsyncGenerator<Record<string, unknown>> {
  if (!resp.body) throw new Error("当前环境不支持流式响应");
  const reader = resp.body.getReader();
  const decoder = new TextDecoder();
  let buf = "";
  const emitLine = (line: string): Record<string, unknown> | null => {
    if (!line.startsWith("data:")) return null;
    const payload = line.slice(5).trim();
    if (!payload || payload === "[DONE]") return null;
    try {
      const v: unknown = JSON.parse(payload);
      return typeof v === "object" && v !== null ? (v as Record<string, unknown>) : null;
    } catch {
      return null;
    }
  };
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    let idx: number;
    while ((idx = buf.indexOf("\n")) >= 0) {
      const obj = emitLine(buf.slice(0, idx).replace(/\r$/, ""));
      buf = buf.slice(idx + 1);
      if (obj) yield obj;
    }
  }
  const tail = emitLine(buf.replace(/\r$/, ""));
  if (tail) yield tail;
}

export function safeParseArgs(s: string | undefined | null): Record<string, unknown> {
  if (!s) return {};
  try {
    const v = JSON.parse(s) as unknown;
    return typeof v === "object" && v !== null && !Array.isArray(v)
      ? (v as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function asRecord(v: unknown): Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : {};
}

function requireApiKey(settings: AppSettings): string {
  if (settings.api_key) return settings.api_key;
  // Ollama 等本地服务通常不校验 API Key，允许留空
  if (settings.provider === "local") return "";
  throw new Error(`未配置 API Key，请前往「设置」填写 ${settings.provider} 的 api_key`);
}

function pickModel(settings: AppSettings): string {
  if (settings.model) return settings.model;
  switch (settings.provider) {
    case "anthropic":
      return "claude-3-5-sonnet-latest";
    case "gemini":
      return "gemini-1.5-flash";
    case "local":
      return "qwen2.5:7b";
    default:
      return "gpt-4o-mini";
  }
}

function baseUrlOf(settings: AppSettings, fallback: string): string {
  return (settings.base_url || fallback).replace(/\/+$/, "");
}

// ---------- OpenAI 兼容（openai / openrouter / custom / local） ----------

function fallbackBaseUrl(provider: string): string {
  switch (provider) {
    case "openrouter":
      return "https://openrouter.ai/api/v1";
    case "local":
      return "http://localhost:11434/v1";
    default:
      return "https://api.openai.com/v1";
  }
}

async function openAiCompatibleChat(
  settings: AppSettings,
  messages: ChatMessage[],
  tools: LlmTool[],
  onDelta?: OnDelta,
  signal?: AbortSignal,
): Promise<ChatResult> {
  const apiKey = requireApiKey(settings);
  const url = `${baseUrlOf(settings, fallbackBaseUrl(settings.provider))}/chat/completions`;
  const body: Record<string, unknown> = {
    model: pickModel(settings),
    messages: messages.map((m) => {
      if (m.role === "tool") {
        return { role: "tool", tool_call_id: m.toolCallId ?? m.name ?? "", content: m.content };
      }
      if (m.role === "assistant" && m.toolCalls && m.toolCalls.length > 0) {
        return {
          role: "assistant",
          content: m.content || null,
          tool_calls: m.toolCalls.map((c) => ({
            id: c.id,
            type: "function",
            function: { name: c.name, arguments: JSON.stringify(c.arguments ?? {}) },
          })),
        };
      }
      return { role: m.role, content: m.content };
    }),
  };
  if (tools.length > 0) {
    body.tools = tools.map((t) => ({
      type: "function",
      function: { name: t.name, description: t.description, parameters: t.parameters },
    }));
  }
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (apiKey) headers.authorization = `Bearer ${apiKey}`;
  const init: HttpInit = {
    method: "POST",
    headers,
    body: JSON.stringify(body),
    signal,
  };

  // 流式分支（SSE）
  if (onDelta) {
    const streamInit: HttpInit = {
      method: "POST",
      headers: init.headers,
      body: JSON.stringify({ ...body, stream: true }),
      signal,
    };
    const resp = await httpStream(url, streamInit, "OpenAI 兼容");
    let text: string | null = null;
    const calls = new Map<number, ToolCall>();
    for await (const chunk of sseData(resp)) {
      const choices = Array.isArray(chunk.choices) ? chunk.choices : [];
      const delta = asRecord(asRecord(choices[0]).delta);
      if (typeof delta.content === "string" && delta.content) {
        text = text ? text + delta.content : delta.content;
        onDelta(delta.content);
      }
      const rawCalls = Array.isArray(delta.tool_calls) ? delta.tool_calls : [];
      for (let i = 0; i < rawCalls.length; i++) {
        const rc = asRecord(rawCalls[i]);
        const idx = typeof rc.index === "number" ? rc.index : i;
        const fn = asRecord(rc.function);
        const cur = calls.get(idx) ?? {
          id: `call_${idx}`,
          name: "",
          arguments: {} as Record<string, unknown>,
        };
        let rawArgs = (cur as unknown as { _raw?: string })._raw ?? "";
        if (typeof rc.id === "string" && rc.id) cur.id = rc.id;
        if (typeof fn.name === "string" && fn.name) cur.name = fn.name;
        if (typeof fn.arguments === "string" && fn.arguments) rawArgs += fn.arguments;
        (cur as unknown as { _raw?: string })._raw = rawArgs;
        calls.set(idx, cur);
      }
    }
    const toolCalls: ToolCall[] = [...calls.entries()]
      .sort(([a], [b]) => a - b)
      .map(([, c]) => ({ id: c.id, name: c.name, arguments: safeParseArgs((c as unknown as { _raw?: string })._raw) }))
      .filter((c) => c.name);
    return { text, toolCalls };
  }

  const data = asRecord(await httpJson(url, init));
  const choices = Array.isArray(data.choices) ? data.choices : [];
  const msg = asRecord(choices[0]?.message);
  const rawCalls = Array.isArray(msg.tool_calls) ? msg.tool_calls : [];
  const toolCalls: ToolCall[] = [];
  for (let i = 0; i < rawCalls.length; i++) {
    const tc = asRecord(rawCalls[i]);
    const fn = asRecord(tc.function);
    const name = typeof fn.name === "string" ? fn.name : "";
    if (!name) continue;
    toolCalls.push({
      id: typeof tc.id === "string" && tc.id ? tc.id : `call_${i}`,
      name,
      arguments: safeParseArgs(typeof fn.arguments === "string" ? fn.arguments : null),
    });
  }
  const text = typeof msg.content === "string" && msg.content ? msg.content : null;
  return { text, toolCalls };
}

// ---------- Anthropic ----------

type AnthropicBlock = Record<string, unknown>;

async function anthropicChat(
  settings: AppSettings,
  messages: ChatMessage[],
  tools: LlmTool[],
  onDelta?: OnDelta,
  signal?: AbortSignal,
): Promise<ChatResult> {
  const apiKey = requireApiKey(settings);
  let base = baseUrlOf(settings, "https://api.anthropic.com");
  if (base.endsWith("/v1")) base = base.slice(0, -3);
  const url = `${base}/v1/messages`;

  const system = messages
    .filter((m) => m.role === "system")
    .map((m) => m.content)
    .join("\n\n");

  const converted: Array<{ role: "user" | "assistant"; content: string | AnthropicBlock[] }> = [];
  for (const m of messages) {
    if (m.role === "system") continue;
    if (m.role === "tool") {
      const block: AnthropicBlock = {
        type: "tool_result",
        tool_use_id: m.toolCallId ?? "",
        content: m.content,
      };
      const last = converted[converted.length - 1];
      if (last && last.role === "user" && Array.isArray(last.content)) {
        last.content.push(block);
      } else {
        converted.push({ role: "user", content: [block] });
      }
      continue;
    }
    if (m.role === "assistant" && m.toolCalls && m.toolCalls.length > 0) {
      const blocks: AnthropicBlock[] = [];
      if (m.content) blocks.push({ type: "text", text: m.content });
      for (const c of m.toolCalls) {
        blocks.push({ type: "tool_use", id: c.id, name: c.name, input: c.arguments ?? {} });
      }
      converted.push({ role: "assistant", content: blocks });
      continue;
    }
    converted.push({ role: m.role === "assistant" ? "assistant" : "user", content: m.content });
  }

  const body: Record<string, unknown> = {
    model: pickModel(settings),
    max_tokens: 4096,
    messages: converted,
  };
  if (system) body.system = system;
  if (tools.length > 0) {
    body.tools = tools.map((t) => ({
      name: t.name,
      description: t.description,
      input_schema: t.parameters,
    }));
  }

  const init: HttpInit = {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-api-key": apiKey,
      "anthropic-version": "2023-06-01",
    },
    body: JSON.stringify(body),
    signal,
  };

  // 流式分支（SSE：content_block_delta / content_block_start）
  if (onDelta) {
    const streamInit: HttpInit = {
      method: "POST",
      headers: init.headers,
      body: JSON.stringify({ ...body, stream: true }),
      signal,
    };
    const resp = await httpStream(url, streamInit, "Anthropic");
    let text: string | null = null;
    const partialCalls = new Map<number, { id: string; name: string; raw: string }>();
    for await (const ev of sseData(resp)) {
      const type = typeof ev.type === "string" ? ev.type : "";
      if (type === "content_block_delta") {
        const delta = asRecord(ev.delta);
        if (delta.type === "text_delta" && typeof delta.text === "string" && delta.text) {
          text = text ? text + delta.text : delta.text;
          onDelta(delta.text);
        } else if (delta.type === "input_json_delta" && typeof delta.partial_json === "string") {
          const cur = partialCalls.get(typeof ev.index === "number" ? ev.index : -1);
          if (cur) cur.raw += delta.partial_json;
        }
      } else if (type === "content_block_start") {
        const block = asRecord(ev.content_block);
        if (block.type === "tool_use" && typeof block.name === "string") {
          partialCalls.set(typeof ev.index === "number" ? ev.index : partialCalls.size, {
            id: typeof block.id === "string" && block.id ? block.id : `call_${partialCalls.size}`,
            name: block.name,
            raw: "",
          });
        }
      }
    }
    const toolCalls: ToolCall[] = [...partialCalls.values()]
      .filter((c) => c.name)
      .map((c) => ({ id: c.id, name: c.name, arguments: safeParseArgs(c.raw) }));
    return { text, toolCalls };
  }

  const data = asRecord(await httpJson(url, init));

  let text: string | null = null;
  const toolCalls: ToolCall[] = [];
  const blocks = Array.isArray(data.content) ? data.content : [];
  for (const raw of blocks) {
    const block = asRecord(raw);
    if (block.type === "text" && typeof block.text === "string" && block.text) {
      text = text ? `${text}\n${block.text}` : block.text;
    } else if (block.type === "tool_use" && typeof block.name === "string") {
      toolCalls.push({
        id: typeof block.id === "string" && block.id ? block.id : `call_${toolCalls.length}`,
        name: block.name,
        arguments: asRecord(block.input),
      });
    }
  }
  return { text, toolCalls };
}

// ---------- Gemini ----------

/** 把通用消息转换为 Gemini contents/body（非流式与流式共用；导出仅供测试） */
export function buildGeminiBody(messages: ChatMessage[], tools: LlmTool[]): Record<string, unknown> {
  const system = messages
    .filter((m) => m.role === "system")
    .map((m) => m.content)
    .join("\n\n");

  const contents: Array<{ role: "user" | "model"; parts: AnthropicBlock[] }> = [];
  for (const m of messages) {
    if (m.role === "system") continue;
    if (m.role === "tool") {
      contents.push({
        role: "user",
        parts: [
          {
            functionResponse: {
              name: m.name ?? m.toolCallId ?? "tool",
              response: { result: m.content },
            },
          },
        ],
      });
      continue;
    }
    if (m.role === "assistant" && m.toolCalls && m.toolCalls.length > 0) {
      const parts: AnthropicBlock[] = [];
      if (m.content) parts.push({ text: m.content });
      for (const c of m.toolCalls) {
        parts.push({ functionCall: { name: c.name, args: c.arguments ?? {} } });
      }
      contents.push({ role: "model", parts });
      continue;
    }
    contents.push({ role: m.role === "assistant" ? "model" : "user", parts: [{ text: m.content }] });
  }

  const body: Record<string, unknown> = { contents };
  if (system) body.systemInstruction = { parts: [{ text: system }] };
  if (tools.length > 0) {
    body.tools = [
      {
        functionDeclarations: tools.map((t) => ({
          name: t.name,
          description: t.description,
          parameters: t.parameters,
        })),
      },
    ];
  }
  return body;
}

function geminiUrl(settings: AppSettings, streaming: boolean): string {
  const apiKey = requireApiKey(settings);
  const base = baseUrlOf(settings, "https://generativelanguage.googleapis.com");
  const model = pickModel(settings);
  return streaming
    ? `${base}/v1beta/models/${encodeURIComponent(model)}:streamGenerateContent?alt=sse&key=${encodeURIComponent(apiKey)}`
    : `${base}/v1beta/models/${encodeURIComponent(model)}:generateContent?key=${encodeURIComponent(apiKey)}`;
}

async function geminiChat(
  settings: AppSettings,
  messages: ChatMessage[],
  tools: LlmTool[],
  signal?: AbortSignal,
): Promise<ChatResult> {
  const url = geminiUrl(settings, false);
  const body = buildGeminiBody(messages, tools);

  const data = asRecord(
    await httpJson(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      signal,
    }),
  );

  let text: string | null = null;
  const toolCalls: ToolCall[] = [];
  const candidates = Array.isArray(data.candidates) ? data.candidates : [];
  const content = asRecord(candidates[0]?.content);
  const parts = Array.isArray(content.parts) ? content.parts : [];
  for (const raw of parts) {
    const part = asRecord(raw);
    if (typeof part.text === "string" && part.text) {
      text = text ? `${text}\n${part.text}` : part.text;
    }
    const fc = asRecord(part.functionCall);
    if (typeof fc.name === "string" && fc.name) {
      toolCalls.push({
        id: `gf_${fc.name}_${toolCalls.length}`,
        name: fc.name,
        arguments: asRecord(fc.args),
      });
    }
  }
  return { text, toolCalls };
}

/** Gemini SSE 流式解析：每个 chunk 与 generateContent 响应结构一致 */
async function geminiStreamResult(
  body: Record<string, unknown>,
  url: string,
  onDelta: OnDelta,
  signal?: AbortSignal,
): Promise<ChatResult> {
  const resp = await httpStream(
    url,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      signal,
    },
    "Gemini",
  );
  let text: string | null = null;
  const toolCalls: ToolCall[] = [];
  for await (const chunk of sseData(resp)) {
    const candidates = Array.isArray(chunk.candidates) ? chunk.candidates : [];
    const content = asRecord(candidates[0]?.content);
    const parts = Array.isArray(content.parts) ? content.parts : [];
    for (const raw of parts) {
      const part = asRecord(raw);
      if (typeof part.text === "string" && part.text) {
        text = text ? text + part.text : part.text;
        onDelta(part.text);
      }
      const fc = asRecord(part.functionCall);
      if (typeof fc.name === "string" && fc.name) {
        toolCalls.push({
          id: `gf_${fc.name}_${toolCalls.length}`,
          name: fc.name,
          arguments: asRecord(fc.args),
        });
      }
    }
  }
  return { text, toolCalls };
}

// ---------- 工厂 ----------

/** 按 settings.provider 创建对应 LLM 客户端 */
export function createLlmClient(settings: AppSettings): LlmClient {
  switch (settings.provider) {
    case "anthropic":
      return {
        chat: (m, t, signal) => anthropicChat(settings, m, t, undefined, signal),
        chatStream: (m, t, onDelta, signal) =>
          anthropicChat(settings, m, t, onDelta, signal),
      };
    case "gemini":
      return {
        chat: (m, t, signal) => geminiChat(settings, m, t, signal),
        chatStream: (m, t, onDelta, signal) =>
          geminiStreamResult(buildGeminiBody(m, t), geminiUrl(settings, true), onDelta, signal),
      };
    default:
      // openai / openrouter / custom / local 均走 OpenAI 兼容协议
      return {
        chat: (m, t, signal) => openAiCompatibleChat(settings, m, t, undefined, signal),
        chatStream: (m, t, onDelta, signal) =>
          openAiCompatibleChat(settings, m, t, onDelta, signal),
      };
  }
}
