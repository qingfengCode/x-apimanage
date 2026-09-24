<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Send, Loader2, Save, ChevronDown, Code2, Copy, Terminal, AlertCircle, Clock, Check, CircleStop } from "lucide-vue-next";
import { Menu, MenuButton, MenuItems, MenuItem } from "@headlessui/vue";
import { useTabStore } from "@/stores/tab";
import { useEnvironmentStore } from "@/stores/environment";
import { useCollectionStore } from "@/stores/collection";
import { useHistoryStore } from "@/stores/history";
import { useVariable } from "@/composables/useVariable";
import { runPreScript, runTestScript } from "@/composables/pm";
import { toCurl, copyText } from "@/utils/curl";
import {
  buildPayload,
  friendlyError,
  type RequestSnapshotLike,
} from "@/utils/requestPayload";
import { methodColor } from "@/utils/format";
import { useToast } from "@/composables/useToast";
import { uid } from "@/utils/id";
import type {
  AuthConfig,
  HttpMethod,
  HttpRequestPayload,
  HttpResponsePayload,
  KeyValue,
  RequestBody,
  RequestDraft,
  ScriptRunResult,
} from "@/types";
import { HTTP_METHODS } from "@/types";
import ParamsTab from "./ParamsTab.vue";
import AuthTab from "./AuthTab.vue";
import HeadersTab from "./HeadersTab.vue";
import BodyTab from "./BodyTab.vue";
import ScriptsTab from "./ScriptsTab.vue";
import ResponsePanel from "@/components/response/ResponsePanel.vue";
import TestResults from "@/components/response/TestResults.vue";
import EmptyState from "@/components/common/EmptyState.vue";
import { Inbox } from "lucide-vue-next";

const tabStore = useTabStore();
const envStore = useEnvironmentStore();
const collectionStore = useCollectionStore();
const historyStore = useHistoryStore();
const { resolveString, buildVarMap, extractVars } = useVariable();

const draft = computed(() => tabStore.activeDraft);
// 仅请求 Tab 有运行状态（Tab 可能是文档类型，这里做类型收窄）
const activeTab = computed(() => {
  const t = tabStore.tabs.find((x) => x.id === tabStore.activeTabId);
  return t?.kind === "request" ? t : undefined;
});

// 请求/响应运行状态：存在 tab store（随 tab 保留），组件因切换 Tab 重建时不丢失
const sending = computed(() => activeTab.value?.sending ?? false);
const response = computed(() => activeTab.value?.response ?? null);
const sendError = computed(() => activeTab.value?.sendError ?? null);
const scriptResult = computed(() => activeTab.value?.scriptResult ?? null);
// 后端取消错误文案（AppError::Cancelled 序列化结果），用于把"已终止"与普通错误区分展示
const CANCELLED_MSG = "请求已终止";
const cancelled = computed(() => sendError.value === CANCELLED_MSG);

const activeSection = ref<"params" | "auth" | "headers" | "body" | "scripts">("params");

// 当前激活环境的变量映射
function currentVarMap(): Map<string, string> {
  return buildVarMap(envStore.activeVariables);
}

function setRunState(patch: {
  response?: HttpResponsePayload | null;
  sending?: boolean;
  sendError?: string | null;
  scriptResult?: ScriptRunResult | null;
  sendRequestId?: string | null;
}) {
  if (tabStore.activeTabId) tabStore.setRunState(tabStore.activeTabId, patch);
}

/** 应用变量解析后构造 payload 的公共实现见 utils/requestPayload.ts（与 Runner 共用） */

/** 深拷贝 draft 的发送相关字段，发送期间的用户编辑/切换 Tab 不影响在途请求 */
function snapshotDraft(d: RequestDraft): RequestSnapshotLike & {
  id: string;
  name: string;
  preScript: string;
  testScript: string;
} {
  return {
    id: d.id,
    name: d.name,
    method: d.method,
    url: d.url,
    timeoutMs: d.timeoutMs,
    params: d.params.map((p) => ({ ...p })),
    headers: d.headers.map((h) => ({ ...h })),
    body: d.body
      ? d.body.mode === "raw"
        ? { ...d.body, raw: d.body.raw }
        : { ...d.body, items: d.body.items.map((kv) => ({ ...kv })) }
      : null,
    auth: d.auth ? { ...d.auth } : null,
    preScript: d.preScript,
    testScript: d.testScript,
  };
}

