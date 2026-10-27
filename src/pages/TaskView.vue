<script setup lang="ts">
import { computed, onUnmounted, watch } from "vue";
import { useRoute } from "vue-router";
import MessageList from "@/components/chat/MessageList.vue";
import EmptyState from "@/components/common/EmptyState.vue";
import SectionCard from "@/components/common/SectionCard.vue";
import StepTimeline, { type TimelineItem } from "@/components/task/StepTimeline.vue";
import StatusBadge from "@/components/task/StatusBadge.vue";
import TaskList from "@/components/task/TaskList.vue";
import type { ActivityPayload, EventRecord, TaskStep } from "@/services/api";
import { useAgentStore } from "@/stores/agent";
import { useTaskStore } from "@/stores/task";
import { toolMetaOf } from "@/tools/registry";

const route = useRoute();
const agent = useAgentStore();
const taskStore = useTaskStore();

const taskId = computed(() => {
  const raw = route.params.id;
  return Array.isArray(raw) ? raw[0] : raw;
});

const task = computed(() => taskStore.currentTask);
const messages = computed(() => (taskId.value ? taskStore.messagesByTask[taskId.value] ?? [] : []));
const isActiveTask = computed(() => agent.isRunning && agent.currentTaskId === taskId.value);

// ---------- 时间线构建 ----------

function markLastRunning(items: TimelineItem[], status: "done" | "failed", extra: string | null) {
  for (let i = items.length - 1; i >= 0; i--) {
    if (items[i].status === "running") {
      items[i].status = status;
      if (extra) items[i].detail = items[i].detail ? `${items[i].detail}\n→ ${extra}` : extra;
      return;
    }
  }
}

function stepsToTimeline(steps: TaskStep[]): TimelineItem[] {
  return steps.map((s) => ({
    status:
      s.status === "done" || s.status === "completed"
        ? "done"
        : s.status === "running"
          ? "running"
          : s.status === "failed"
            ? "failed"
            : s.status === "blocked"
              ? "blocked"
              : s.status === "cancelled"
                ? "cancelled"
                : "pending",
    tool: s.tool,
    title: s.tool
      ? `${toolMetaOf(s.tool).icon} ${toolMetaOf(s.tool).label}`
      : (s.input?.slice(0, 60) ?? "步骤"),
    detail: [s.input, s.output].filter((v): v is string => Boolean(v)).join("\n———\n") || null,
    time: s.created_at,
  }));
}

function eventsToTimeline(events: EventRecord[]): TimelineItem[] {
  const items: TimelineItem[] = [];
  for (const ev of [...events].reverse()) {
    if (ev.type === "tool_started") {
      items.push({ status: "running", tool: null, title: "执行工具", detail: ev.data, time: ev.created_at });
    } else if (ev.type === "tool_completed") {
      markLastRunning(items, "done", ev.data);
    } else if (ev.type === "tool_failed") {
      markLastRunning(items, "failed", ev.data);
    } else if (ev.type === "tool_blocked") {
      items.push({ status: "blocked", tool: null, title: "操作被拦截", detail: ev.data, time: ev.created_at });
    } else if (ev.type === "task_cancel_requested") {
      items.push({ status: "failed", tool: null, title: "用户停止任务", detail: ev.data, time: ev.created_at });
    }
  }
  return items;
}

function activitiesToTimeline(list: ActivityPayload[]): TimelineItem[] {
  const items: TimelineItem[] = [];
  for (const a of list) {
    if (a.type === "tool_started") {
      const title = a.tool
        ? `${toolMetaOf(a.tool).icon} ${toolMetaOf(a.tool).label}`
        : "执行工具";
      items.push({ status: "running", tool: a.tool, title, detail: a.detail, time: a.time });
    } else if (a.type === "tool_completed") {
      markLastRunning(items, "done", a.detail);
    } else if (a.type === "tool_failed") {
      markLastRunning(items, "failed", a.detail);
    } else if (a.type === "tool_blocked") {
      items.push({ status: "blocked", tool: a.tool, title: a.summary, detail: a.detail, time: a.time });
    } else if (a.type === "task_cancel_requested") {
      items.push({ status: "failed", tool: null, title: a.summary, detail: a.detail, time: a.time });
    }
  }
  return items;
}

