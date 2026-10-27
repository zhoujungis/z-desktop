import { defineStore } from "pinia";
import { api, type AppSettings } from "@/services/api";

/** 与 Rust AppSettings::default() 对齐 */
export function defaultSettings(): AppSettings {
  return {
    provider: "openai",
    base_url: "",
    api_key: "",
    model: "",
    auto_approve_safe: true,
    auto_approve_low: false,
    auto_approve_medium: false,
    shell_safe_auto_exec: true,
    max_steps: 30,
  };
}

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    settings: defaultSettings(),
    loaded: false,
    saving: false,
  }),
  actions: {
    async load() {
      this.settings = await api.getSettings();
      this.loaded = true;
    },
    async loadOrDefault() {
      try {
        await this.load();
      } catch {
        // 浏览器模式 / 首次启动：保留默认值
        this.loaded = true;
      }
    },
    async save() {
      this.saving = true;
      try {
        await api.saveSettings(this.settings);
      } finally {
        this.saving = false;
      }
    },
  },
});