async function send() {
  const d = draft.value;
  if (!d || !tabStore.activeTabId || sending.value) return;
  const tabId = tabStore.activeTabId;
  if (!d.url.trim()) {
    tabStore.setRunState(tabId, { sendError: "请填写 URL" });
    return;
  }
  // 发送前先对 draft 做快照：后续 await（pre-script 首次加载 wasm 等）期间
  // 用户切换 Tab/编辑都不应影响本次实际发出的请求
  const snap = snapshotDraft(d);
  // 本次发送的唯一标识：随请求下发给后端，「终止」按钮据此取消在途请求
  const requestId = uid("http");

  tabStore.setRunState(tabId, {
    sending: true,
    sendError: null,
    response: null,
    scriptResult: null,
    sendRequestId: requestId,
  });
  try {
    // 1. 执行 Pre-Request 脚本（在变量解析前），它 set 的变量会合并进本次请求
    const baseVars = currentVarMap();
    let mergedVars = baseVars;
    let preLogs: string[] = [];
    let preError: string | undefined;
    if (snap.preScript.trim()) {
      const pre = await runPreScript(snap.preScript, { variables: baseVars });
      preLogs = pre.logs;
      preError = pre.error;
      mergedVars = new Map(baseVars);
      for (const k of Object.keys(pre.variables)) mergedVars.set(k, pre.variables[k]);
      // pre 错误不阻断请求
    }

    // 2. 构造 payload（基于发送时刻的快照）并发送
    const payload = buildPayload(mergedVars, snap);
    const res = await invoke<HttpResponsePayload>("send_http_request", { req: payload, requestId });
    tabStore.setRunState(tabId, { response: res });

    // 3. 组装脚本结果：pre 的日志/错误无论是否成功都保留
    let result: ScriptRunResult | null = null;
    if (preLogs.length || preError) {
      result = { variables: mapToObj(mergedVars), tests: [], logs: preLogs, preError };
    }
    if (snap.testScript.trim()) {
      const t = await runTestScript(snap.testScript, { response: res, variables: mergedVars });
      result = {
        variables: { ...(result?.variables ?? mapToObj(mergedVars)), ...t.variables },
        tests: t.tests,
        logs: [...(result?.logs ?? []), ...t.logs],
        preError: result?.preError,
        testError: t.error,
      };
    }
    tabStore.setRunState(tabId, { scriptResult: result });

    // 记录历史（带完整请求快照 + 响应快照，从历史恢复时请求与响应都可直接查看）
    historyStore.record({
      method: payload.method,
      url: payload.url,
      status: res.status,
      timeMs: res.timeMs,
      size: res.size,
      response: res,
      requestSnapshot: JSON.stringify({
        name: snap.name,
        method: snap.method,
        url: snap.url,
        timeoutMs: snap.timeoutMs,
        params: snap.params,
        headers: snap.headers,
        body: snap.body,
        auth: snap.auth,
        preScript: snap.preScript,
        testScript: snap.testScript,
      }),
    });
  } catch (e: any) {
    tabStore.setRunState(tabId, { sendError: friendlyError(e) });
  } finally {
    tabStore.setRunState(tabId, { sending: false, sendRequestId: null });
  }
}

/** 终止在途请求：后端真实中止 reqwest 请求，invoke 随后以「请求已终止」拒绝 */
function cancel() {
  const rid = activeTab.value?.sendRequestId;
  if (rid) invoke("cancel_http_request", { requestId: rid }).catch(() => {});
}

function mapToObj(m: Map<string, string>): Record<string, string> {
  const o: Record<string, string> = {};
  for (const [k, v] of m) o[k] = v;
  return o;
}

// 保存请求到集合
const showSavePicker = ref(false);
const saving = ref(false);

