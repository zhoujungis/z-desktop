<script setup lang="ts">
defineProps<{
  title: string;
  description?: string;
}>();
</script>

<template>
  <div class="flex flex-col items-center justify-center gap-2 px-6 py-10 text-center">
    <div class="text-sm font-medium text-zinc-400">{{ title }}</div>
    <p v-if="description" class="max-w-sm text-xs leading-relaxed text-zinc-500">
      {{ description }}
    </p>
    <div v-if="$slots.default" class="mt-2">
      <slot />
    </div>
  </div>
</template>
