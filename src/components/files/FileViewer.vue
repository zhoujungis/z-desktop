<script setup lang="ts">
import { ref, watch } from "vue";
import EmptyState from "@/components/common/EmptyState.vue";
import { api, type FsReadResult } from "@/services/api";

const props = defineProps<{
  path: string | null;
}>();

const loading = ref(false);
const error = ref<string | null>(null);
const data = ref<FsReadResult | null>(null);

async function load() {
  data.value = null;
  error.value = null;
  if (!props.path) return;
  loading.value = true;
  try {
    data.value = await api.fsRead(props.path, 1_000_000);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

watch(() => props.path, load, { immediate: true });

function fmtSize(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <div v-if="path" class="flex items-center gap-2 border-b border-surface-border px-4 py-2">
      <span class="min-w-0 flex-1 truncate font-mono text-xs text-zinc-400" :title="path">
        {{ path }}
      </span>
      <span v-if="data" class="shrink-0 text-[10px] text-zinc-500">{{ fmtSize(data.size) }}</span>
      <span
        v-if="data?.truncated"
        class="shrink-0 rounded bg-warn/15 px-1.5 py-0.5 text-[10px] text-warn"
      >
        内容已截断
      </span>
    </div>

    <div v-if="loading" class="flex flex-1 items-center justify-center text-xs text-zinc-500">
      加载中…
    </div>
    <EmptyState
      v-else-if="error"
      title="无法读取文件"
      :description="error"
    />
    <EmptyState
      v-else-if="!path"
      title="未选择文件"
      description="点击左侧文件树中的文本文件进行预览。"
    />
    <EmptyState
      v-else-if="data?.binary"
      title="二进制文件"
      :description="`该文件为二进制格式（${fmtSize(data.size)}），已读取为 base64 内容，暂不支持预览。`"
    />
    <pre
      v-else-if="data"
      class="selectable min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words p-4 font-mono text-xs leading-relaxed text-zinc-300"
      >{{ data.content }}</pre
    >
  </div>
</template>