const toast = useToast();

/** 当前归属集合名（用于提示） */
const collectionName = computed(() => {
  const cid = draft.value?.collectionId;
  if (!cid) return "";
  const col = collectionStore.collections.find((c) => c.id === cid);
  return col?.name ?? "";
});

const saveTitle = computed(() =>
  draft.value?.collectionId
    ? "已归属集合，修改自动保存（500ms 停顿后落库）；也可手动保存 (Ctrl+S)"
    : "保存到集合…（首次保存后自动保存）(Ctrl+S)"
);

function setMethod(m: HttpMethod) {
  if (draft.value) {
    draft.value.method = m;
    tabStore.markDirty();
  }
}

function onParamsChange(v: KeyValue[]) {
  if (draft.value) {
    draft.value.params = v;
    tabStore.markDirty();
  }
}
function onHeadersChange(v: KeyValue[]) {
  if (draft.value) {
    draft.value.headers = v;
    tabStore.markDirty();
  }
}
function onAuthChange(a: AuthConfig | null) {
  if (draft.value) {
    draft.value.auth = a;
    tabStore.markDirty();
  }
}
function onBodyChange(b: RequestBody | null) {
  if (draft.value) {
    draft.value.body = b;
    tabStore.markDirty();
  }
}

// 保存：若 collectionId 为空，需要选择集合
async function saveToCollection(collectionId: string) {
  if (saving.value) return;
  saving.value = true;
  const tabId = tabStore.activeTabId;
  const d = draft.value!;
  try {
    const saved = await collectionStore.saveRequest({
      id: d.collectionId ? d.id : undefined,
      collectionId,
      name: effectiveName(d),
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
    d.id = saved.id;
    d.collectionId = saved.collectionId;
    d.name = saved.name;
    d.dirty = false;
    showSavePicker.value = false;
    // 首次保存（临时 tab.id → 真实请求 id）后同步 tab 身份，保持树高亮一致
    if (tabId) tabStore.syncTabIdentity(tabId);
    // 已落库的草稿不再需要本地快照
    tabStore.flushAutosave();
    const col = collectionStore.collections.find((c) => c.id === saved.collectionId);
    toast.push(`已保存到「${col?.name ?? "集合"}」`, "success", 2000);
  } catch (e) {
    toast.push(`保存失败：${String(e)}`, "error", 5000);
  } finally {
    saving.value = false;
  }
}

function onSaveTargetChange(e: Event) {
  const v = (e.target as HTMLSelectElement).value;
  if (v) saveToCollection(v);
}

/** 保存名：用户留空或默认名时自动用「METHOD path」命名，便于在集合树里辨认 */
function effectiveName(d: RequestDraft): string {
  const trimmed = d.name.trim();
  if (trimmed && trimmed !== "Untitled Request") return trimmed;
  let path = "/";
  try {
    // {{变量}} 会让 URL 解析失败，先替换成占位符
    const u = new URL(d.url.replace(/\{\{[^}]+\}\}/g, "var"));
    path = u.pathname || "/";
  } catch {
    const m = d.url.match(/\/[^\s?]*/);
    if (m) path = m[0];
  }
  return `${d.method} ${path}`.slice(0, 64);
}

/** 保存：已关联集合则直接存回当前集合；否则展开选择器 */
function onSaveClick() {
  const d = draft.value;
  if (!d || saving.value) return;
  if (d.collectionId) {
    saveToCollection(d.collectionId);
  } else {
    showSavePicker.value = true;
  }
}

// 监听全局快捷键（send/save）
function onShortcutSend() {
  send();
}
function onShortcutSave() {
  onSaveClick();
}
function clearSendError() {
  setRunState({ sendError: null });
}

// ---- 请求级超时设置（随请求保存/随快照恢复）----
const TIMEOUT_PRESETS = [1000, 5000, 10_000, 30_000, 60_000, 120_000];
const timeoutLabel = computed(() => {
  const ms = draft.value?.timeoutMs ?? 30_000;
  return ms >= 1000 && ms % 1000 === 0 ? `${ms / 1000}s` : `${ms}ms`;
});
function setPresetTimeout(ms: number) {
  if (!draft.value) return;
  draft.value.timeoutMs = ms;
  tabStore.markDirty();
}
function onTimeoutInput(e: Event) {
  if (!draft.value) return;
  const v = Math.round(Number((e.target as HTMLInputElement).value));
  if (!Number.isFinite(v)) return;
  draft.value.timeoutMs = Math.max(100, v);
  tabStore.markDirty();
}
onMounted(() => {
  window.addEventListener("shortcut:send", onShortcutSend);
  window.addEventListener("shortcut:save", onShortcutSave);
});
onBeforeUnmount(() => {
  window.removeEventListener("shortcut:send", onShortcutSend);
  window.removeEventListener("shortcut:save", onShortcutSave);
});

// ---- 复制 cURL / URL ----
async function copyCurl() {
  const d = draft.value;
  if (!d) return;
  const payload = buildPayload(currentVarMap(), d);
  const curl = toCurl(payload);
  if (await copyText(curl)) toast.push("已复制 cURL", "success", 2000);
}
async function copyResolvedUrl() {
  const d = draft.value;
  if (!d) return;
  const payload = buildPayload(currentVarMap(), d);
  if (await copyText(payload.url)) toast.push("已复制解析后 URL", "success", 2000);
}

// ---- URL 变量解析预览 ----
const resolvedUrlPreview = computed(() => {
  const d = draft.value;
  if (!d || !d.url) return "";
  const resolved = resolveString(d.url, currentVarMap());
  return resolved === d.url ? "" : resolved;
});

// ---- 未定义变量检测：URL/Params/Headers 引用了 {{var}} 但当前环境没有 ----
// 发送时这类变量会原样发送字面 {{var}}，多半是环境选错或漏配，提前提示
const unresolvedVars = computed<string[]>(() => {
  const d = draft.value;
  if (!d) return [];
  const vars = new Set<string>();
  const collect = (s: string) => extractVars(s).forEach((v) => vars.add(v));
  if (d.url) collect(d.url);
  for (const p of d.params) if (p.enabled) collect(p.value);
  for (const h of d.headers) if (h.enabled) collect(h.value);
  const defined = currentVarMap();
  return [...vars].filter((v) => !defined.has(v));
});

function openEnvManager() {
  window.dispatchEvent(new CustomEvent("open:env-manager"));
}

// ---- 是否显示 Body section（GET/HEAD 通常无 body） ----
const showBodySection = computed(() => {
  const m = draft.value?.method.toUpperCase();
  return m !== "GET" && m !== "HEAD";
});

// ---- 可拖拽的请求/响应分栏 ----
const splitPercent = ref(38);
function onSplitDragStart(e: MouseEvent) {
  e.preventDefault();
  const container = (e.currentTarget as HTMLElement).parentElement;
  if (!container) return;
  const onMove = (ev: MouseEvent) => {
    const rect = container.getBoundingClientRect();
    const pct = ((ev.clientY - rect.top) / rect.height) * 100;
    splitPercent.value = Math.min(Math.max(pct, 15), 75);
  };
  const onUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}
</script>

<template>
  <div v-if="draft" class="flex flex-col h-full">
    <!-- 顶部：名称 + Method + URL + 按钮 -->
    <div class="border-b border-app-border px-2.5 pt-1.5 pb-0 flex flex-col bg-app-surface">
      <div class="flex items-center gap-1">
        <input
          v-model="draft.name"
          spellcheck="false"
          placeholder="请求名称"
          class="flex-1 min-w-0 bg-transparent text-[13px] font-medium outline-none px-1.5 h-6 rounded focus:bg-app-surface2 transition-colors"
          @input="tabStore.markDirty()"
        />
        <button
          class="btn-ghost btn-sm"
          title="复制解析后的 URL"
          @click="copyResolvedUrl"
        >
          <Copy :size="13" /> URL
        </button>
        <button
          class="btn-ghost btn-sm"
          title="复制为 cURL 命令"
          @click="copyCurl"
        >
          <Terminal :size="13" /> cURL
        </button>
        <!-- 自动保存指示（已归属集合的请求；500ms 停顿后自动落库） -->
        <span
          v-if="draft.collectionId"
          class="flex items-center gap-1 text-[11px] text-app-muted/70 flex-shrink-0"
          :title="'修改在停止编辑 500ms 后自动保存到' + (collectionName ? `「${collectionName}」` : '集合')"
        >
          <Loader2 v-if="tabStore.autoSaveState === 'saving'" :size="11" class="animate-spin" />
          <Check v-else-if="tabStore.autoSaveState === 'saved'" :size="11" class="text-accent-green" />
          <span v-else-if="draft.dirty" class="w-1.5 h-1.5 rounded-full bg-accent-orange/80" />
          {{
            tabStore.autoSaveState === "saving"
              ? "保存中"
              : tabStore.autoSaveState === "saved"
                ? "已自动保存"
                : "自动保存"
          }}
        </span>
        <button
          class="btn-ghost btn-sm"
          :class="{ 'opacity-70': !draft.dirty && !saving }"
          :title="saveTitle"
          @click="onSaveClick"
        >
          <Loader2 v-if="saving" :size="13" class="animate-spin" />
          <Save v-else :size="13" />
          {{ saving ? "保存中" : draft.collectionId ? "保存" : "保存到…" }}
        </button>
      </div>

      <!-- 保存目标选择 -->
      <div
        v-if="showSavePicker"
        class="mt-1.5 pop px-2 py-1.5 flex items-center gap-2 flex-wrap"
      >
        <span class="text-xs text-app-muted">保存到：</span>
        <select
          class="input"
          @change="onSaveTargetChange"
        >
          <option value="" disabled selected>选择集合...</option>
          <option
            v-for="col in collectionStore.collections.filter((c) => c.kind === 'collection')"
            :key="col.id"
            :value="col.id"
          >
            {{ col.name }}
          </option>
        </select>
        <button
          v-if="!collectionStore.collections.some((c) => c.kind === 'collection')"
          class="text-xs text-accent-blue hover:underline ml-2"
          @click="showSavePicker = false"
        >
          请先在左侧创建集合
        </button>
      </div>

      <div class="flex flex-col gap-1 mt-1.5">
        <div
          class="flex items-center gap-1 h-9 rounded-lg bg-app-surface2 border pl-1 pr-1 transition-colors"
          :class="sendError === '请填写 URL' ? 'border-accent-red' : 'border-app-border focus-within:border-accent-blue/60'"
        >
          <!-- Method 选择 -->
          <Menu as="div" class="relative flex-shrink-0">
            <MenuButton
              class="h-7 px-2.5 rounded-md font-mono text-xs font-bold uppercase flex items-center gap-1 hover:bg-app-hover transition-colors"
              :class="methodColor(draft.method)"
            >
              {{ draft.method }}
              <ChevronDown :size="12" class="text-app-muted" />
            </MenuButton>
            <MenuItems
              class="absolute left-0 mt-1 z-40 min-w-[96px] py-1 pop menu-enter"
            >
              <MenuItem v-for="m in HTTP_METHODS" :key="m" v-slot="{ active }">
                <button
                  class="w-full text-left px-3 py-1 font-mono text-xs font-bold uppercase rounded-sm"
                  :class="[active ? 'bg-app-hover' : '', methodColor(m)]"
                  @click="setMethod(m)"
                >
                  {{ m }}
                </button>
              </MenuItem>
            </MenuItems>
          </Menu>

          <div class="w-px h-5 bg-app-border flex-shrink-0" />

          <!-- URL 输入 -->
          <input
            v-model="draft.url"
            spellcheck="false"
            placeholder="输入请求 URL，例如 {{baseUrl}}/api/users"
            class="flex-1 h-7 min-w-0 bg-transparent px-1 text-xs font-mono outline-none"
            @input="tabStore.markDirty(); clearSendError()"
            @keydown.enter.exact.prevent="send"
            @keydown.ctrl.enter.prevent="send"
          />

          <!-- 请求级超时设置 -->
          <Menu as="div" class="relative flex-shrink-0">
            <MenuButton
              class="h-7 px-2 rounded-md hover:bg-app-hover transition-colors flex items-center gap-1 text-[11px] text-app-muted font-medium"
              title="请求超时（保存后随请求持久化）"
            >
              <Clock :size="12" />
              {{ timeoutLabel }}
            </MenuButton>
            <MenuItems
              class="absolute right-0 mt-1 z-40 min-w-[150px] py-1 pop menu-enter"
            >
              <div class="px-3 py-1 text-[10px] text-app-muted uppercase tracking-wide">超时</div>
              <MenuItem v-for="ms in TIMEOUT_PRESETS" :key="ms" v-slot="{ active }">
                <button
                  class="w-full text-left px-3 py-1 text-xs"
                  :class="active ? 'bg-app-hover' : ''"
                  @click="setPresetTimeout(ms)"
                >
                  {{ ms / 1000 }}s
                </button>
              </MenuItem>
              <div class="my-1 border-t border-app-border" />
              <div class="px-2 py-1">
                <input
                  type="number"
                  :value="draft?.timeoutMs ?? 30000"
                  min="100"
                  step="100"
                  class="w-full input"
                  title="自定义超时（毫秒）"
                  @change="onTimeoutInput"
                />
                <div class="text-[10px] text-app-muted mt-0.5">自定义（毫秒）</div>
              </div>
            </MenuItems>
          </Menu>

          <!-- Send / 终止 按钮：发送中可随时终止在途请求 -->
          <button
            v-if="sending"
            class="h-7 px-3.5 rounded-md bg-accent-red hover:bg-accent-red/80 text-white font-medium text-xs flex items-center gap-1.5 transition-colors"
            title="终止当前请求"
            @click="cancel"
          >
            <Loader2 :size="13" class="animate-spin" />
            终止
          </button>
          <button
            v-else
            class="h-7 px-3.5 rounded-md bg-accent-orange hover:bg-accent-orange-hover text-white font-medium text-xs flex items-center gap-1.5 send-glow transition-colors"
            @click="send"
          >
            <Send :size="13" />
            Send
          </button>
        </div>
        <!-- 变量解析预览 -->
        <div
          v-if="resolvedUrlPreview"
          class="text-[11px] text-app-muted px-1 truncate pb-0.5"
          title="变量解析后的实际 URL（当前环境）"
        >
          → {{ resolvedUrlPreview }}
        </div>
        <!-- 未定义变量警告 -->
        <button
          v-if="unresolvedVars.length"
          class="flex items-center gap-1.5 self-start mx-1 mb-0.5 px-1.5 h-5 rounded-md bg-accent-orange/10 text-accent-orange text-[11px] hover:bg-accent-orange/20 transition-colors"
          :title="`发送时这些变量会原样发送字面 {{}}。缺失：${unresolvedVars.join('、')}。点击打开环境管理`"
          @click="openEnvManager"
        >
          <AlertCircle :size="11" />
          {{ unresolvedVars.length }} 个变量未定义：{{ unresolvedVars.slice(0, 3).join("、") }}{{ unresolvedVars.length > 3 ? "…" : "" }}
        </button>
      </div>

      <!-- Section Tabs -->
      <div class="flex items-center gap-0.5 -mb-px">
        <button
          v-for="s in (showBodySection ? (['params', 'auth', 'headers', 'body', 'scripts'] as const) : (['params', 'auth', 'headers', 'scripts'] as const))"
          :key="s"
          class="px-2.5 h-7 text-[11px] uppercase tracking-wide border-b-2 flex items-center gap-1.5"
          :class="
            activeSection === s
              ? 'border-accent-orange text-app-text'
              : 'border-transparent text-app-muted hover:text-app-text'
          "
          @click="activeSection = s"
        >
          <Code2 v-if="s === 'scripts'" :size="12" />
          {{ s === "params" ? "Params" : s === "auth" ? "Auth" : s === "headers" ? "Headers" : s === "body" ? "Body" : "Scripts" }}
          <span
            v-if="s === 'params' && draft.params.filter((p) => p.key).length"
            class="text-[10px] bg-app-hover px-1.5 rounded-full"
          >
            {{ draft.params.filter((p) => p.key).length }}
          </span>
          <span
            v-if="s === 'headers' && draft.headers.filter((h) => h.key).length"
            class="text-[10px] bg-app-hover px-1.5 rounded-full"
          >
            {{ draft.headers.filter((h) => h.key).length }}
          </span>
          <span
            v-if="s === 'auth' && draft.auth"
            class="w-1.5 h-1.5 inline-block rounded-full bg-accent-green"
            title="已配置认证"
          />
          <span
            v-if="s === 'body' && draft.body"
            class="w-1.5 h-1.5 inline-block rounded-full bg-accent-orange"
          />
          <span
            v-if="s === 'scripts' && (draft.preScript || draft.testScript)"
            class="w-1.5 h-1.5 inline-block rounded-full bg-accent-orange"
          />
        </button>
      </div>
    </div>

    <!-- 请求编辑区 + 响应区 上下分栏（可拖拽） -->
    <div class="flex-1 flex flex-col min-h-0">
      <!-- 请求编辑区 -->
      <div
        class="border-b border-app-border bg-app-bg overflow-hidden"
        :style="{ height: splitPercent + '%' }"
      >
        <ParamsTab
          v-if="activeSection === 'params'"
          :model-value="draft.params"
          :url="draft.url"
          @update:model-value="onParamsChange"
          @update:url="(v) => ((draft!.url = v), tabStore.markDirty())"
        />
        <AuthTab
          v-else-if="activeSection === 'auth'"
          :model-value="draft.auth"
          @update:model-value="onAuthChange"
        />
        <HeadersTab
          v-else-if="activeSection === 'headers'"
          :model-value="draft.headers"
          @update:model-value="onHeadersChange"
        />
        <BodyTab
          v-else-if="activeSection === 'body'"
          :model-value="draft.body"
          @update:model-value="onBodyChange"
        />
        <ScriptsTab
          v-else
          :pre-script="draft.preScript"
          :test-script="draft.testScript"
          @update:pre-script="(v: string) => ((draft!.preScript = v), tabStore.markDirty())"
          @update:test-script="(v: string) => ((draft!.testScript = v), tabStore.markDirty())"
        />
      </div>

      <!-- 可拖拽分隔条 -->
      <div
        class="h-1 flex-shrink-0 bg-app-border hover:bg-accent-blue cursor-row-resize transition-colors"
        @mousedown="onSplitDragStart"
      />

      <!-- 响应区 -->
      <div class="flex-1 min-h-0 overflow-hidden flex flex-col">
        <div class="flex-1 min-h-0 overflow-hidden">
          <div
            v-if="sendError"
            class="h-full flex flex-col items-center justify-center text-sm px-4 text-center gap-2"
            :class="cancelled ? 'text-app-muted' : 'text-accent-red'"
          >
            <CircleStop v-if="cancelled" :size="26" :stroke-width="1.2" class="opacity-70" />
            <AlertCircle v-else :size="28" :stroke-width="1.2" class="opacity-70" />
            <span>{{ sendError }}</span>
            <button v-if="sendError === '请填写 URL'" class="text-xs text-app-muted">在上方 URL 栏输入地址</button>
          </div>
          <EmptyState
            v-else-if="!response && !sending"
            :icon="Inbox"
            title="暂无响应"
            hint="点击右上角 Send 发送请求 (Ctrl+Enter)"
          />
          <ResponsePanel v-else-if="response" :response="response" />
          <div v-else class="h-full flex items-center justify-center text-app-muted text-sm">
            <Loader2 :size="18" class="animate-spin mr-2" /> 请求中...
          </div>
        </div>
        <!-- 测试脚本结果面板 -->
        <TestResults :result="scriptResult" />
      </div>
    </div>
  </div>
</template>
