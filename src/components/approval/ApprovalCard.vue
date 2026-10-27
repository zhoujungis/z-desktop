<script setup lang="ts">
import { computed, ref } from "vue";
import { useAgentStore } from "@/stores/agent";
import { toolMetaOf } from "@/tools/registry";

const agent = useAgentStore();
const approval = computed(() => agent.pendingApproval);
const meta = computed(() => toolMetaOf(approval.value?.tool));
const submitting = ref(false);

const RISK: Record<string, { label: string; cls: string }> = {
  safe: { label: "安全", cls: "bg-ok/15 text-ok" },
  low: { label: "低风险", cls: "bg-sky-400/15 text-sky-300" },
  medium: { label: "中风险", cls: "bg-warn/15 text-warn" },
  high: { label: "高风险", cls: "bg-orange-400/15 text-orange-300" },
  critical: { label: "严重", cls: "bg-danger/15 text-danger" },
};

const risk = computed(() => {
  const key = String(approval.value?.riskLevel ?? "medium").toLowerCase();
  return RISK[key] ?? RISK.medium;
});

const canAlwaysAllow = computed(() =>
  ["safe", "low", "medium"].includes(String(approval.value?.riskLevel ?? "").toLowerCase()),
);

const jsonText = computed(() => {
  const input = approval.value?.input;
  if (input === null || input === undefined) return "{}";
  if (typeof input === "string") return input;
  try {
    return JSON.stringify(input, null, 2);
  } catch {
    return String(input);
  }
});

async function decide(decision: "allow" | "deny", alwaysAllow = false) {
  if (submitting.value) return;
  submitting.value = true;
  try {
    const ok = await agent.resolveApproval(decision, alwaysAllow);
    // 决定已提交，立即收起卡片；Agent Loop 会继续执行或收尾
    if (ok) agent.clearPendingApproval();
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <div
    v-if="approval"
    class="fixed bottom-6 right-6 z-50 w-96 rounded-xl border border-warn/40 bg-surface-overlay shadow-2xl shadow-black/50"
    role="alertdialog"
    aria-labelledby="approval-title"
    aria-describedby="approval-summary"
  >
    <header class="flex items-center justify-between border-b border-surface-border px-4 py-3">
      <h2 id="approval-title" class="text-sm font-semibold text-warn">需要你的审批</h2>
      <span class="rounded-full px-2 py-0.5 text-xs font-medium" :class="risk.cls">
        {{ risk.label }}
      </span>
    </header>

    <div class="space-y-2 px-4 py-3">
      <p id="approval-summary" class="text-sm text-zinc-200">{{ approval.summary }}</p>
      <div class="text-xs text-zinc-500">
        {{ meta.group }} · {{ meta.label }}：<span class="font-mono text-accent">{{ approval.tool }}</span>
      </div>
      <pre class="selectable max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-surface p-2 text-xs text-zinc-400">{{ jsonText }}</pre>
    </div>

    <footer class="flex justify-end gap-2 border-t border-surface-border px-4 py-3">
      <button type="button" class="btn-danger" :disabled="submitting" @click="decide('deny')">
        拒绝
      </button>
      <button type="button" class="btn-primary" :disabled="submitting" @click="decide('allow')">
        允许一次
      </button>
      <button
        v-if="canAlwaysAllow"
        type="button"
        class="btn-ghost"
        :disabled="submitting"
        @click="decide('allow', true)"
      >
        总是允许
      </button>
    </footer>
  </div>
</template>
