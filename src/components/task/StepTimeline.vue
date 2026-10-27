<script lang="ts">
/** 时间线条目（步骤记录或从 activity 事件构建） */
export interface TimelineItem {
  status: "done" | "running" | "failed" | "blocked" | "cancelled" | "pending";
  tool: string | null;
  title: string;
  detail: string | null;
  time: number;
}

const DOT: Record<TimelineItem["status"], string> = {
  done: "bg-ok/15 text-ok",
  running: "bg-accent-soft text-accent",
  failed: "bg-danger/15 text-danger",
  blocked: "bg-warn/15 text-warn",
  cancelled: "bg-zinc-500/15 text-zinc-400",
  pending: "bg-zinc-500/15 text-zinc-500",
};

const GLYPH: Record<TimelineItem["status"], string> = {
  done: "✓",
  running: "●",
  failed: "✗",
  blocked: "!",
  cancelled: "⊘",
  pending: "○",
};

function fmtTime(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
</script>

<script setup lang="ts">
import EmptyState from "@/components/common/EmptyState.vue";

defineProps<{
  items: TimelineItem[];
}>();
</script>

<template>
  <EmptyState
    v-if="items.length === 0"
    title="暂无执行步骤"
    description="任务启动后，每个工具调用都会出现在这条时间线上。"
  />
  <ol v-else class="relative space-y-1">
    <li v-for="(item, i) in items" :key="`${item.time}-${i}`" class="flex gap-3">
      <!-- 时间线轴 -->
      <div class="flex flex-col items-center">
        <span
          class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[11px] font-bold"
          :class="DOT[item.status]"
        >
          <span v-if="item.status === 'running'" class="h-2 w-2 animate-pulse rounded-full bg-accent" />
          <template v-else>{{ GLYPH[item.status] }}</template>
        </span>
        <span v-if="i < items.length - 1" class="w-px flex-1 bg-surface-border" />
      </div>
      <!-- 内容 -->
      <div class="min-w-0 flex-1 pb-3">
        <div class="flex items-baseline gap-2">
          <span class="truncate text-sm text-zinc-200">{{ item.title }}</span>
          <span class="ml-auto shrink-0 text-[10px] text-zinc-500">{{ fmtTime(item.time) }}</span>
        </div>
        <details v-if="item.detail" class="mt-1">
          <summary class="cursor-pointer text-xs text-zinc-500 hover:text-zinc-400">详情</summary>
          <pre class="selectable mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-surface p-2 text-xs text-zinc-400">{{ item.detail }}</pre>
        </details>
      </div>
    </li>
  </ol>
</template>
