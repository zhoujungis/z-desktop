<script setup lang="ts">
import { onMounted, onUnmounted, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import ActivityPanel from "@/components/activity/ActivityPanel.vue";
import ApprovalCard from "@/components/approval/ApprovalCard.vue";
import ChatInput from "@/components/chat/ChatInput.vue";
import { useAgentStore } from "@/stores/agent";
import { useSettingsStore } from "@/stores/settings";
import { useTaskStore } from "@/stores/task";
import { useWorkspaceStore } from "@/stores/workspace";

const route = useRoute();
const router = useRouter();
const agent = useAgentStore();
const taskStore = useTaskStore();
const workspace = useWorkspaceStore();
const settingsStore = useSettingsStore();

const navItems = [
  { to: "/", label: "首页" },
  { to: "/task", label: "任务" },
  { to: "/files", label: "文件" },
  { to: "/projects", label: "项目" },
  { to: "/settings", label: "设置" },
];

function isActive(to: string): boolean {
  if (to === "/") return route.path === "/";
  return route.path === to || route.path.startsWith(`${to}/`);
}

async function handleSend(text: string) {
  await agent.startTask(text);
}

// 任务创建后自动跳转到任务页
watch(
  () => agent.currentTaskId,
  (id) => {
    if (id) void router.push(`/task/${id}`);
  },
);

onMounted(() => {
  agent.initSafe();
  taskStore.init().catch(() => undefined);
  void settingsStore.loadOrDefault();
  void taskStore.loadTasksSafe();
  void workspace.loadProjectsSafe();
});

onUnmounted(() => {
  agent.dispose();
  taskStore.dispose();
});
</script>

<template>
  <div class="flex h-full w-full">
    <!-- 左侧导航 -->
    <aside class="flex w-52 shrink-0 flex-col border-r border-surface-border bg-surface-raised/40">
      <div class="flex items-center gap-2.5 px-5 pb-4 pt-5">
        <div class="flex h-7 w-7 items-center justify-center rounded-lg bg-accent font-bold text-white">
          Z
        </div>
        <span class="text-sm font-semibold tracking-wide text-zinc-100">Z Desktop</span>
      </div>

      <nav class="flex-1 space-y-1 px-3">
        <router-link
          v-for="item in navItems"
          :key="item.to"
          :to="item.to"
          class="block rounded-lg px-3 py-2 text-sm transition-colors"
          :class="
            isActive(item.to)
              ? 'bg-accent-soft font-medium text-accent'
              : 'text-zinc-400 hover:bg-surface-overlay hover:text-zinc-200'
          "
          :aria-current="isActive(item.to) ? 'page' : undefined"
        >
          {{ item.label }}
        </router-link>
      </nav>

      <div class="border-t border-surface-border px-5 py-4">
        <div class="text-[10px] uppercase tracking-wider text-zinc-600">活跃项目</div>
        <div class="mt-1 truncate text-xs text-zinc-400" :title="workspace.activeProject?.path">
          {{ workspace.activeProject?.name ?? "未设置" }}
        </div>
      </div>
    </aside>

    <!-- 中间主区域：页面 + 底部 Chat 输入 -->
    <main class="flex min-w-0 flex-1 flex-col">
      <div class="min-h-0 flex-1 overflow-y-auto">
        <router-view />
      </div>
      <div class="px-6 pb-5 pt-3">
        <ChatInput @send="handleSend" />
      </div>
    </main>

    <!-- 右侧实时活动 -->
    <ActivityPanel />

    <!-- 全局审批浮层 -->
    <ApprovalCard />
  </div>
</template>
