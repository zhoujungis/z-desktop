import { createRouter, createWebHashHistory } from "vue-router";
import FilesView from "@/pages/FilesView.vue";
import HomeView from "@/pages/HomeView.vue";
import ProjectsView from "@/pages/ProjectsView.vue";
import SettingsView from "@/pages/SettingsView.vue";
import TaskView from "@/pages/TaskView.vue";

const router = createRouter({
  // Tauri 环境下 hash 路由更稳定
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "home", component: HomeView },
    { path: "/task/:id?", name: "task", component: TaskView },
    { path: "/files", name: "files", component: FilesView },
    { path: "/projects", name: "projects", component: ProjectsView },
    { path: "/settings", name: "settings", component: SettingsView },
    { path: "/:pathMatch(.*)*", redirect: "/" },
  ],
});

export default router;
