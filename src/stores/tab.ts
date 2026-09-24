import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import type {
  AuthConfig,
  HttpResponsePayload,
  KeyValue,
  RequestBody,
  RequestDraft,
  RequestItem,
  ScriptRunResult,
} from "@/types";
import { uid } from "@/utils/id";
// 仅在函数体内运行时引用（自动保存落库），与 collection → tab 的引用构成
// ESM 循环但双方都是运行时取值，live binding 下安全
import { useCollectionStore } from "./collection";

/** 请求 Tab：编辑/运行一个请求草稿 */
interface RequestTab {
  id: string;
  kind: "request";
  draft: RequestDraft;
  // ---- 运行状态（随 tab 存活，切换/重建面板不丢失）----
  response: HttpResponsePayload | null;
  sending: boolean;
  sendError: string | null;
  scriptResult: ScriptRunResult | null;
  /** 在途请求的取消标识（发送时生成，请求结束后置 null；关闭 Tab 时据此终止） */
  sendRequestId: string | null;
}

export type DocTabType = "api" | "product";

/** 文档 Tab：查看/编辑一篇 Markdown 文档，编辑状态随 tab 持久 */
export interface DocTab {
  /** "doc:<docId>"；新建未落库时为 "doc:new:<uid>" */
  id: string;
  kind: "doc";
  docId: string | null;
  title: string;
  docType: DocTabType;
  content: string;
  editing: boolean;
  // 未保存基线（加载/保存后刷新，用于 dirty 判定）
  baseTitle: string;
  baseType: DocTabType;
  baseContent: string;
}

/** Mock Tab：编辑一条 mock 路由配置。路由数据以 mockStore 为唯一来源，Tab 只持有 id */
export interface MockTab {
  /** "mock:<routeId>" */
  id: string;
  kind: "mock";
  routeId: string;
}

export type Tab = RequestTab | DocTab | MockTab;

interface TabState {
  tabs: Tab[];
  activeTabId: string | null;
  /** 最近关闭的 Tab 草稿快照（Ctrl+Shift+T 恢复），上限 10（仅请求 Tab） */
  closedDrafts: RequestDraft[];
  /** 请求自动保存指示（已归属集合的请求，编辑停顿 500ms 后落库） */
  autoSaveState: "idle" | "saving" | "saved";
}

function emptyDraft(): RequestDraft {
  return {
    id: uid("tab"),
    name: "Untitled Request",
    collectionId: null,
    method: "GET",
    url: "",
    params: [],
    headers: [],
    body: null,
    auth: null,
    preScript: "",
    testScript: "",
    timeoutMs: 30_000,
    dirty: false,
  };
}

function emptyRunState() {
  return { response: null, sending: false, sendError: null, scriptResult: null, sendRequestId: null };
}

/** 终止某 tab 仍在途的请求（关闭 Tab 时调用；请求已结束则为空操作，失败无碍） */
function cancelTabInFlight(t: Tab) {
  if (t.kind === "request" && t.sendRequestId) {
    invoke("cancel_http_request", { requestId: t.sendRequestId }).catch(() => {});
  }
}

/** 文档 Tab 是否有未保存修改 */
export function docTabDirty(t: DocTab): boolean {
  return (
    t.editing &&
    (t.title !== t.baseTitle || t.docType !== t.baseType || t.content !== t.baseContent)
  );
}

// ---- 未保存草稿的本地自动保存/恢复（防应用退出丢内容） ----
const DRAFTS_LS_KEY = "x-apimanage.dirty-drafts.v1";
/** 会话 Tab 列表快照（重启恢复上次打开的请求/文档/Mock 标签） */
const SESSION_LS_KEY = "x-apimanage.session.v1";
const MAX_RESTORE_TABS = 20;
let draftsTimer: ReturnType<typeof setTimeout> | undefined;

/** 把当前所有 dirty 草稿写入 localStorage（防抖，仅 dirty 的请求 Tab） */
function persistDrafts(tabs: Tab[]) {
  const dirty = tabs
    .filter((t): t is RequestTab => t.kind === "request" && t.draft.dirty)
    .slice(0, MAX_RESTORE_TABS)
    .map((t) => t.draft);
  try {
    if (!dirty.length) {
      localStorage.removeItem(DRAFTS_LS_KEY);
    } else {
      localStorage.setItem(DRAFTS_LS_KEY, JSON.stringify({ v: 1, tabs: dirty }));
    }
  } catch {
    /* 写入失败（如超出配额）不影响主流程 */
  }
}

