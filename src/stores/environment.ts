import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import type { Environment, KeyValue } from "@/types";

interface EnvironmentState {
  environments: Environment[];
  loaded: boolean;
}

export const useEnvironmentStore = defineStore("environment", {
  state: (): EnvironmentState => ({
    environments: [],
    loaded: false,
  }),

  getters: {
    active(state): Environment | undefined {
      return state.environments.find((e) => e.isActive);
    },
    /** 当前激活环境的变量数组 */
    activeVariables(): KeyValue[] {
      const env = this.active;
      if (!env) return [];
      try {
        return JSON.parse(env.variables) as KeyValue[];
      } catch {
        return [];
      }
    },
  },

  actions: {
    async init() {
      this.environments = await invoke<Environment[]>("list_environments");
      this.loaded = true;
    },

    parseVariables(env: Environment): KeyValue[] {
      try {
        return JSON.parse(env.variables) as KeyValue[];
      } catch {
        return [];
      }
    },

    async setActive(id: string) {
      await invoke("set_active_environment", { id });
      for (const e of this.environments) e.isActive = e.id === id;
    },

    async save(input: {
      id?: string;
      name: string;
      variables: KeyValue[];
    }): Promise<Environment> {
      const saved = await invoke<Environment>("save_environment", {
        input: {
          id: input.id,
          name: input.name,
          variables: JSON.stringify(input.variables),
        },
      });
      const idx = this.environments.findIndex((e) => e.id === saved.id);
      if (idx >= 0) this.environments.splice(idx, 1, saved);
      else this.environments.push(saved);
      return saved;
    },

    async remove(id: string) {
      await invoke("delete_environment", { id });
      this.environments = this.environments.filter((e) => e.id !== id);
      // 删除的是当前激活环境时，自动提升剩余的第一个，避免应用停在"无环境"
      if (!this.environments.length) return;
      if (!this.environments.some((e) => e.isActive)) {
        const next = this.environments[0];
        try {
          await invoke("set_active_environment", { id: next.id });
        } catch {
          /* 后端失败不阻断删除本身 */
        }
        for (const e of this.environments) e.isActive = e.id === next.id;
      }
    },
  },
});
