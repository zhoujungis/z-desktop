<script setup lang="ts">
import { ref, watch } from "vue";
import { useAgentStore } from "@/stores/agent";

const emit = defineEmits<{ send: [text: string] }>();

const agent = useAgentStore();
const text = ref("");
const textareaRef = ref<HTMLTextAreaElement | null>(null);

watch(
  () => agent.isRunning,
  (running) => {
    if (!running) text.value = "";
  },
);

function autoResize() {
  const el = textareaRef.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
}

function focus() {
  textareaRef.value?.focus();
}

function submit() {
  const t = text.value.trim();
  if (!t || agent.isRunning) return;
  text.value = "";
  autoResize();
  emit("send", t);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    submit();
  }
}

function stop() {
  void agent.stop();
}

defineExpose({ focus });
</script>

<template>
  <div class="card shadow-lg shadow-black/30">
    <!-- 上次任务失败：给出可见的错误提示，而不是只留在任务详情里 -->
    <div
      v-if="!agent.isRunning && agent.error"
      class="flex items-start gap-3 border-b border-danger/30 bg-danger/5 px-4 py-2.5"
      role="alert"
    >
      <span class="mt-0.5 h-2 w-2 shrink-0 rounded-full bg-danger" />
      <p class="selectable min-w-0 flex-1 break-words text-xs text-danger">{{ agent.error }}</p>
      <button
        type="button"
        class="shrink-0 text-xs text-zinc-500 hover:text-zinc-300"
        title="关闭提示"
        @click="agent.clearError()"
      >
        关闭
      </button>
    </div>

    <!-- 运行中：状态条 + Stop -->
    <div v-if="agent.isRunning" class="flex items-center gap-3 px-4 py-3">
      <span class="h-2 w-2 shrink-0 animate-pulse rounded-full bg-accent" />
      <div class="min-w-0 flex-1">
        <div class="text-sm font-medium text-zinc-200">{{ agent.stateLabel }}</div>
        <p v-if="agent.error" class="truncate text-xs text-danger">{{ agent.error }}</p>
        <p v-else class="truncate text-xs text-zinc-500">
          Agent 正在工作，可随时停止
        </p>
      </div>
      <button type="button" class="btn-danger shrink-0" @click="stop">停止</button>
    </div>

    <!-- 空闲：输入框 -->
    <div v-else class="flex items-end gap-2 p-3">
      <textarea
        ref="textareaRef"
        v-model="text"
        rows="1"
        class="input max-h-40 flex-1 resize-none border-0 bg-transparent px-2 py-1.5 focus:ring-0"
        placeholder="让 Z 帮你做点什么…（Enter 发送，Shift+Enter 换行）"
        @input="autoResize"
        @keydown="onKeydown"
      />
      <button
        type="button"
        class="btn-primary shrink-0"
        :disabled="!text.trim()"
        @click="submit"
      >
        发送
      </button>
    </div>
  </div>
</template>