function scheduleDraftPersist(tabs: () => Tab[]) {
  if (draftsTimer) clearTimeout(draftsTimer);
  draftsTimer = setTimeout(() => persistDrafts(tabs()), 400);
}

// ---- 请求自动保存：编辑停顿 500ms 后把已归属集合的请求直接落库 ----
const AUTOSAVE_DELAY = 500;
/** 全局编辑序号：保存期间又有编辑时，序号变化会阻止误清 dirty */
let editSeq = 0;
let autosaveTimer: ReturnType<typeof setTimeout> | undefined;
let autosaveQueued: string | null = null;
let autosaveRunning = false;

function scheduleAutoSave(tabId: string) {
  autosaveQueued = tabId;
  if (autosaveTimer) clearTimeout(autosaveTimer);
  autosaveTimer = setTimeout(() => void runAutoSave(), AUTOSAVE_DELAY);
}

async function runAutoSave() {
  const tabId = autosaveQueued;
  autosaveQueued = null;
  if (!tabId) return;
  if (autosaveRunning) {
    // 上一次保存仍在途：稍后重试（保留排队目标）
    scheduleAutoSave(tabId);
    return;
  }
  autosaveRunning = true;
  try {
    const tabStore = useTabStore();
    const tab = tabStore.tabs.find((t) => t.id === tabId);
    if (!tab || tab.kind !== "request") return;
    const d = tab.draft;
    if (!d.dirty || !d.collectionId) return;
    const seqBefore = editSeq;
    const collectionId = d.collectionId;
    tabStore.autoSaveState = "saving";
    const collectionStore = useCollectionStore();
    await collectionStore.saveRequest({
      id: d.id,
      collectionId,
      name: d.name || "Untitled",
      method: d.method,
      url: d.url,
      params: JSON.stringify(d.params.filter((p) => p.key)),
      headers: JSON.stringify(d.headers.filter((h) => h.key)),
      body: d.body ? JSON.stringify(d.body) : "null",
      auth: d.auth ? JSON.stringify(d.auth) : null,
      preScript: d.preScript,
      testScript: d.testScript,
      timeoutMs: d.timeoutMs,
    });
    // 保存期间无新编辑才清 dirty；否则保持（下一次停顿会再存）
    if (editSeq === seqBefore) {
      d.dirty = false;
      tabStore.flushAutosave();
    }
    tabStore.autoSaveState = "saved";
    setTimeout(() => {
      if (tabStore.autoSaveState === "saved") tabStore.autoSaveState = "idle";
    }, 1500);
  } catch {
    // 自动保存失败：保持 dirty（localStorage 快照兜底），下一次编辑会重试
    useTabStore().autoSaveState = "idle";
  } finally {
    autosaveRunning = false;
    if (autosaveQueued) scheduleAutoSave(autosaveQueued);
  }
}

