<script setup lang="ts">
import { computed, ref, watch } from "vue";
import EmptyState from "@/components/common/EmptyState.vue";
import { api, type FsEntry } from "@/services/api";

interface TNode extends FsEntry {
  expanded: boolean;
  loading: boolean;
  error: string | null;
  children: TNode[];
}

interface FlatNode {
  node: TNode;
  depth: number;
}

const props = defineProps<{
  rootPath: string;
}>();

const emit = defineEmits<{ open: [entry: FsEntry] }>();

const nodes = ref<TNode[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

function toNode(e: FsEntry): TNode {
  return { ...e, expanded: false, loading: false, error: null, children: [] };
}

function toNodes(list: FsEntry[]): TNode[] {
  return [...list]
    .sort((a, b) =>
      a.isDir === b.isDir ? a.name.localeCompare(b.name) : a.isDir ? -1 : 1,
    )
    .map(toNode);
}

async function load() {
  if (!props.rootPath) {
    nodes.value = [];
    error.value = null;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const res = await api.fsList(props.rootPath, 1);
    nodes.value = toNodes(res.entries);
  } catch (e) {
    nodes.value = [];
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

watch(() => props.rootPath, load, { immediate: true });

async function toggle(node: TNode) {
  if (!node.isDir) {
    emit("open", node);
    return;
  }
  node.expanded = !node.expanded;
  if (node.expanded && node.children.length === 0 && !node.loading) {
    node.loading = true;
    node.error = null;
    try {
      const res = await api.fsList(node.path, 1);
      node.children = toNodes(res.entries);
    } catch (e) {
      node.error = e instanceof Error ? e.message : String(e);
    } finally {
      node.loading = false;
    }
  }
}

const flat = computed<FlatNode[]>(() => {
  const out: FlatNode[] = [];
  const walk = (list: TNode[], depth: number) => {
    for (const n of list) {
      out.push({ node: n, depth });
      if (n.isDir && n.expanded) walk(n.children, depth + 1);
    }
  };
  walk(nodes.value, 0);
  return out;
});

function fmtSize(size: number): string {
  if (size <= 0) return "";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto p-2">
    <div v-if="loading" class="px-3 py-2 text-xs text-zinc-500">加载中…</div>
    <EmptyState v-else-if="error" title="无法读取目录" :description="error" />
    <EmptyState
      v-else-if="flat.length === 0"
      title="空目录"
      description="当前目录下没有文件或子目录。"
    />
    <ul v-else>
      <li v-for="f in flat" :key="f.node.path">
        <button
          type="button"
          class="flex w-full items-center gap-1.5 rounded-lg px-2 py-1.5 text-left text-sm transition-colors hover:bg-surface-overlay"
          :style="{ paddingLeft: `${8 + f.depth * 16}px` }"
          @click="toggle(f.node)"
        >
          <span class="w-3 shrink-0 text-center text-xs text-zinc-500">
            <template v-if="f.node.isDir">{{ f.node.expanded ? "▾" : "▸" }}</template>
          </span>
          <span
            class="min-w-0 flex-1 truncate"
            :class="f.node.isDir ? 'text-zinc-200' : 'text-zinc-400'"
            :title="f.node.path"
          >
            {{ f.node.name }}
          </span>
          <span v-if="f.node.loading" class="shrink-0 text-[10px] text-zinc-500">…</span>
          <span v-else-if="!f.node.isDir" class="shrink-0 text-[10px] text-zinc-600">
            {{ fmtSize(f.node.size) }}
          </span>
        </button>
        <p v-if="f.node.error" class="truncate px-2 text-[10px] text-danger" :style="{ paddingLeft: `${24 + f.depth * 16}px` }">
          {{ f.node.error }}
        </p>
      </li>
    </ul>
  </div>
</template>
