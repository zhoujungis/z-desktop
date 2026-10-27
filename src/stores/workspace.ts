import { defineStore } from "pinia";
import { api, type Project } from "@/services/api";

export interface AddProjectPayload {
  name: string;
  path: string;
  description?: string;
  technology?: string;
  gitUrl?: string;
}

export const useWorkspaceStore = defineStore("workspace", {
  state: () => ({
    projects: [] as Project[],
    activeProject: null as Project | null,
    loading: false,
  }),
  actions: {
    async loadProjects() {
      this.loading = true;
      try {
        this.projects = await api.listProjects();
        this.activeProject = await api.getActiveProject();
      } finally {
        this.loading = false;
      }
    },
    async loadProjectsSafe() {
      try {
        await this.loadProjects();
      } catch {
        // 浏览器模式下忽略
      }
    },
    async addProject(payload: AddProjectPayload) {
      const project = await api.addProject(payload);
      await this.loadProjects();
      return project;
    },
    async setActiveProject(id: string | null) {
      await api.setActiveProject(id);
      await this.loadProjects();
    },
  },
});