export const useTabStore = defineStore("tab", {
  state: (): TabState => ({
    tabs: [],
    activeTabId: null,
    closedDrafts: [],
    autoSaveState: "idle",
  }),

  getters: {
    activeTab(state): Tab | undefined {
      return state.tabs.find((t) => t.id === state.activeTabId);
    },
    activeDraft(state): RequestDraft | undefined {
      const t = state.tabs.find((t) => t.id === state.activeTabId);
      return t?.kind === "request" ? t.draft : undefined;
    },
    activeDocTab(state): DocTab | undefined {
      const t = state.tabs.find((t) => t.id === state.activeTabId);
      return t?.kind === "doc" ? t : undefined;
    },
    activeMockTab(state): MockTab | undefined {
      const t = state.tabs.find((t) => t.id === state.activeTabId);
      return t?.kind === "mock" ? t : undefined;
    },
  },

  actions: {
    /** 新建一个空白请求 Tab */
    newTab(): string {
      const draft = emptyDraft();
      const tab: RequestTab = { id: draft.id, kind: "request", draft, ...emptyRunState() };
      this.tabs.push(tab);
      this.activeTabId = tab.id;
      return tab.id;
    },

    /** 用部分字段新建请求 Tab（cURL 导入等场景），未提供的字段取默认值 */
    openDraft(patch: Partial<RequestDraft>): string {
      const draft = { ...emptyDraft(), ...patch, id: uid("tab"), collectionId: null, dirty: true };
      const tab: RequestTab = { id: draft.id, kind: "request", draft, ...emptyRunState() };
      this.tabs.push(tab);
      this.activeTabId = tab.id;
      return tab.id;
    },

    /** 从已有的请求项打开 Tab（若已存在则激活） */
    openRequest(req: RequestItem): string {
      // 已存在则激活
      const existing = this.tabs.find((t) => t.kind === "request" && t.draft.id === req.id);
      if (existing) {
        this.activeTabId = existing.id;
        return existing.id;
      }
      const draft: RequestDraft = {
        id: req.id,
        name: req.name,
        collectionId: req.collectionId,
        method: (req.method as RequestDraft["method"]) || "GET",
        url: req.url || "",
        params: parseJson<KeyValue[]>(req.params, []),
        headers: parseJson<KeyValue[]>(req.headers, []),
        body: parseJson<RequestBody | null>(req.body, null),
        auth: parseJson<AuthConfig | null>(req.auth, null),
        preScript: req.preScript || "",
        testScript: req.testScript || "",
        timeoutMs: req.timeoutMs ?? 30_000,
        dirty: false,
      };
      const tab: RequestTab = { id: draft.id, kind: "request", draft, ...emptyRunState() };
      this.tabs.push(tab);
      this.activeTabId = tab.id;
      return tab.id;
    },

    /** 打开文档 Tab（已打开则激活；以查看模式进入） */
    openDocTab(doc: { id: string; title: string; docType: string; content: string }): string {
      const tabId = "doc:" + doc.id;
      const existing = this.tabs.find((t) => t.id === tabId);
      if (existing) {
        this.activeTabId = tabId;
        return tabId;
      }
      const docType: DocTabType = doc.docType === "product" ? "product" : "api";
      const tab: DocTab = {
        id: tabId,
        kind: "doc",
        docId: doc.id,
        title: doc.title,
        docType,
        content: doc.content,
        editing: false,
        baseTitle: doc.title,
        baseType: docType,
        baseContent: doc.content,
      };
      this.tabs.push(tab);
      this.activeTabId = tab.id;
      return tab.id;
    },

    /** 新建文档 Tab（编辑态，保存时才落库） */
    newDocTab(): string {
      const tab: DocTab = {
        id: "doc:new:" + uid("t"),
        kind: "doc",
        docId: null,
        title: "新文档",
        docType: "api",
        content: "# 标题\n\n正文（Markdown）",
        editing: true,
        baseTitle: "新文档",
        baseType: "api",
        baseContent: "# 标题\n\n正文（Markdown）",
      };
      this.tabs.push(tab);
      this.activeTabId = tab.id;
      return tab.id;
    },

    /** 文档首次保存（new → 落库）后同步 tab 身份，使左侧列表点击能命中同一 Tab */
    syncDocTabIdentity(tabId: string, docId: string) {
      const tab = this.tabs.find((t) => t.id === tabId);
      if (!tab || tab.kind !== "doc") return;
      const newId = "doc:" + docId;
      if (this.tabs.some((t) => t.id === newId)) return; // 已存在同文档 Tab（如并发场景），保守不重命名
      tab.id = newId;
      if (this.activeTabId === tabId) this.activeTabId = newId;
    },

    /** 文档被删除后静默关闭对应 Tab（不进恢复栈） */
    closeDocTabByDocId(docId: string) {
      const tabId = "doc:" + docId;
      if (!this.tabs.some((t) => t.id === tabId)) return;
      this.closeTab(tabId);
    },

    /** 打开 Mock 路由 Tab（已打开则激活） */
    openMockTab(routeId: string): string {
      const tabId = "mock:" + routeId;
      if (this.tabs.some((t) => t.id === tabId)) {
        this.activeTabId = tabId;
        return tabId;
      }
      this.tabs.push({ id: tabId, kind: "mock", routeId });
      this.activeTabId = tabId;
      return tabId;
    },

    /** Mock 路由被删除后静默关闭对应 Tab */
    closeMockTabByRouteId(routeId: string) {
      const tabId = "mock:" + routeId;
      if (!this.tabs.some((t) => t.id === tabId)) return;
      this.closeTab(tabId);
    },

    /**
     * 更新某 tab 的请求运行状态（响应/发送中/错误/脚本结果）。
     * 运行状态存在 tab 上而不是 RequestPanel 组件内，切换 Tab 后不丢失。
     */
    setRunState(
      id: string,
      patch: {
        response?: HttpResponsePayload | null;
        sending?: boolean;
        sendError?: string | null;
        scriptResult?: ScriptRunResult | null;
        sendRequestId?: string | null;
      }
    ) {
      const tab = this.tabs.find((t) => t.id === id);
      if (tab && tab.kind === "request") Object.assign(tab, patch);
    },

    /** 记录到"最近关闭"栈（深拷贝，供 Ctrl+Shift+T 恢复；仅请求 Tab） */
    rememberClosed(tabs: Tab[]) {
      const drafts = tabs
        .filter((t): t is RequestTab => t.kind === "request")
        .map((t) => JSON.parse(JSON.stringify(t.draft)) as RequestDraft);
      if (!drafts.length) return;
      this.closedDrafts.push(...drafts);
      if (this.closedDrafts.length > 10) {
        this.closedDrafts.splice(0, this.closedDrafts.length - 10);
      }
    },

    /** 恢复最近关闭的 Tab（Ctrl+Shift+T），返回是否恢复成功 */
    restoreLastClosed(): boolean {
      const draft = this.closedDrafts.pop();
      if (!draft) return false;
      const tab: RequestTab = { id: uid("tab"), kind: "request", draft, ...emptyRunState() };
      this.tabs.push(tab);
      this.activeTabId = tab.id;
      return true;
    },

    closeTab(id: string) {
      const idx = this.tabs.findIndex((t) => t.id === id);
      if (idx === -1) return;
      const [tab] = this.tabs.splice(idx, 1);
      if (tab.kind === "request") {
        this.rememberClosed([tab]);
        // Tab 关闭后终止按钮不复存在，仍在途的请求这里直接中止
        cancelTabInFlight(tab);
      }
      if (this.activeTabId === id) {
        this.activeTabId = this.tabs[idx]?.id ?? this.tabs[idx - 1]?.id ?? null;
      }
      persistDrafts(this.tabs);
    },

    /** 关闭 Tab：若有未保存改动（请求草稿/文档编辑），弹确认。返回是否真的关闭 */
    async closeTabWithConfirm(id: string): Promise<boolean> {
      const tab = this.tabs.find((t) => t.id === id);
      if (!tab) return false;
      const dirty = tab.kind === "request" ? tab.draft.dirty : tab.kind === "doc" ? docTabDirty(tab) : false;
      if (dirty) {
        const name = tabName(tab);
        const ok = await confirmDiscard(name);
        if (!ok) return false;
      }
      this.closeTab(id);
      return true;
    },

    /** 关闭其他所有 Tab（有脏改动时统一确认一次） */
    async closeOthers(id: string): Promise<void> {
      const others = this.tabs.filter((t) => t.id !== id);
      if (!others.length) return;
      const dirtyNames = others.filter(isTabDirty).map(tabName);
      if (dirtyNames.length) {
        const ok = await confirmDiscardMany(dirtyNames);
        if (!ok) return;
      }
      this.rememberClosed(others);
      others.forEach(cancelTabInFlight);
      this.tabs = this.tabs.filter((t) => t.id === id);
      this.activeTabId = id;
      persistDrafts(this.tabs);
    },

    /** 关闭右侧所有 Tab */
    async closeRight(id: string): Promise<void> {
      const idx = this.tabs.findIndex((t) => t.id === id);
      if (idx === -1 || idx === this.tabs.length - 1) return;
      const right = this.tabs.slice(idx + 1);
      const dirtyNames = right.filter(isTabDirty).map(tabName);
      if (dirtyNames.length) {
        const ok = await confirmDiscardMany(dirtyNames);
        if (!ok) return;
      }
      this.rememberClosed(right);
      right.forEach(cancelTabInFlight);
      this.tabs = this.tabs.slice(0, idx + 1);
      if (this.activeTabId && !this.tabs.some((t) => t.id === this.activeTabId)) {
        this.activeTabId = id;
      }
      persistDrafts(this.tabs);
    },

    /** 关闭全部 Tab */
    async closeAll(): Promise<void> {
      if (!this.tabs.length) return;
      const dirtyNames = this.tabs.filter(isTabDirty).map(tabName);
      if (dirtyNames.length) {
        const ok = await confirmDiscardMany(dirtyNames);
        if (!ok) return;
      }
      this.rememberClosed(this.tabs);
      this.tabs.forEach(cancelTabInFlight);
      this.tabs = [];
      this.activeTabId = null;
      persistDrafts([]);
    },

    setActive(id: string) {
      this.activeTabId = id;
    },

    /**
     * 请求首次保存进集合后 draft.id 变为真实请求 id。
     * 同步 tab.id（连同 activeTabId），保证左侧树用 activeTabId === req.id
     * 做的高亮/激活判定始终有效。
     */
    syncTabIdentity(tabId: string) {
      const tab = this.tabs.find((t) => t.id === tabId);
      if (!tab || tab.kind !== "request") return;
      if (tab.draft.id === tab.id) return;
      if (this.activeTabId === tab.id) this.activeTabId = tab.draft.id;
      tab.id = tab.draft.id;
    },

    markDirty() {
      const t = this.activeTab;
      if (t?.kind === "request") {
        t.draft.dirty = true;
        editSeq++;
        // 已归属集合的请求：500ms 停顿后自动落库；
        // 临时草稿（无 collectionId）仍走 localStorage 快照 + 手动保存
        if (t.draft.collectionId) scheduleAutoSave(t.id);
      }
      scheduleDraftPersist(() => this.tabs);
    },

    /** 保存完成后调用：草稿已落库（dirty=false），清理本地快照 */
    flushAutosave() {
      persistDrafts(this.tabs);
    },

    /**
     * 启动时恢复上次退出前未保存的草稿 Tab（仅恢复 dirty 的请求草稿）。
     * 恢复成功后清除本地快照，避免重复恢复。
     */
    initDrafts(): number {
      try {
        const raw = localStorage.getItem(DRAFTS_LS_KEY);
        if (!raw) return 0;
        const parsed = JSON.parse(raw);
        if (!parsed || parsed.v !== 1 || !Array.isArray(parsed.tabs)) return 0;
        const stored = (parsed.tabs as RequestDraft[]).filter(
          (d) => d && typeof d.id === "string" && typeof d.url === "string"
        );
        if (!stored.length) return 0;
        for (const d of stored.slice(-MAX_RESTORE_TABS)) {
          // 默认字段兜底后覆盖存储值（老版本快照缺 timeoutMs 等新字段时安全）
          const draft: RequestDraft = { ...emptyDraft(), ...d, dirty: true };
          this.tabs.push({ id: draft.id, kind: "request", draft, ...emptyRunState() });
        }
        this.activeTabId = this.tabs[this.tabs.length - 1]?.id ?? null;
        localStorage.removeItem(DRAFTS_LS_KEY);
        return this.tabs.length;
      } catch {
        return 0;
      }
    },

    // ---- 会话持久化：记录打开的 Tab（请求/文档/Mock）与激活态，重启后恢复 ----

    /** 把当前 Tab 列表序列化到 localStorage（App.vue 里防抖调用）。
     *  未保存的临时草稿不记录（由 initDrafts 的快照机制恢复）；已归属集合的请求按请求 id 记录。 */
    persistSession() {
      type Entry = { t: "req" | "doc" | "mock"; id: string };
      const entries: Entry[] = [];
      for (const tab of this.tabs) {
        if (tab.kind === "request") {
          if (tab.draft.collectionId) entries.push({ t: "req", id: tab.draft.id });
        } else if (tab.kind === "doc") {
          if (tab.docId) entries.push({ t: "doc", id: tab.docId });
        } else {
          entries.push({ t: "mock", id: tab.routeId });
        }
      }
      try {
        localStorage.setItem(
          SESSION_LS_KEY,
          JSON.stringify({ v: 1, tabs: entries, active: this.activeTabId })
        );
      } catch {
        /* 写入失败不影响主流程 */
      }
    },

    /**
     * 启动时恢复上一次会话的 Tab（须在各数据 store 加载完成后调用，
     * 请求/文档/Mock 分别从各自 store 按 id 找回）。
     * 返回恢复的 Tab 数。
     */
    restoreSession(deps: {
      findRequest: (id: string) => RequestItem | undefined;
      findDoc: (id: string) => { id: string; title: string; docType: string; content: string } | undefined;
      findMock: (id: string) => { id: string } | undefined;
    }): number {
      let restored = 0;
      let activeTarget: string | null = null;
      try {
        const raw = localStorage.getItem(SESSION_LS_KEY);
        if (!raw) return 0;
        const parsed = JSON.parse(raw);
        if (!parsed || parsed.v !== 1 || !Array.isArray(parsed.tabs)) return 0;
        for (const e of parsed.tabs as { t: string; id: string }[]) {
          let tabId: string | null = null;
          if (e.t === "req") {
            const req = deps.findRequest(e.id);
            if (req) tabId = this.openRequest(req);
          } else if (e.t === "doc") {
            const doc = deps.findDoc(e.id);
            if (doc) tabId = this.openDocTab(doc);
          } else if (e.t === "mock") {
            if (deps.findMock(e.id)) tabId = this.openMockTab(e.id);
          }
          if (tabId) {
            restored++;
            // 记录原激活 Tab 对应的新 id（请求 Tab 落库后 id 会变）
            const wasActive =
              parsed.active === e.id ||
              parsed.active === `doc:${e.id}` ||
              parsed.active === `mock:${e.id}`;
            if (wasActive) activeTarget = tabId;
          }
        }
        if (activeTarget && this.tabs.some((t) => t.id === activeTarget)) {
          this.activeTabId = activeTarget;
        }
        // 恢复完成即清快照？保留：窗口关闭时 persistSession 会重写。
      } catch {
        /* 恢复失败不阻断 */
      }
      return restored;
    },
  },
});

