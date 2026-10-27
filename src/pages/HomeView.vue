<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useRouter } from "vue-router";
import EmptyState from "@/components/common/EmptyState.vue";
import SectionCard from "@/components/common/SectionCard.vue";
import TaskList from "@/components/task/TaskList.vue";
import { useAgentStore } from "@/stores/agent";
import { useTaskStore } from "@/stores/task";
import { useWorkspaceStore } from "@/stores/workspace";

const router = useRouter();
const agent = useAgentStore();
const taskStore = useTaskStore();
const workspace = useWorkspaceStore();

const greeting = computed(() => {
  const h = new Date().getHours();
  if (h < 6) return "夜深了";
  if (h < 11) return "早上好";
  if (h < 14) return "中午好";
  if (h < 18) return "下午好";
  return "晚上好";
});

const quickActions = computed(() => {
  const project = workspace.activeProject;
  return [
    {
      title: "整理下载文件夹",
      desc: "按类型归类文件",
      prompt: "整理我的下载文件夹：把里面的文件按类型（图片、文档、压缩包、安装包等）分类移动到对应子目录，完成后列出整理结果。",
    },
    {
      title: "运行项目",
      desc: project ? project.name : "启动并验证",
      prompt: project
        ? `运行项目「${project.name}」（路径：${project.path}）：安装依赖（如需要）并启动，确认启动成功后告诉我访问方式。`
        : "启动我的默认项目：安装依赖（如需要）并启动，确认启动成功。",
    },
    {
      title: "排查 Bug",
      desc: "定位并修复问题",
      prompt: "排查当前项目无法启动的 Bug：查看日志与配置，定位根本原因并修复，最后验证修复有效。",
    },
    {
      title: "打开记事本写日记",
      desc: "GUI 自动化",
      prompt: "打开记事本（Notepad），帮我写今天的日记：输入今天的日期作为标题，然后记录一段关于今天工作的简短总结，最后保存到桌面。",
    },
  ];
});

async function start(prompt: string) {
  if (agent.isRunning) return;
  await agent.startTask(prompt);
}

onMounted(() => {
  void taskStore.loadTasksSafe(50);
});
</script>

<template>
  <div class="mx-auto max-w-4xl space-y-6 px-6 py-8">
    <!-- 问候 -->
    <div>
      <h1 class="text-2xl font-semibold text-zinc-100">{{ greeting }}，我是 Z</h1>
      <p class="mt-1 text-sm text-zinc-500">告诉我你想做什么，我会在这台电脑上替你完成。</p>
    </div>

    <!-- Quick Actions -->
    <div>
      <h2 class="mb-3 text-sm font-semibold text-zinc-400">快捷操作</h2>
      <div class="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <button
          v-for="qa in quickActions"
          :key="qa.title"
          type="button"
          :disabled="agent.isRunning"
          class="card px-4 py-3 text-left transition-colors hover:border-accent/50 hover:bg-surface-overlay disabled:cursor-not-allowed disabled:opacity-40"
          @click="start(qa.prompt)"
        >
          <div class="text-sm font-medium text-zinc-200">{{ qa.title }}</div>
          <div class="mt-1 truncate text-xs text-zinc-500">{{ qa.desc }}</div>
        </button>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-3">
      <!-- Recent Tasks -->
      <SectionCard title="最近任务" class="lg:col-span-2">
        <template #actions>
          <button type="button" class="btn-ghost text-xs" @click="router.push('/task')">查看全部</button>
        </template>
        <TaskList v-if="taskStore.tasks.length > 0" :tasks="taskStore.tasks" :limit="5" />
        <EmptyState
          v-else
          title="还没有任务"
          description="在下方输入框描述任务，或点击上面的快捷操作开始。"
        />
      </SectionCard>

      <!-- 活跃项目 -->
      <SectionCard title="活跃项目">
        <template #actions>
          <button type="button" class="btn-ghost text-xs" @click="router.push('/projects')">管理</button>
        </template>
        <div v-if="workspace.activeProject">
          <div class="text-sm font-medium text-zinc-200">{{ workspace.activeProject.name }}</div>
          <p class="mt-1 break-all font-mono text-xs text-zinc-500">
            {{ workspace.activeProject.path }}
          </p>
          <p v-if="workspace.activeProject.description" class="mt-2 text-xs text-zinc-400">
            {{ workspace.activeProject.description }}
          </p>
        </div>
        <EmptyState
          v-else
          title="未设置活跃项目"
          description="设置后 Agent 会以项目路径为工作目录。"
        >
          <button type="button" class="btn-primary text-xs" @click="router.push('/projects')">
            去添加项目
          </button>
        </EmptyState>
      </SectionCard>
    </div>
  </div>
</template>
