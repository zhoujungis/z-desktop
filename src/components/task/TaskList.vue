<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import StatusBadge from "@/components/task/StatusBadge.vue";
import type { Task } from "@/services/api";

const props = defineProps<{
  tasks: Task[];
  limit?: number;
}>();

const router = useRouter();

const shown = computed(() => (props.limit ? props.tasks.slice(0, props.limit) : props.tasks));

function fmtTime(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

function open(task: Task) {
  void router.push(`/task/${task.id}`);
}
</script>

<template>
  <ul class="divide-y divide-surface-border">
    <li v-for="task in shown" :key="task.id">
      <button
        type="button"
        class="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-surface-overlay"
        @click="open(task)"
      >
        <div class="min-w-0 flex-1">
          <div class="truncate text-sm text-zinc-200">{{ task.title }}</div>
          <div class="mt-0.5 text-xs text-zinc-500">{{ fmtTime(task.created_at) }}</div>
        </div>
        <StatusBadge :status="task.status" />
      </button>
    </li>
  </ul>
</template>
