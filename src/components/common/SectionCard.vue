<script setup lang="ts">
defineProps<{
  title: string;
  subtitle?: string;
}>();
</script>

<template>
  <section class="card">
    <header class="flex items-center justify-between gap-3 border-b border-surface-border px-4 py-3">
      <div class="min-w-0">
        <h2 class="text-sm font-semibold text-zinc-200">{{ title }}</h2>
        <p v-if="subtitle" class="mt-0.5 truncate text-xs text-zinc-500">{{ subtitle }}</p>
      </div>
      <div v-if="$slots.actions" class="flex shrink-0 items-center gap-2">
        <slot name="actions" />
      </div>
    </header>
    <div class="p-4">
      <slot />
    </div>
  </section>
</template>
