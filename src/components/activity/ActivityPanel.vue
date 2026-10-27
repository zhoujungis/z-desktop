<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import EmptyState from "@/components/common/EmptyState.vue";
import { useAgentStore } from "@/stores/agent";
import { toolMetaOf } from "@/tools/registry";

const agent = useAgentStore();
const COLLAPSE_KEY = "z-desktop:activity-collapsed";
const collapsed = ref(readCollapsed());
const listRef = ref<HTMLElement | null>(null);

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSE_KEY) === "1";
  } catch {
    return false;
  }
}

watch(collapsed, (value) => {
  try {
    localStorage.setItem(COLLAPSE_KEY, value ? "1" : "0");
  } catch {
    // localStorage 不可用时忽略
  }
});

function metaOf(tool: string | undefined) {
  return toolMetaOf(tool);
}

watch(
  () => agent.activities.length,
  async () => {
    if (collapsed.value) return;
    await nextTick();
    const el = listRef.value;
    if (el) el.scrollTop = el.scrollHeight;
  },
);

function fmtTime(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

function typeCls(type: string): string {
  if (type.endsWith("_failed")) return "bg-danger";
  if (type.endsWith("_blocked")) return "bg-warn";
  if (type === "task_cancel_requested") return "bg-danger";
  if (type.endsWith("_completed")) return "bg-ok";
  return "bg-accent";
}
</script>

<template>
  <!-- 收起状态：右侧竖条 -->
  <button
    v-if="collapsed"
    type="button"
    class="flex w-9 shrink-0 flex-col items-center gap-2 border-l border-surface-border bg-surface-raised/60 py-3 text-xs text-zinc-400 hover:text-zinc-200"
    title="展开活动面板"
    @click="collapsed = false"
  >
    <span class="h-2 w-2 rounded-full" :class="agent.isRunning ? 'animate-pulse bg-accent' : 'bg-zinc-600'" />
    <span class="select-none [writing-mode:vertical-rl]">实时活动</span>
    <span class="text-[10px] text-zinc-500">{{ agent.activities.length }}</span>
  </button>

  <!-- 展开状态 -->
  <aside v-else class="flex w-80 shrink-0 flex-col border-l border-surface-border bg-surface-raised/60">
    <header class="flex items-center justify-between border-b border-surface-border px-4 py-3">
      <h2 class="text-sm font-semibold text-zinc-200">
        实时活动
        <span class="ml-1 text-xs font-normal text-zinc-500">{{ agent.activities.length }}</span>
      </h2>
      <button
        type="button"
        class="btn-ghost px-2 py-1 text-xs"
        title="收起面板"
        @click="collapsed = true"
      >
        收起
      </button>
    </header>

    <div ref="listRef" class="min-h-0 flex-1 overflow-y-auto px-4 py-3" aria-live="polite">
      <EmptyState
        v-if="agent.activities.length === 0"
        title="暂无活动"
        description="Agent 的每个动作都会实时显示在这里。"
      />
      <ul v-else class="space-y-3">
        <li v-for="(a, i) in agent.activities" :key="`${a.time}-${i}`" class="text-xs">
          <div class="flex items-center gap-2">
            <span class="h-1.5 w-1.5 shrink-0 rounded-full" :class="typeCls(a.type)" />
            <span class="shrink-0 font-mono text-[10px] text-zinc-500">{{ fmtTime(a.time) }}</span>
            <span
              v-if="a.tool"
              class="shrink-0 rounded bg-surface-overlay px-1.5 py-0.5 font-mono text-[10px] text-accent"
              :title="`${metaOf(a.tool).group} · ${metaOf(a.tool).label}`"
            >
              {{ metaOf(a.tool).icon }} {{ a.tool }}
            </span>
            <span class="truncate text-zinc-500">{{ a.type }}</span>
          </div>
          <p class="mt-1 break-words text-zinc-300">{{ a.summary }}</p>
          <details v-if="a.detail" class="mt-1">
            <summary class="cursor-pointer text-[10px] text-zinc-500 hover:text-zinc-400">详情</summary>
            <pre class="selectable mt-1 max-h-32 overflow-auto whitespace-pre-wrap break-words rounded bg-surface p-2 text-[10px] text-zinc-400">{{ a.detail }}</pre>
          </details>
        </li>
      </ul>
    </div>

    <footer class="border-t border-surface-border p-3">
      <button type="button" class="btn-ghost w-full text-xs" @click="agent.clearActivities()">
        清空活动
      </button>
    </footer>
  </aside>
</template>
