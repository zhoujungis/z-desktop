<script setup lang="ts">
import { onMounted, onUnmounted, reactive, ref } from "vue";
import SectionCard from "@/components/common/SectionCard.vue";
import { useSettingsStore } from "@/stores/settings";

const settingsStore = useSettingsStore();

const PROVIDERS = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "gemini", label: "Gemini" },
  { value: "openrouter", label: "OpenRouter" },
  { value: "custom", label: "自定义（OpenAI 兼容）" },
  { value: "local", label: "本地模型（Ollama 等）" },
];

const form = reactive({
  provider: "openai",
  base_url: "",
  api_key: "",
  model: "",
  auto_approve_safe: true,
  auto_approve_low: false,
  auto_approve_medium: false,
  shell_safe_auto_exec: true,
  max_steps: 30,
});

const saving = ref(false);
const message = ref<string | null>(null);
const messageOk = ref(false);
let timer: number | undefined;

function baseUrlPlaceholder(): string {
  switch (form.provider) {
    case "openrouter":
      return "https://openrouter.ai/api/v1";
    case "anthropic":
      return "https://api.anthropic.com";
    case "local":
      return "http://localhost:11434/v1";
    default:
      return "https://api.openai.com/v1（留空使用默认）";
  }
}

function modelPlaceholder(): string {
  switch (form.provider) {
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

async function save() {
  saving.value = true;
  message.value = null;
  // 规整数值字段后再保存
  form.max_steps = Math.max(1, Math.min(200, Math.floor(form.max_steps) || 30));
  try {
    Object.assign(settingsStore.settings, form);
    await settingsStore.save();
    message.value = "设置已保存，立即生效";
    messageOk.value = true;
    window.clearTimeout(timer);
    timer = window.setTimeout(() => (message.value = null), 2500);
  } catch (e) {
    message.value = e instanceof Error ? e.message : String(e);
    messageOk.value = false;
  } finally {
    saving.value = false;
  }
}

onMounted(async () => {
  await settingsStore.loadOrDefault();
  Object.assign(form, settingsStore.settings);
});

onUnmounted(() => window.clearTimeout(timer));
</script>

<template>
  <div class="mx-auto max-w-2xl space-y-4 px-6 py-8">
    <h1 class="text-xl font-semibold text-zinc-100">设置</h1>

    <p v-if="message" class="rounded-lg px-3 py-2 text-sm"
      :class="messageOk ? 'bg-ok/10 text-ok' : 'bg-danger/10 text-danger'">
      {{ message }}
    </p>

    <SectionCard title="LLM 提供商" subtitle="api_key / model / base_url 均来自此处配置">
      <div class="space-y-3">
        <label class="block">
          <span class="mb-1 block text-xs text-zinc-400">Provider</span>
          <select v-model="form.provider" class="input">
            <option v-for="p in PROVIDERS" :key="p.value" :value="p.value">{{ p.label }}</option>
          </select>
        </label>
        <label class="block">
          <span class="mb-1 block text-xs text-zinc-400">Base URL</span>
          <input v-model="form.base_url" class="input font-mono text-xs" :placeholder="baseUrlPlaceholder()" />
        </label>
        <label class="block">
          <span class="mb-1 block text-xs text-zinc-400">API Key</span>
          <input
            v-model="form.api_key"
            type="password"
            autocomplete="off"
            class="input font-mono text-xs"
            placeholder="sk-…（本地模型可留空）"
          />
        </label>
        <label class="block">
          <span class="mb-1 block text-xs text-zinc-400">模型</span>
          <input v-model="form.model" class="input font-mono text-xs" :placeholder="modelPlaceholder()" />
        </label>
      </div>
    </SectionCard>

    <SectionCard title="权限与自动批准" subtitle="控制哪些风险等级的操作无需逐次审批">
      <ul class="space-y-3">
        <li class="flex items-center justify-between gap-4">
          <div>
            <div class="text-sm text-zinc-200">自动批准 safe 操作</div>
            <p class="text-xs text-zinc-500">读取文件、列目录、截图等只读操作</p>
          </div>
          <button
            type="button"
            role="switch"
            :aria-checked="form.auto_approve_safe"
            class="relative h-5 w-9 shrink-0 rounded-full transition-colors"
            :class="form.auto_approve_safe ? 'bg-accent' : 'bg-zinc-600'"
            @click="form.auto_approve_safe = !form.auto_approve_safe"
          >
            <span
              class="absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white transition-transform"
              :class="form.auto_approve_safe ? 'translate-x-4' : ''"
            />
          </button>
        </li>
        <li class="flex items-center justify-between gap-4">
          <div>
            <div class="text-sm text-zinc-200">自动批准 low 操作</div>
            <p class="text-xs text-zinc-500">创建目录、复制文件、启动应用等</p>
          </div>
          <button
            type="button"
            role="switch"
            :aria-checked="form.auto_approve_low"
            class="relative h-5 w-9 shrink-0 rounded-full transition-colors"
            :class="form.auto_approve_low ? 'bg-accent' : 'bg-zinc-600'"
            @click="form.auto_approve_low = !form.auto_approve_low"
          >
            <span
              class="absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white transition-transform"
              :class="form.auto_approve_low ? 'translate-x-4' : ''"
            />
          </button>
        </li>
        <li class="flex items-center justify-between gap-4">
          <div>
            <div class="text-sm text-zinc-200">自动批准 medium 操作</div>
            <p class="text-xs text-zinc-500">写入文件、执行 Shell 命令等（谨慎开启）</p>
          </div>
          <button
            type="button"
            role="switch"
            :aria-checked="form.auto_approve_medium"
            class="relative h-5 w-9 shrink-0 rounded-full transition-colors"
            :class="form.auto_approve_medium ? 'bg-accent' : 'bg-zinc-600'"
            @click="form.auto_approve_medium = !form.auto_approve_medium"
          >
            <span
              class="absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white transition-transform"
              :class="form.auto_approve_medium ? 'translate-x-4' : ''"
            />
          </button>
        </li>
        <li class="flex items-center justify-between gap-4">
          <div>
            <div class="text-sm text-zinc-200">安全命令自动执行</div>
            <p class="text-xs text-zinc-500">命中白名单的 Shell 命令跳过审批（设计文档 #19）</p>
          </div>
          <button
            type="button"
            role="switch"
            :aria-checked="form.shell_safe_auto_exec"
            class="relative h-5 w-9 shrink-0 rounded-full transition-colors"
            :class="form.shell_safe_auto_exec ? 'bg-accent' : 'bg-zinc-600'"
            @click="form.shell_safe_auto_exec = !form.shell_safe_auto_exec"
          >
            <span
              class="absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white transition-transform"
              :class="form.shell_safe_auto_exec ? 'translate-x-4' : ''"
            />
          </button>
        </li>
      </ul>
    </SectionCard>

    <SectionCard title="Agent Loop" subtitle="单次任务的最大工具调用步数">
      <label class="block max-w-xs">
        <span class="mb-1 block text-xs text-zinc-400">max_steps（1 - 200）</span>
        <input v-model.number="form.max_steps" type="number" min="1" max="200" class="input" />
      </label>
    </SectionCard>

    <div class="flex justify-end">
      <button type="button" class="btn-primary" :disabled="saving" @click="save">
        {{ saving ? "保存中…" : "保存设置" }}
      </button>
    </div>
  </div>
</template>
