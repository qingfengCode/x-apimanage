import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import type { HistoryItem, HttpResponsePayload, RequestSnapshot } from "@/types";
import { useTabStore } from "./tab";

/** 历史响应快照的体积上限：文本 20 万字符；二进制 base64 超 1MB（约 750KB 文件）不保存 */
const SNAPSHOT_TEXT_LIMIT = 200_000;
const SNAPSHOT_BINARY_LIMIT = 1_000_000;

/** 把响转换为可入库的快照（过大文本截断、过大二进制降级为占位文本） */
function snapshotResponse(res: HttpResponsePayload): HttpResponsePayload {
  if (res.body.kind === "text") {
    if (res.body.text.length <= SNAPSHOT_TEXT_LIMIT) return res;
    return {
      ...res,
      body: {
        kind: "text",
        text: res.body.text.slice(0, SNAPSHOT_TEXT_LIMIT) + "\n\n…（历史快照已截断，重发可获取完整响应）",
        mime: res.body.mime,
      },
    };
  }
  if (res.body.base64.length <= SNAPSHOT_BINARY_LIMIT) return res;
  return {
    ...res,
    body: {
      kind: "text",
      text: `[二进制响应 ${res.size}B 超出快照上限，未保存；重发可重新获取]`,
      mime: "text/plain",
    },
  };
}

interface HistoryState {
  items: HistoryItem[];
  loaded: boolean;
  loadError: string | null;
}

export const useHistoryStore = defineStore("history", {
  state: (): HistoryState => ({
    items: [],
    loaded: false,
    loadError: null,
  }),

  actions: {
    async init() {
      try {
        this.items = await invoke<HistoryItem[]>("list_history");
        this.loaded = true;
      } catch (e) {
        this.loadError = String(e);
        this.loaded = true;
      }
    },

    async record(input: {
      method: string;
      url: string;
      status: number | null;
      timeMs: number | null;
      size: number | null;
      requestSnapshot?: string;
      /** 当时的完整响应（自动做体积裁剪后入库，恢复历史可直接查看） */
      response?: HttpResponsePayload | null;
    }) {
      try {
        const { response, ...rest } = input;
        const payload = {
          ...rest,
          responseSnapshot: response ? JSON.stringify(snapshotResponse(response)) : null,
        };
        const id = await invoke<string>("save_history", { input: payload });
        this.items.unshift({
          id,
          createdAt: Date.now(),
          ...rest,
          hasResponse: !!response,
        });
        // 防止历史无限增长，保留最近 500 条
        if (this.items.length > 500) this.items = this.items.slice(0, 500);
      } catch {
        // 记录失败不阻断主流程
      }
    },

    async clear() {
      await invoke("clear_history");
      this.items = [];
    },

    /** 从历史记录恢复为新请求 Tab：优先用完整快照，退化只填 method/url */
    restoreFromHistory(item: HistoryItem): string | null {
      let snapshot: RequestSnapshot | null = null;
      if (item.requestSnapshot) {
        try {
          snapshot = JSON.parse(item.requestSnapshot) as RequestSnapshot;
        } catch {
          snapshot = null;
        }
      }
      const tabStore = useTabStore();
      const id = tabStore.newTab();
      const t = tabStore.tabs.find((x) => x.id === id);
      const draft = t?.kind === "request" ? t.draft : undefined;
      if (!draft) return null;

      if (snapshot) {
        draft.name = snapshot.name || `${snapshot.method} ${snapshot.url}`.slice(0, 48);
        draft.method = (snapshot.method as typeof draft.method) || "GET";
        draft.url = snapshot.url || "";
        draft.params = snapshot.params || [];
        draft.headers = snapshot.headers || [];
        draft.body = snapshot.body || null;
        draft.auth = snapshot.auth ?? null;
        draft.preScript = snapshot.preScript || "";
        draft.testScript = snapshot.testScript || "";
        draft.timeoutMs = snapshot.timeoutMs ?? 30_000;
      } else {
        draft.method = (item.method as typeof draft.method) || "GET";
        draft.url = item.url || "";
        draft.name = `${item.method || "GET"} ${safePathName(item.url)}`.slice(0, 48);
      }
      draft.dirty = false;

      // 恢复当时的响应：列表只带存在性标记，这里按 id 单条拉取快照本体，
      // 注入 Tab 运行状态后响应面板直接展示（旧记录无快照则保持空）
      if (item.hasResponse) {
        void (async () => {
          try {
            const row = await invoke<{ responseSnapshot: string | null }>("get_history", {
              id: item.id,
            });
            if (!row?.responseSnapshot) return;
            const res = JSON.parse(row.responseSnapshot) as HttpResponsePayload;
            tabStore.setRunState(id, { response: res });
          } catch {
            /* 响应恢复失败不影响已恢复的请求 */
          }
        })();
      }
      return id;
    },
  },
});

function safePathName(url: string | null): string {
  if (!url) return "/";
  try {
    return new URL(url).pathname || "/";
  } catch {
    // 含 {{}} 变量或非法 URL 时，取第一个 / 之后部分
    const m = url.match(/\/[^\s?]*/);
    return m ? m[0] : url;
  }
}
