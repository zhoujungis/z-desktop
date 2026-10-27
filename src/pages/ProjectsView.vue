<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import EmptyState from "@/components/common/EmptyState.vue";
import SectionCard from "@/components/common/SectionCard.vue";
import { useWorkspaceStore } from "@/stores/workspace";

const workspace = useWorkspaceStore();

const form = reactive({
  name: "",
  path: "",
  description: "",
  technology: "",
  gitUrl: "",
});

const submitting = ref(false);
const message = ref<string | null>(null);
const messageOk = ref(false);

async function submit() {
  if (!form.name.trim() || !form.path.trim()) {
    message.value = "名称与路径为必填项";
    messageOk.value = false;
    return;
  }
  submitting.value = true;
  message.value = null;
  try {
    await workspace.addProject({
      name: form.name.trim(),
      path: form.path.trim(),
      description: form.description.trim() || undefined,
      technology: form.technology.trim() || undefined,
      gitUrl: form.gitUrl.trim() || undefined,
    });
    Object.assign(form, { name: "", path: "", description: "", technology: "", gitUrl: "" });
    message.value = "项目已添加";
    messageOk.value = true;
  } catch (e) {
    message.value = e instanceof Error ? e.message : String(e);
    messageOk.value = false;
  } finally {
    submitting.value = false;
  }
}

async function setActive(id: string) {
  message.value = null;
  try {
    await workspace.setActiveProject(id);
  } catch (e) {
    message.value = e instanceof Error ? e.message : String(e);
    messageOk.value = false;
  }
}

async function clearActive() {
  message.value = null;
  try {
    await workspace.setActiveProject(null);
  } catch (e) {
    message.value = e instanceof Error ? e.message : String(e);
    messageOk.value = false;
  }
}

onMounted(() => {
  void workspace.loadProjectsSafe();
});
</script>

<template>
  <div class="mx-auto max-w-4xl space-y-4 px-6 py-8">
    <h1 class="text-xl font-semibold text-zinc-100">项目</h1>

    <p v-if="message" class="rounded-lg px-3 py-2 text-sm"
      :class="messageOk ? 'bg-ok/10 text-ok' : 'bg-danger/10 text-danger'">
      {{ message }}
    </p>

    <!-- 项目列表 -->
    <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
      <div
        v-for="p in workspace.projects"
        :key="p.id"
        class="card p-4"
        :class="p.is_active ? 'border-accent/60' : ''"
      >
        <div class="flex items-start justify-between gap-2">
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <span class="truncate text-sm font-medium text-zinc-200">{{ p.name }}</span>
              <span
                v-if="p.is_active"
                class="shrink-0 rounded-full bg-accent-soft px-2 py-0.5 text-[10px] font-medium text-accent"
              >
                活跃
              </span>
            </div>
            <p class="mt-1 break-all font-mono text-xs text-zinc-500">{{ p.path }}</p>
          </div>
          <button
            v-if="!p.is_active"
            type="button"
            class="btn-ghost shrink-0 text-xs"
            @click="setActive(p.id)"
          >
            设为活跃
          </button>
          <button v-else type="button" class="btn-ghost shrink-0 text-xs" @click="clearActive">
            取消活跃
          </button>
        </div>
        <p v-if="p.description" class="mt-2 text-xs leading-relaxed text-zinc-400">
          {{ p.description }}
        </p>
        <div class="mt-2 flex flex-wrap gap-2 text-[10px] text-zinc-500">
          <span v-if="p.technology" class="rounded bg-surface-overlay px-1.5 py-0.5">
            {{ p.technology }}
          </span>
          <span v-if="p.git_url" class="truncate rounded bg-surface-overlay px-1.5 py-0.5">
            {{ p.git_url }}
          </span>
        </div>
      </div>
    </div>
    <div v-if="workspace.projects.length === 0" class="card">
      <EmptyState
        title="还没有项目"
        description="添加项目后，Agent 会以活跃项目路径作为默认工作目录。"
      />
    </div>

    <!-- 添加表单 -->
    <SectionCard title="添加项目" subtitle="name 与 path 为必填">
      <form class="grid grid-cols-1 gap-3 lg:grid-cols-2" @submit.prevent="submit">
        <label class="block">
          <span class="mb-1 block text-xs text-zinc-400">名称 *</span>
          <input v-model="form.name" class="input" placeholder="My Project" />
        </label>
        <label class="block">
          <span class="mb-1 block text-xs text-zinc-400">路径 *</span>
          <input v-model="form.path" class="input font-mono text-xs" placeholder="C:\Users\me\projects\my-app" />
        </label>
        <label class="block lg:col-span-2">
          <span class="mb-1 block text-xs text-zinc-400">描述</span>
          <input v-model="form.description" class="input" placeholder="项目简介" />
        </label>
        <label class="block">
          <span class="mb-1 block text-xs text-zinc-400">技术栈</span>
          <input v-model="form.technology" class="input" placeholder="Vue / Rust / Python…" />
        </label>
        <label class="block">
          <span class="mb-1 block text-xs text-zinc-400">Git 仓库</span>
          <input v-model="form.gitUrl" class="input font-mono text-xs" placeholder="https://github.com/…" />
        </label>
        <div class="lg:col-span-2">
          <button type="submit" class="btn-primary" :disabled="submitting">
            {{ submitting ? "添加中…" : "添加项目" }}
          </button>
        </div>
      </form>
    </SectionCard>
  </div>
</template>
