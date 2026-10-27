import type { UnlistenFn } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { api, type EventRecord, type Message, type Task, type TaskStep } from "@/services/api";
import { onTaskUpdated } from "@/services/events";

let unlistenTaskUpdated: UnlistenFn | null = null;

export const useTaskStore = defineStore("task", {
  state: () => ({
    tasks: [] as Task[],
    currentTask: null as Task | null,
    currentSteps: [] as TaskStep[],
    currentEvents: [] as EventRecord[],
    messagesByTask: {} as Record<string, Message[]>,
  }),
  actions: {
    /** 注册全局 task-updated 监听（App 挂载时调用一次） */
    async init() {
      if (unlistenTaskUpdated) return;
      unlistenTaskUpdated = await onTaskUpdated((task) => {
        const idx = this.tasks.findIndex((t) => t.id === task.id);
        if (idx >= 0) this.tasks.splice(idx, 1, task);
        else this.tasks.unshift(task);
        if (this.currentTask?.id === task.id) this.currentTask = task;
      });
    },
    dispose() {
      unlistenTaskUpdated?.();
      unlistenTaskUpdated = null;
    },
    async loadTasks(limit = 50) {
      this.tasks = await api.listTasks(limit);
    },
    async loadTasksSafe(limit = 50) {
      try {
        await this.loadTasks(limit);
      } catch {
        // 浏览器模式下忽略
      }
    },
    /** 打开任务：拉取任务详情、步骤、事件与消息 */
    async openTask(id: string) {
      const [task, steps, events, messages] = await Promise.all([
        api.getTask(id),
        api.listSteps(id),
        api.listEvents(id, 300),
        api.listMessages(id),
      ]);
      this.currentTask = task;
      this.currentSteps = steps;
      this.currentEvents = events;
      this.messagesByTask = { ...this.messagesByTask, [id]: messages };
    },
  },
});
