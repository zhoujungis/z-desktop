<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  status: string;
}>();

const MAP: Record<string, { label: string; cls: string }> = {
  pending: { label: "待开始", cls: "bg-zinc-500/15 text-zinc-400" },
  running: { label: "运行中", cls: "bg-accent-soft text-accent" },
  waiting_approval: { label: "等待审批", cls: "bg-warn/15 text-warn" },
  paused: { label: "已暂停", cls: "bg-zinc-500/15 text-zinc-400" },
  completed: { label: "已完成", cls: "bg-ok/15 text-ok" },
  failed: { label: "失败", cls: "bg-danger/15 text-danger" },
  cancelled: { label: "已取消", cls: "bg-zinc-500/15 text-zinc-500" },
};

const info = computed(() => MAP[props.status] ?? { label: props.status, cls: "bg-zinc-500/15 text-zinc-400" });
</script>

<template>
  <span
    class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium"
    :class="info.cls"
  >
    {{ info.label }}
  </span>
</template>
