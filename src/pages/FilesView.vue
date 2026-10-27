<script setup lang="ts">
import { onMounted, ref } from "vue";
import FileTree from "@/components/files/FileTree.vue";
import FileViewer from "@/components/files/FileViewer.vue";
import EmptyState from "@/components/common/EmptyState.vue";
import { api, type FsEntry } from "@/services/api";
import { useWorkspaceStore } from "@/stores/workspace";
import { parentPath } from "@/utils/path";

const workspace = useWorkspaceStore();

const currentPath = ref("/");
const pathInput = ref("/");
const viewerPath = ref<string | null>(null);

const searchInput = ref("");
const searching = ref(false);
const searchError = ref<string | null>(null);
const searchResults = ref<FsEntry[] | null>(null);
const searchOpened = ref(false);

onMounted(async () => {
  await workspace.loadProjectsSafe();
  const p = workspace.activeProject?.path;
  if (p) {
    currentPath.value = p;
    pathInput.value = p;
  }
});

function go() {
  const p = pathInput.value.trim();
  if (!p) return;
  searchResults.value = null;
  searchOpened.value = false;
  currentPath.value = p;
}

function goUp() {
  pathInput.value = parentPath(currentPath.value);
  go();
}

function openEntry(entry: FsEntry) {
  viewerPath.value = entry.path;
}

async function doSearch() {
  const pattern = searchInput.value.trim() || "*";
  searching.value = true;
  searchError.value = null;
  searchResults.value = null;
  try {
    const res = await api.fsSearch(currentPath.value, pattern, 200);
    searchResults.value = res.matches;
    searchOpened.value = true;
  } catch (e) {
    searchError.value = e instanceof Error ? e.message : String(e);
  } finally {
    searching.value = false;
  }
}

function clearSearch() {
  searchResults.value = null;
  searchError.value = null;
  searchOpened.value = false;
  searchInput.value = "";
}

function openSearchResult(entry: FsEntry) {
  if (entry.isDir) {
    currentPath.value = entry.path;
    pathInput.value = entry.path;
    clearSearch();
  } else {
    viewerPath.value = entry.path;
  }
}
</script>

<template>
  <div class="flex h-full flex-col px-6 py-6">
    <!-- 工具栏 -->
    <div class="flex flex-wrap items-center gap-2 pb-4">
      <input
        v-model="pathInput"
        class="input max-w-md flex-1 font-mono text-xs"
        placeholder="输入目录路径，如 C:\Users\me\Documents"
        @keydown.enter="go"
      />
      <button type="button" class="btn-primary shrink-0" @click="go">转到</button>
      <button type="button" class="btn-ghost shrink-0" @click="goUp">上级</button>
      <div class="mx-2 hidden h-5 w-px bg-surface-border sm:block" />
      <input
        v-model="searchInput"
        class="input max-w-xs flex-1 text-xs"
        placeholder="搜索文件名，如 *.pdf"
        @keydown.enter="doSearch"
      />
      <button type="button" class="btn-ghost shrink-0" :disabled="searching" @click="doSearch">
        {{ searching ? "搜索中…" : "搜索" }}
      </button>
      <button
        v-if="searchOpened"
        type="button"
        class="btn-ghost shrink-0"
        @click="clearSearch"
      >
        清除搜索
      </button>
    </div>

    <!-- 主体 -->
    <div class="grid min-h-0 flex-1 grid-cols-1 gap-4 lg:grid-cols-5">
      <!-- 左：目录树 / 搜索结果 -->
      <div class="card flex min-h-0 flex-col overflow-hidden lg:col-span-2">
        <template v-if="searchOpened">
          <div class="border-b border-surface-border px-4 py-2 text-xs text-zinc-500">
            搜索结果
            <span v-if="searchResults">（{{ searchResults.length }}）</span>
          </div>
          <div class="min-h-0 flex-1 overflow-y-auto p-2">
            <div v-if="searching" class="px-3 py-2 text-xs text-zinc-500">搜索中…</div>
            <EmptyState v-else-if="searchError" title="搜索失败" :description="searchError" />
            <EmptyState
              v-else-if="!searchResults || searchResults.length === 0"
              title="没有匹配的文件"
              description="换个通配符试试，例如 *.log 或 report*。"
            />
            <ul v-else class="space-y-0.5">
              <li v-for="entry in searchResults" :key="entry.path">
                <button
                  type="button"
                  class="w-full rounded-lg px-2 py-1.5 text-left hover:bg-surface-overlay"
                  @click="openSearchResult(entry)"
                >
                  <div class="truncate text-sm" :class="entry.isDir ? 'text-zinc-200' : 'text-zinc-400'">
                    {{ entry.name }}
                  </div>
                  <div class="truncate font-mono text-[10px] text-zinc-600">{{ entry.path }}</div>
                </button>
              </li>
            </ul>
          </div>
        </template>
        <template v-else>
          <div class="truncate border-b border-surface-border px-4 py-2 font-mono text-xs text-zinc-500" :title="currentPath">
            {{ currentPath }}
          </div>
          <FileTree :root-path="currentPath" @open="openEntry" />
        </template>
      </div>

      <!-- 右：文件预览 -->
      <div class="card flex min-h-0 flex-col overflow-hidden lg:col-span-3">
        <FileViewer :path="viewerPath" />
      </div>
    </div>
  </div>
</template>