const timeline = computed<TimelineItem[]>(() => {
  if (isActiveTask.value) {
    const live = activitiesToTimeline(
      agent.activities.filter((a) => a.taskId === taskId.value),
    );
    if (live.length > 0) return live;
  }
  if (taskStore.currentSteps.length > 0) return stepsToTimeline(taskStore.currentSteps);
  return eventsToTimeline(taskStore.currentEvents);
});

// ---------- 加载与轮询 ----------

const noop = () => undefined;
let timer: number | undefined;

function stopPolling() {
  if (timer !== undefined) {
    window.clearInterval(timer);
    timer = undefined;
  }
}

function refresh() {
  if (taskId.value) void taskStore.openTask(taskId.value).catch(noop);
}

watch(
  [taskId, () => taskStore.currentTask?.status, () => agent.isRunning],
  ([id]) => {
    stopPolling();
    if (!id) return;
    refresh();
    const status = taskStore.currentTask?.status;
    const active =
      status === "running" ||
      status === "waiting_approval" ||
      status === "pending" ||
      (agent.isRunning && agent.currentTaskId === id);
    if (active) {
      timer = window.setInterval(refresh, 2000);
    }
  },
  { immediate: true },
);

onUnmounted(stopPolling);

function stopTask() {
  void agent.stop();
}
</script>

<template>
  <!-- 未指定任务：任务列表 -->
  <div v-if="!taskId" class="mx-auto max-w-4xl space-y-4 px-6 py-8">
    <h1 class="text-xl font-semibold text-zinc-100">任务</h1>
    <SectionCard title="全部任务">
      <template #actions>
        <span class="text-xs text-zinc-500">{{ taskStore.tasks.length }} 条记录</span>
      </template>
      <TaskList v-if="taskStore.tasks.length > 0" :tasks="taskStore.tasks" />
      <EmptyState v-else title="还没有任务" description="回到 Home 页输入你的第一个任务。" />
    </SectionCard>
  </div>

  <!-- 任务详情 -->
  <div v-else class="mx-auto flex h-full max-w-5xl flex-col px-6 py-6">
    <div class="flex items-center gap-3 pb-4">
      <div class="min-w-0 flex-1">
        <h1 class="truncate text-lg font-semibold text-zinc-100">
          {{ task?.title ?? "任务详情" }}
        </h1>
        <div class="mt-1 flex items-center gap-2 text-xs text-zinc-500">
          <StatusBadge :status="task?.status ?? 'pending'" />
          <span v-if="isActiveTask" class="text-accent">{{ agent.stateLabel }}</span>
          <span v-if="agent.currentTaskId === taskId && agent.error" class="truncate text-danger">
            {{ agent.error }}
          </span>
        </div>
        <!-- 流式思考预览（Streaming，#005） -->
        <p
          v-if="isActiveTask && agent.thinkingText"
          class="mt-2 max-h-16 overflow-hidden text-xs leading-5 text-zinc-500"
        >
          {{ agent.thinkingText }}
        </p>
      </div>
      <button
        v-if="isActiveTask"
        type="button"
        class="btn-danger shrink-0"
        @click="stopTask"
      >
        停止任务
      </button>
    </div>

    <div class="grid min-h-0 flex-1 grid-cols-1 gap-4 lg:grid-cols-5">
      <SectionCard title="执行时间线" class="min-h-0 overflow-hidden lg:col-span-3">
        <div class="h-full overflow-y-auto">
          <StepTimeline :items="timeline" />
        </div>
      </SectionCard>
      <SectionCard title="对话记录" class="flex min-h-0 flex-col overflow-hidden lg:col-span-2">
        <MessageList :messages="messages" />
      </SectionCard>
    </div>
  </div>
</template>
