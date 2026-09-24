import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import type { Collection, RequestItem } from "@/types";
import { useTabStore } from "./tab";

interface CollectionState {
  collections: Collection[];
  requests: Record<string, RequestItem[]>; // collectionId -> requests
  /** 节点展开状态：id -> 是否展开。缺省视为展开（Map 记录“收起”的节点） */
  expanded: Map<string, boolean>;
  loadedCollections: boolean;
}

export const useCollectionStore = defineStore("collection", {
  state: (): CollectionState => ({
    collections: [],
    requests: {},
    expanded: new Map(),
    loadedCollections: false,
  }),

  getters: {
    /** 顶层集合/文件夹 */
    roots(state): Collection[] {
      return state.collections
        .filter((c) => c.parentId === null)
        .sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name));
    },
    /** 某节点的子节点 */
    childrenOf: (state) => (parentId: string | null): Collection[] => {
      return state.collections
        .filter((c) => c.parentId === parentId)
        .sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name));
    },
    /** 某集合下的请求 */
    requestsOf: (state) => (collectionId: string): RequestItem[] => {
      return (state.requests[collectionId] || []).slice().sort(
        (a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name)
      );
    },
  },

  actions: {
    async init() {
      await this.loadCollections();
      // 预加载所有集合的请求
      for (const c of this.collections) {
        await this.loadRequests(c.id);
      }
    },

    async loadCollections() {
      this.collections = await invoke<Collection[]>("list_collections");
      this.loadedCollections = true;
    },

    async loadRequests(collectionId: string) {
      const list = await invoke<RequestItem[]>("list_requests", { collectionId });
      this.requests = { ...this.requests, [collectionId]: list };
    },

    isExpanded(id: string): boolean {
      // 未记录过（从未手动收起/展开）视为展开，保持开箱即用的默认
      return this.expanded.get(id) ?? true;
    },
    toggleExpand(id: string) {
      this.expanded.set(id, !this.isExpanded(id));
    },
    setExpanded(id: string, v: boolean) {
      this.expanded.set(id, v);
    },

    async createCollection(input: {
      name: string;
      parentId: string | null;
      kind: "collection" | "folder";
      description?: string;
    }): Promise<Collection> {
      const col = await invoke<Collection>("create_collection", {
        input: {
          name: input.name,
          parentId: input.parentId,
          kind: input.kind,
          description: input.description ?? null,
        },
      });
      this.collections.push(col);
      if (col.parentId) this.expanded.set(col.parentId, true);
      return col;
    },

    async renameCollection(id: string, name: string) {
      const col = this.collections.find((c) => c.id === id);
      if (!col) return;
      await invoke("update_collection", {
        input: {
          id,
          name,
          parentId: col.parentId,
          kind: col.kind,
          description: col.description,
          sortOrder: col.sortOrder,
        },
      });
      col.name = name;
    },

    async deleteCollection(id: string) {
      await invoke("delete_collection", { id });
      // 后端 FK 级联删除整棵子树，前端同样递归清理（folder 后代 + 其下请求）
      const removed = new Set<string>([id]);
      let changed = true;
      while (changed) {
        changed = false;
        for (const c of this.collections) {
          if (c.parentId && removed.has(c.parentId) && !removed.has(c.id)) {
            removed.add(c.id);
            changed = true;
          }
        }
      }
      this.collections = this.collections.filter((c) => !removed.has(c.id));
      for (const rid of removed) {
        delete this.requests[rid];
        this.expanded.delete(rid);
      }
    },

    async saveRequest(input: {
      id?: string;
      collectionId: string;
      name: string;
      method: string;
      url: string;
      params: string;
      headers: string;
      body: string;
      auth?: string | null;
      preScript?: string;
      testScript?: string;
      timeoutMs?: number | null;
    }): Promise<RequestItem> {
      const saved = await invoke<RequestItem>("save_request", {
        input: {
          id: input.id,
          collectionId: input.collectionId,
          name: input.name,
          method: input.method,
          url: input.url,
          params: input.params,
          headers: input.headers,
          body: input.body,
          auth: input.auth ?? null,
          preScript: input.preScript ?? null,
          testScript: input.testScript ?? null,
          timeoutMs: input.timeoutMs ?? null,
        },
      });
      // 若请求原来挂在其它集合下（保存时改了目标集合 = 移动），先从旧集合移除
      for (const colId of Object.keys(this.requests)) {
        if (colId === saved.collectionId) continue;
        const other = this.requests[colId];
        const i = other.findIndex((r) => r.id === saved.id);
        if (i >= 0) other.splice(i, 1);
      }
      const list = this.requests[saved.collectionId] || [];
      const idx = list.findIndex((r) => r.id === saved.id);
      if (idx >= 0) list.splice(idx, 1, saved);
      else list.push(saved);
      this.requests = { ...this.requests, [saved.collectionId]: list };
      return saved;
    },

    async deleteRequest(id: string, collectionId: string) {
      await invoke("delete_request", { id });
      const list = this.requests[collectionId] || [];
      this.requests[collectionId] = list.filter((r) => r.id !== id);
    },

    /** 复制请求（同集合下新建副本，含 auth/脚本/超时全部字段），返回新请求 */
    async duplicateRequestById(id: string): Promise<RequestItem | null> {
      for (const colId of Object.keys(this.requests)) {
        const r = this.requests[colId].find((x) => x.id === id);
        if (!r) continue;
        const saved = await this.saveRequest({
          collectionId: r.collectionId,
          name: `${r.name} (副本)`,
          method: r.method,
          url: r.url || "",
          params: r.params || "[]",
          headers: r.headers || "[]",
          body: r.body || "null",
          auth: r.auth ?? null,
          preScript: r.preScript || undefined,
          testScript: r.testScript || undefined,
          timeoutMs: r.timeoutMs ?? null,
        });
        this.expanded.set(r.collectionId, true);
        return saved;
      }
      return null;
    },

    /** 重命名请求（仅更新 name，保留其它字段） */
    async renameRequest(id: string, name: string) {
      // 找到原请求取所有字段
      for (const colId of Object.keys(this.requests)) {
        const r = this.requests[colId].find((x) => x.id === id);
        if (r) {
          const saved = await this.saveRequest({
            id: r.id,
            collectionId: r.collectionId,
            name,
            method: r.method,
            url: r.url || "",
            params: r.params || "[]",
            headers: r.headers || "[]",
            body: r.body || "null",
            auth: r.auth ?? null,
            preScript: r.preScript || undefined,
            testScript: r.testScript || undefined,
            timeoutMs: r.timeoutMs ?? null,
          });
          // 同步已打开的同请求 Tab，否则 Tab 里旧名会在下次保存时把重命名覆盖回去
          const tabStore = useTabStore();
          for (const t of tabStore.tabs) {
            if (t.kind === "request" && t.draft.id === id) t.draft.name = saved.name;
          }
          return saved;
        }
      }
    },

    /** 移动请求到另一个集合/文件夹（拖拽）；同时同步已打开 Tab 的归属 */
    async moveRequest(id: string, targetCollectionId: string) {
      let fromId: string | null = null;
      for (const colId of Object.keys(this.requests)) {
        if (this.requests[colId].some((r) => r.id === id)) {
          fromId = colId;
          break;
        }
      }
      if (!fromId || fromId === targetCollectionId) return;

      await invoke("move_request", { id, collectionId: targetCollectionId });

      // 更新本地缓存：从源集合移除，加入目标集合
      const item = this.requests[fromId].find((r) => r.id === id)!;
      this.requests[fromId] = this.requests[fromId].filter((r) => r.id !== id);
      item.collectionId = targetCollectionId;
      if (this.requests[targetCollectionId]) {
        this.requests[targetCollectionId].push(item);
      } else {
        // 目标请求列表尚未加载 → 全量拉取
        await this.loadRequests(targetCollectionId);
      }
      // 展开目标节点，让移动结果可见
      this.expanded.set(targetCollectionId, true);
      // 同步已打开的同请求 Tab 的归属，否则下次保存会把请求写回原文件夹
      const tabStore = useTabStore();
      for (const t of tabStore.tabs) {
        if (t.kind === "request" && t.draft.id === id) t.draft.collectionId = targetCollectionId;
      }
    },

    /** nodeId 是否位于 ancestorId 的子树中（沿 parentId 上溯判断） */
    isNodeDescendant(ancestorId: string, nodeId: string): boolean {
      let cur = this.collections.find((c) => c.id === nodeId);
      while (cur && cur.parentId) {
        if (cur.parentId === ancestorId) return true;
        cur = this.collections.find((c) => c.id === cur!.parentId);
      }
      return false;
    },

    /** 移动集合/文件夹节点到另一个节点下（拖拽），targetId=null 表示提升为顶层；
     *  防御：目标不能是自己或自己的子孙 */
    async moveCollection(id: string, targetId: string | null) {
      if (id === targetId) return;
      const node = this.collections.find((c) => c.id === id);
      if (!node) return;
      if (node.parentId === targetId) return; // 已在目标下，无变化
      if (targetId && this.isNodeDescendant(id, targetId)) return; // 目标在源子树内 → 会成环

      await invoke("move_node", { id, newParentId: targetId, sortOrder: null });
      node.parentId = targetId;
      node.updatedAt = Date.now();
      // 展开目标节点，让移动结果可见（提到顶层无需）
      if (targetId) this.expanded.set(targetId, true);
    },

    /**
     * 递归导入一棵 ImportedNode 树（来自 Postman 解析）。
     * 顶层为根文件夹 -> 会作为顶层 Collection 创建。
     * 返回创建的集合数量。
     */
    async importTree(root: {
      type: "folder" | "request";
      name: string;
      children?: any[];
      request?: any;
    }): Promise<number> {
      // 顶层建一个 Collection
      const rootCol = await this.createCollection({
        name: root.name || "Imported Collection",
        parentId: null,
        kind: "collection",
      });
      let count = 0;
      for (const child of root.children || []) {
        count += await this._importNode(child, rootCol.id);
      }
      return count;
    },

    /** 内部：递归一个节点，folder 创建文件夹/请求落到 parentCol */
    async _importNode(node: any, parentColId: string): Promise<number> {
      if (node.type === "folder") {
        // 嵌套文件夹也用 collection 类型（我们的树统一用 collection 表示容器）
        const folder = await this.createCollection({
          name: node.name || "Folder",
          parentId: parentColId,
          kind: "folder",
        });
        let count = 0;
        for (const child of node.children || []) {
          count += await this._importNode(child, folder.id);
        }
        return count;
      }
      // request
      const r = node.request;
      if (!r) return 0;
      await this.saveRequest({
        collectionId: parentColId,
        name: r.name,
        method: r.method,
        url: r.url,
        params: JSON.stringify(r.params || []),
        headers: JSON.stringify(r.headers || []),
        body: r.body ? JSON.stringify(r.body) : "null",
        preScript: r.preScript || "",
        testScript: r.testScript || "",
      });
      return 1;
    },
  },
});