/** Tab 是否有未保存改动（请求草稿 dirty / 文档编辑未保存；mock 自动保存恒为否） */
function isTabDirty(t: Tab): boolean {
  return t.kind === "request" ? t.draft.dirty : t.kind === "doc" ? docTabDirty(t) : false;
}

function tabName(t: Tab): string {
  if (t.kind === "request") return t.draft.name;
  if (t.kind === "doc") return t.title;
  return "Mock 路由";
}

function parseJson<T>(s: string | null, fallback: T): T {
  if (!s) return fallback;
  try {
    return JSON.parse(s) as T;
  } catch {
    return fallback;
  }
}

/** 丢弃未保存改动的确认（优先用 Tauri 原生 dialog，退化到 window.confirm） */
async function confirmDiscard(name: string): Promise<boolean> {
  try {
    const { ask } = await import("@tauri-apps/plugin-dialog");
    return await ask(`"${name}" 有未保存的改动，确定关闭并丢弃吗？`, {
      title: "未保存的改动",
      kind: "warning",
      okLabel: "丢弃",
      cancelLabel: "取消",
    });
  } catch {
    return window.confirm(`"${name}" 有未保存的改动，确定关闭并丢弃吗？`);
  }
}

/** 批量确认：多个 Tab 有未保存改动时统一弹一次 */
async function confirmDiscardMany(names: string[]): Promise<boolean> {
  const list = names.map((n) => `"${n}"`).join("、");
  const message = `${names.length} 个 Tab 有未保存的改动：${list}。确定关闭并丢弃吗？`;
  try {
    const { ask } = await import("@tauri-apps/plugin-dialog");
    return await ask(message, {
      title: "未保存的改动",
      kind: "warning",
      okLabel: "丢弃",
      cancelLabel: "取消",
    });
  } catch {
    return window.confirm(message);
  }
}
