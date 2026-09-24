import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import type { MockLogEntry, MockRoute } from "@/types";

interface MockState {
  routes: MockRoute[];
  serverUrl: string | null; // 运行中地址，null 表示未启动
  loaded: boolean;
  /** 最近请求日志（服务运行时由面板轮询刷新） */
  logs: MockLogEntry[];
}

// 保存/删除串行队列：mock 路由编辑是"整行全量保存"，并发交错时
// 后落库的旧快照会覆盖先提交的改动（编辑器已做防抖，这里兜底跨组件竞态，
// 例如启用开关与编辑器保存同时发生）
let mutationQueue: Promise<unknown> = Promise.resolve();
function enqueue<T>(fn: () => Promise<T>): Promise<T> {
  const run = mutationQueue.then(fn, fn);
  mutationQueue = run.catch(() => {});
  return run;
}

export const useMockStore = defineStore("mock", {
  state: (): MockState => ({
    routes: [],
    serverUrl: null,
    loaded: false,
    logs: [],
  }),

  actions: {
    async init() {
      await Promise.all([this.loadRoutes(), this.pollStatus()]);
      this.loaded = true;
    },

    async loadRoutes() {
      this.routes = await invoke<MockRoute[]>("list_mock_routes");
    },

    async pollStatus() {
      try {
        this.serverUrl = await invoke<string | null>("mock_server_status");
      } catch {
        this.serverUrl = null;
      }
    },

    async start(port?: number) {
      this.serverUrl = await invoke<string>("start_mock_server", { port: port ?? null });
      return this.serverUrl;
    },

    async stop() {
      await invoke("stop_mock_server");
      this.serverUrl = null;
    },

    async refreshRoutes() {
      await invoke("refresh_mock_routes");
      await this.loadRoutes();
    },

    async save(input: {
      id?: string;
      method?: string;
      path: string;
      status?: number;
      responseHeaders?: string;
      responseBody?: string;
      delayMs?: number;
      enabled?: boolean;
    }): Promise<MockRoute> {
      return enqueue(async () => {
        const saved = await invoke<MockRoute>("save_mock_route", { input });
        const idx = this.routes.findIndex((r) => r.id === saved.id);
        if (idx >= 0) this.routes.splice(idx, 1, saved);
        else this.routes.unshift(saved);
        // 若 server 在运行，刷新路由表
        if (this.serverUrl) await invoke("refresh_mock_routes");
        return saved;
      });
    },

    async remove(id: string) {
      return enqueue(async () => {
        await invoke("delete_mock_route", { id });
        this.routes = this.routes.filter((r) => r.id !== id);
        if (this.serverUrl) await invoke("refresh_mock_routes");
      });
    },

    /** 拉取最近请求日志（服务未运行时静默清空） */
    async loadLogs() {
      try {
        this.logs = await invoke<MockLogEntry[]>("list_mock_logs");
      } catch {
        this.logs = [];
      }
    },

    async clearLogs() {
      await invoke("clear_mock_logs");
      this.logs = [];
    },
  },
});
