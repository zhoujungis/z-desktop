<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import EmptyState from "@/components/common/EmptyState.vue";
import type { Message } from "@/services/api";

const props = defineProps<{
  messages: Message[];
}>();

const listRef = ref<HTMLElement | null>(null);

watch(
  () => props.messages.length,
  async () => {
    await nextTick();
    const el = listRef.value;
    if (el) el.scrollTop = el.scrollHeight;
  },
);

function fmtTime(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}`;
}
</script>

<template>
  <div ref="listRef" class="min-h-0 flex-1 space-y-3 overflow-y-auto p-4">
    <EmptyState
      v-if="messages.length === 0"
      title="暂无消息"
      description="任务启动后，这里会显示你的任务描述与 Agent 的总结。"
    />
    <template v-else>
      <div
        v-for="m in messages"
        :key="m.id"
        class="flex"
        :class="m.role === 'user' ? 'justify-end' : 'justify-start'"
      >
        <div
          class="max-w-[85%] rounded-2xl px-3.5 py-2.5 text-sm leading-relaxed selectable"
          :class="
            m.role === 'user'
              ? 'rounded-br-md bg-accent text-white'
              : 'rounded-bl-md bg-surface-overlay text-zinc-200'
          "
        >
          <p class="whitespace-pre-wrap break-words">{{ m.content }}</p>
          <div
            class="mt-1 text-right text-[10px]"
            :class="m.role === 'user' ? 'text-white/60' : 'text-zinc-500'"
          >
            {{ fmtTime(m.created_at) }}
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
