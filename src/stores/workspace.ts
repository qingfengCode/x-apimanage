import { defineStore } from "pinia";

/**
 * 主区工作区状态：决定主区域显示 请求/文档/Mock Tab / 集合运行。
 * 文档与 Mock 路由均以标签页形式打开（与请求 Tab 共存），不独占工作区。
 */
export type WorkspaceMode = "requests" | "runner";

interface WorkspaceState {
  mode: WorkspaceMode;
  /** Runner 目标集合 id（null = 未选择，由 Runner 内的下拉决定） */
  runnerCollectionId: string | null;
}

export const useWorkspaceStore = defineStore("workspace", {
  state: (): WorkspaceState => ({
    mode: "requests",
    runnerCollectionId: null,
  }),

  actions: {
    setMode(mode: WorkspaceMode) {
      this.mode = mode;
    },

    /** 打开集合运行器，可携带目标集合 */
    openRunner(collectionId?: string) {
      if (collectionId) this.runnerCollectionId = collectionId;
      this.mode = "runner";
    },
  },
});
