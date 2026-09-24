<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  PlayCircle,
  Square,
  Loader2,
  CheckCircle2,
  XCircle,
  ChevronDown,
  CircleAlert,
  Download,
  RotateCcw,
  ExternalLink,
  FileCode,
} from "lucide-vue-next";
import { useCollectionStore } from "@/stores/collection";
import { useEnvironmentStore } from "@/stores/environment";
import { useHistoryStore } from "@/stores/history";
import { useTabStore } from "@/stores/tab";
import { useWorkspaceStore } from "@/stores/workspace";
import { useVariable } from "@/composables/useVariable";
import { runPreScript, runTestScript } from "@/composables/pm";
import { buildPayload, friendlyError } from "@/utils/requestPayload";
import { uid } from "@/utils/id";
import { responseText } from "@/utils/mime";
import { methodColor, statusColor, formatTime } from "@/utils/format";
import type {
  AuthConfig,
  HttpResponsePayload,
  KeyValue,
  RequestItem,
  RequestBody,
} from "@/types";
import EmptyState from "@/components/common/EmptyState.vue";
import { useToast } from "@/composables/useToast";

const collectionStore = useCollectionStore();
const envStore = useEnvironmentStore();
const historyStore = useHistoryStore();
const tabStore = useTabStore();
const workspaceStore = useWorkspaceStore();
const { buildVarMap } = useVariable();
const toast = useToast();

/** 在请求 Tab 中打开该结果对应的请求并切回请求工作区（失败排查闭环） */
function openInTab(item: RunItem) {
  let req: RequestItem | undefined;
  for (const colId of Object.keys(collectionStore.requests)) {
    req = collectionStore.requests[colId].find((r) => r.id === item.id);
    if (req) break;
  }
  if (!req) {
    toast.push("未找到该请求（可能已被删除）", "error", 3000);
    return;
  }
  workspaceStore.setMode("requests");
  tabStore.openRequest(req);
}

/** 后端 list_requests_recursive 返回的行（与 RequestItem 同构） */
interface RunnerRow {
  id: string;
  collectionId: string;
  name: string;
  method: string;
  url: string | null;
  params: string | null;
  headers: string | null;
  body: string | null;
  auth: string | null;
  preScript: string | null;
  testScript: string | null;
  timeoutMs: number | null;
}

type ItemState = "pending" | "running" | "done" | "error";

interface RunItem {
  id: string;
  /** v-for/展开态用的唯一键（迭代>1 时同 id 会出现多轮） */
  key: string;
  iteration: number;
  name: string;
  method: string;
  url: string;
  state: ItemState;
  status: number | null;
  timeMs: number | null;
  error: string | null;
  tests: { name: string; passed: boolean; error?: string }[];
  testError?: string;
  responsePreview: string;
}

const selectedCollectionId = ref<string | null>(null);
const items = ref<RunItem[]>([]);
const running = ref(false);
const abortFlag = ref(false);
/** 当前在途单发的取消标识（stop 时据此真实中止后端请求，而非等它跑完/超时） */
const inFlightId = ref<string | null>(null);
const expandedKey = ref<string | null>(null);
const totalMs = ref(0);

// ---- 运行参数 ----
const iterations = ref(1);
const delayMs = ref(0);
/** 上一轮运行的行快照与实际使用的参数（供「仅重跑失败项」与导出） */
const lastRows = ref<RunnerRow[]>([]);
const runMeta = ref({ collectionName: "", iterations: 1, delayMs: 0 });

const collections = computed(() =>
  collectionStore.collections.filter((c) => c.kind === "collection")
);

// 从集合右键菜单进入时携带目标集合
watch(
  () => workspaceStore.runnerCollectionId,
  (id) => {
    if (id) selectedCollectionId.value = id;
  },
  { immediate: true }
);

const summary = computed(() => {
  const done = items.value.filter((i) => i.state === "done");
  const failed = items.value.filter(
    (i) => i.state === "error" || (i.state === "done" && i.tests.some((t) => !t.passed))
  );
  return {
    total: items.value.length,
    finished: items.value.filter((i) => i.state === "done" || i.state === "error").length,
    passed: done.length - done.filter((i) => i.tests.some((t) => !t.passed)).length,
    failed: failed.length,
  };
});

/** 有可重跑的失败项（且不在运行中） */
const canRerunFailed = computed(
  () => !running.value && !!lastRows.value.length && summary.value.failed > 0
);

function parseJson<T>(s: string | null, fallback: T): T {
  if (!s) return fallback;
  try {
    return JSON.parse(s) as T;
  } catch {
    return fallback;
  }
}

/** 从请求行构造发送快照（空 URL 直接判失败，不发） */
function rowToSnapshot(r: RunnerRow) {
  return {
    id: r.id,
    name: r.name,
    method: (r.method || "GET").toUpperCase(),
    url: r.url || "",
    params: parseJson<KeyValue[]>(r.params, []),
    headers: parseJson<KeyValue[]>(r.headers, []),
    body: parseJson<RequestBody | null>(r.body, null),
    auth: parseJson<AuthConfig | null>(r.auth, null),
    preScript: r.preScript || "",
    testScript: r.testScript || "",
    timeoutMs: r.timeoutMs ?? 30_000,
  };
}

async function loadRows(collectionId: string): Promise<RunnerRow[]> {
  return invoke<RunnerRow[]>("list_requests_recursive", { collectionId });
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

/**
 * 运行：默认全量；onlyFailed=true 时仅重跑上一轮失败的请求。
 * 串行执行 plan = 迭代 × 请求行，行间可配置延迟。
 */
async function run(onlyFailed = false) {
  const cid = selectedCollectionId.value;
  if (!cid || running.value) return;
  abortFlag.value = false;
  running.value = true;
  totalMs.value = 0;
  expandedKey.value = null;

  try {
    let rows = await loadRows(cid);
    lastRows.value = rows;
    if (onlyFailed) {
      const failedIds = new Set(items.value.filter(itemFailed).map((i) => i.id));
      rows = rows.filter((r) => failedIds.has(r.id));
    }

    const iterCount = Math.max(1, Math.min(Math.round(iterations.value) || 1, 100));
    runMeta.value = {
      collectionName: collections.value.find((c) => c.id === cid)?.name ?? "",
      iterations: iterCount,
      delayMs: Math.max(0, delayMs.value),
    };

    // 执行计划：迭代 × 请求
    const plan: { row: RunnerRow; iteration: number }[] = [];
    for (let it = 1; it <= iterCount; it++) {
      for (const r of rows) plan.push({ row: r, iteration: it });
    }
    items.value = plan.map(({ row, iteration }) => ({
      id: row.id,
      key: `${row.id}#${iteration}`,
      iteration,
      name: row.name,
      method: (row.method || "GET").toUpperCase(),
      url: row.url || "",
      state: "pending",
      status: null,
      timeMs: null,
      error: null,
      tests: [],
      responsePreview: "",
    }));

    const startAt = performance.now();
    for (let i = 0; i < plan.length; i++) {
      if (abortFlag.value) break;
      const item = items.value[i];
      item.state = "running";
      await runOne(plan[i].row, item);
      item.state = item.error ? "error" : "done";
      // 请求间延迟（最后一个不等待）
      if (runMeta.value.delayMs > 0 && i < plan.length - 1) {
        await sleep(runMeta.value.delayMs);
      }
    }
    totalMs.value = Math.round(performance.now() - startAt);
  } finally {
    running.value = false;
  }
}

async function runOne(row: RunnerRow, item: RunItem) {
  const snap = rowToSnapshot(row);
  if (!snap.url.trim()) {
    item.error = "URL 为空";
    return;
  }
  try {
    // 1. Pre-Request 脚本（变量解析前执行，set 的变量并入本次请求）
    const baseVars = buildVarMap(envStore.activeVariables);
    let mergedVars = baseVars;
    if (snap.preScript.trim()) {
      const pre = await runPreScript(snap.preScript, { variables: baseVars });
      mergedVars = new Map(baseVars);
      for (const k of Object.keys(pre.variables)) mergedVars.set(k, pre.variables[k]);
      // pre 错误不阻断发送
    }

    // 2. 发送
    const payload = buildPayload(mergedVars, snap);
    const requestId = uid("run");
    inFlightId.value = requestId;
    let res: HttpResponsePayload;
    try {
      res = await invoke<HttpResponsePayload>("send_http_request", { req: payload, requestId });
    } finally {
      inFlightId.value = null;
    }
    item.status = res.status;
    item.timeMs = res.timeMs;
    item.responsePreview = responseText(res.body).slice(0, 2000);

    // 3. Test 脚本
    if (snap.testScript.trim()) {
      const t = await runTestScript(snap.testScript, {
        response: res,
        variables: mergedVars,
      });
      item.tests = t.tests;
      if (t.error) item.testError = t.error;
    }

    // 记录历史（与单发一致，含响应快照，便于回溯）
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
    item.error = friendlyError(e);
  }
}

function stop() {
  abortFlag.value = true;
  // 有单发仍在途时立即终止它，否则要等它自然结束（可能是 30s 超时）才真正停下
  if (inFlightId.value) {
    invoke("cancel_http_request", { requestId: inFlightId.value }).catch(() => {});
  }
}

function toggleExpand(key: string) {
  expandedKey.value = expandedKey.value === key ? null : key;
}

/** 行整体判定：错误或断言失败都算失败 */
function itemFailed(i: RunItem): boolean {
  return i.state === "error" || (i.state === "done" && i.tests.some((t) => !t.passed));
}

/** 导出 JSON 运行报告（原生保存对话框） */
async function exportReport() {
  if (!items.value.length) return;
  const report = {
    type: "x-apimanage-runner-report",
    collection: runMeta.value.collectionName,
    exportedAt: new Date().toISOString(),
    iterations: runMeta.value.iterations,
    delayMs: runMeta.value.delayMs,
    totalMs: totalMs.value,
    summary: {
      total: summary.value.total,
      passed: summary.value.passed,
      failed: summary.value.failed,
    },
    items: items.value.map((i) => ({
      iteration: i.iteration,
      name: i.name,
      method: i.method,
      url: i.url,
      status: i.status,
      timeMs: i.timeMs,
      state: i.state,
      error: i.error,
      tests: i.tests.map((t) => ({ name: t.name, passed: t.passed, error: t.error ?? null })),
    })),
  };
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const defaultName = `runner-${runMeta.value.collectionName || "report"}-${Date.now()}.json`;
    const path = await save({
      title: "导出运行报告",
      defaultPath: defaultName.replace(/[\\/:*?"<>|]/g, "_"),
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    const { invoke: inv } = await import("@tauri-apps/api/core");
    await inv("write_text_file", { path, content: JSON.stringify(report, null, 2) });
    toast.push("已导出运行报告", "success");
  } catch (e) {
    toast.push(String(e), "error", 5000);
  }
}

/** 导出 HTML 运行报告（自包含暗色页面，可直接分享/存档） */
async function exportHtmlReport() {
  if (!items.value.length) return;
  const s = summary.value;
  const esc = (v: unknown) =>
    String(v ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const rows = items.value
    .map((i) => {
      const fail = itemFailed(i);
      const tests = i.tests.length
        ? `<span class="badge ${i.tests.some((t) => !t.passed) ? "b-fail" : "b-pass"}">${i.tests.filter((t) => t.passed).length}/${i.tests.length}</span>`
        : "";
      const detail = [
        i.error ? `<div class="err">✗ ${esc(i.error)}</div>` : "",
        ...i.tests.map(
          (t) =>
            `<div class="test">${t.passed ? "✓" : "✗"} ${esc(t.name)}${t.error ? ` <span class="dim">— ${esc(t.error)}</span>` : ""}</div>`
        ),
        i.testError ? `<div class="test warn">脚本错误：${esc(i.testError)}</div>` : "",
      ].join("");
      return `<tr class="${fail ? "row-fail" : ""}">
        <td class="st">${fail ? "✗" : i.state === "done" ? "✓" : "…"}</td>
        <td><span class="badge ${fail ? "b-fail" : "b-pass"}">${esc(i.method)}</span></td>
        <td>${esc(i.name)}${runMeta.value.iterations > 1 ? ` <span class="dim">#${i.iteration}</span>` : ""}</td>
        <td class="mono dim">${esc(i.url)}</td>
        <td class="mono">${i.status ?? ""}</td>
        <td class="mono">${i.timeMs != null ? formatTime(i.timeMs) : ""}</td>
        <td>${tests}</td>
      </tr>${detail ? `<tr class="detail-row"><td></td><td colspan="6">${detail}</td></tr>` : ""}`;
    })
    .join("\n");
  const html = `<!DOCTYPE html>
<html lang="zh-CN"><head><meta charset="UTF-8"><title>Runner 报告 - ${esc(runMeta.value.collectionName)}</title>
<style>
  body{background:#12141a;color:#e2e5ec;font:13px/1.5 -apple-system,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif;margin:24px}
  h1{font-size:18px;margin:0 0 4px} .meta{color:#8a8e99;font-size:12px;margin-bottom:16px}
  .sum{display:flex;gap:16px;margin-bottom:16px;font-size:13px}
  .sum b{font-size:18px} .ok{color:#34d399} .bad{color:#f85a5a} .dim{color:#8a8e99}
  table{border-collapse:collapse;width:100%;background:#0d0f13;border:1px solid #262a33;border-radius:8px;overflow:hidden}
  th,td{padding:6px 10px;text-align:left;border-bottom:1px solid #1e2129;vertical-align:top}
  th{color:#8a8e99;font-size:11px;text-transform:uppercase;letter-spacing:.05em;background:#16181f}
  .mono{font-family:Consolas,Menlo,monospace;font-size:12px} .st{width:20px}
  .badge{display:inline-block;padding:1px 6px;border-radius:4px;font-size:11px;font-family:Consolas,monospace}
  .b-pass{background:rgba(52,211,153,.12);color:#34d399} .b-fail{background:rgba(248,90,90,.12);color:#f85a5a}
  .row-fail td{background:rgba(248,90,90,.04)}
  .detail-row td{background:#101218;padding:4px 10px 8px}
  .err,.test{font-size:12px;padding:1px 0} .err{color:#f85a5a} .warn{color:#e3b342}
</style></head><body>
<h1>集合运行报告 — ${esc(runMeta.value.collectionName)}</h1>
<div class="meta">${new Date().toLocaleString()} · ${runMeta.value.iterations} 轮迭代${runMeta.value.delayMs ? ` · 间隔 ${runMeta.value.delayMs}ms` : ""} · 总耗时 ${formatTime(totalMs.value)} · x-apimanage Runner</div>
<div class="sum"><span>共 <b>${s.total}</b></span><span class="ok">通过 <b>${s.passed}</b></span><span class="${s.failed ? "bad" : "dim"}">失败 <b>${s.failed}</b></span></div>
<table><thead><tr><th></th><th>Method</th><th>名称</th><th>URL</th><th>状态</th><th>耗时</th><th>断言</th></tr></thead>
<tbody>${rows}</tbody></table></body></html>`;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const defaultName = `runner-${runMeta.value.collectionName || "report"}-${Date.now()}.html`;
    const path = await save({
      title: "导出 HTML 报告",
      defaultPath: defaultName.replace(/[\\/:*?"<>|]/g, "_"),
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
    if (!path) return;
    const { invoke: inv } = await import("@tauri-apps/api/core");
    await inv("write_text_file", { path, content: html });
    toast.push("已导出 HTML 报告", "success");
  } catch (e) {
    toast.push(String(e), "error", 5000);
  }
}
</script>

<template>
  <div class="h-full flex flex-col bg-app-bg min-w-0">
    <!-- 配置/汇总工具栏 -->
    <div class="px-3 py-2 border-b border-app-border bg-app-surface flex flex-col gap-2 flex-shrink-0">
      <div class="flex items-center gap-2">
        <PlayCircle :size="15" class="text-accent-orange flex-shrink-0" />
        <span class="text-[13px] font-medium flex-shrink-0">集合运行</span>
        <select
          v-model="selectedCollectionId"
          class="input w-52"
          :disabled="running"
        >
          <option :value="null" disabled>选择集合…</option>
          <option v-for="c in collections" :key="c.id" :value="c.id">{{ c.name }}</option>
        </select>

        <label class="flex items-center gap-1 text-[11px] text-app-muted select-none" title="重复运行的轮数（变量/测试脚本可跨轮累积）">
          迭代
          <input
            v-model.number="iterations"
            type="number"
            min="1"
            max="100"
            class="input w-14 text-[11px] px-1.5 font-mono"
            :disabled="running"
          />
        </label>
        <label class="flex items-center gap-1 text-[11px] text-app-muted select-none" title="每个请求之间的等待毫秒数，避免压垮目标服务">
          间隔
          <input
            v-model.number="delayMs"
            type="number"
            min="0"
            step="100"
            class="input w-16 text-[11px] px-1.5 font-mono"
            :disabled="running"
          />
          ms
        </label>

        <button
          v-if="!running"
          class="btn-primary"
          :disabled="!selectedCollectionId"
          @click="run(false)"
        >
          <PlayCircle :size="13" /> 运行
        </button>
        <button v-else class="btn bg-app-hover text-app-text hover:brightness-110" @click="stop">
          <Square :size="12" /> 停止
        </button>

        <div class="ml-auto flex items-center gap-1.5">
          <button
            v-if="canRerunFailed"
            class="btn-ghost border border-accent-red/40 hover:border-accent-red/70 hover:text-accent-red"
            title="仅重新运行上一轮失败的请求"
            @click="run(true)"
          >
            <RotateCcw :size="12" /> 重跑失败 ({{ summary.failed }})
          </button>
          <button
            v-if="items.length && !running"
            class="btn-ghost"
            title="导出 HTML 运行报告（自包含页面，可分享/存档）"
            @click="exportHtmlReport"
          >
            <FileCode :size="13" /> HTML
          </button>
          <button
            v-if="items.length && !running"
            class="btn-ghost"
            title="导出 JSON 运行报告"
            @click="exportReport"
          >
            <Download :size="13" /> JSON
          </button>
        </div>
      </div>

      <!-- 汇总 -->
      <div v-if="items.length" class="flex items-center gap-4 text-xs">
        <span class="text-app-muted">
          进度
          <span class="font-mono text-app-text">{{ summary.finished }}</span>
          / {{ summary.total }}
        </span>
        <span class="flex items-center gap-1 text-accent-green">
          <CheckCircle2 :size="12" /> {{ summary.passed }} 通过
        </span>
        <span v-if="summary.failed" class="flex items-center gap-1 text-accent-red">
          <XCircle :size="12" /> {{ summary.failed }} 失败
        </span>
        <span v-if="runMeta.iterations > 1" class="text-app-muted">
          共 {{ runMeta.iterations }} 轮迭代
        </span>
        <span v-if="totalMs" class="text-app-muted ml-auto font-mono tabular-nums">
          总耗时 {{ formatTime(totalMs) }}
        </span>
      </div>
    </div>

    <!-- 结果列表 -->
    <div class="flex-1 min-h-0 overflow-auto">
      <EmptyState
        v-if="!items.length"
        :icon="PlayCircle"
        title="尚未运行"
        hint="选择一个集合点击「运行」，或从集合右键菜单选择「运行集合」；可配置迭代轮数与请求间隔"
      />

      <template v-for="(item, idx) in items" :key="item.key">
        <!-- 迭代分组头（多轮时显示） -->
        <div
          v-if="runMeta.iterations > 1 && (idx === 0 || items[idx - 1].iteration !== item.iteration)"
          class="sticky top-0 z-10 flex items-center gap-2 px-3 h-6 bg-app-surface/95 backdrop-blur text-[10px] uppercase tracking-wider text-app-muted font-semibold border-b border-app-border/30"
        >
          第 {{ item.iteration }} 轮
        </div>

        <div
          class="flex items-center gap-2.5 px-3 h-8 border-b border-app-border/40 cursor-pointer hover:bg-app-hover/60 transition-colors"
          :class="{ 'bg-accent-red/5': itemFailed(item) && item.state !== 'pending' && item.state !== 'running' }"
          @click="toggleExpand(item.key)"
        >
          <!-- 状态图标 -->
          <span class="w-4 flex-shrink-0 flex items-center justify-center">
            <Loader2 v-if="item.state === 'running'" :size="13" class="animate-spin text-accent-blue" />
            <CircleAlert v-else-if="item.state === 'error'" :size="13" class="text-accent-red" />
            <XCircle v-else-if="itemFailed(item)" :size="13" class="text-accent-red" />
            <CheckCircle2 v-else-if="item.state === 'done'" :size="13" class="text-accent-green" />
            <span v-else class="w-1.5 h-1.5 rounded-full bg-app-border" />
          </span>

          <span
            class="font-mono text-[10px] font-bold w-8 flex-shrink-0"
            :class="methodColor(item.method)"
          >
            {{ item.method.slice(0, 4) }}
          </span>
          <span class="text-xs truncate flex-shrink-0 max-w-[220px]">{{ item.name }}</span>
          <span class="text-[11px] text-app-muted truncate flex-1 min-w-0 font-mono">{{ item.url }}</span>

          <span
            v-if="item.status"
            class="font-mono text-[11px] w-8 text-right flex-shrink-0"
            :class="statusColor(item.status)"
          >
            {{ item.status }}
          </span>
          <span v-if="item.timeMs != null" class="font-mono text-[11px] text-app-muted w-14 text-right flex-shrink-0">
            {{ formatTime(item.timeMs) }}
          </span>
          <span
            v-if="item.tests.length"
            class="text-[10px] px-1.5 rounded-full flex-shrink-0"
            :class="item.tests.some((t) => !t.passed) ? 'bg-accent-red/15 text-accent-red' : 'bg-accent-green/15 text-accent-green'"
          >
            {{ item.tests.filter((t) => t.passed).length }}/{{ item.tests.length }}
          </span>
          <ChevronDown
            :size="12"
            class="text-app-muted flex-shrink-0 transition-transform"
            :class="{ 'rotate-180': expandedKey === item.key }"
          />
        </div>

        <!-- 详情 -->
        <div
          v-if="expandedKey === item.key"
          class="px-3 py-2.5 border-b border-app-border bg-app-surface/60 flex flex-col gap-2"
        >
          <!-- 快捷操作：跳到该请求调试 -->
          <div class="flex items-center gap-2">
            <button class="btn-ghost btn-sm border border-app-border" @click="openInTab(item)">
              <ExternalLink :size="12" /> 在 Tab 中打开该请求
            </button>
          </div>

          <!-- 错误 -->
          <div v-if="item.error" class="text-xs text-accent-red flex items-start gap-1.5">
            <CircleAlert :size="13" class="flex-shrink-0 mt-0.5" />
            <span class="break-all">{{ item.error }}</span>
          </div>

          <!-- 测试断言 -->
          <div v-if="item.tests.length || item.testError">
            <div class="text-[11px] text-app-muted mb-1">Test 断言</div>
            <div
              v-for="(t, ti) in item.tests"
              :key="ti"
              class="flex items-center gap-1.5 text-xs py-0.5"
            >
              <component
                :is="t.passed ? CheckCircle2 : XCircle"
                :size="12"
                :class="t.passed ? 'text-accent-green' : 'text-accent-red'"
              />
              <span>{{ t.name }}</span>
              <span v-if="t.error" class="text-app-muted font-mono text-[11px] truncate">
                — {{ t.error }}
              </span>
            </div>
            <div v-if="item.testError" class="text-xs text-accent-yellow mt-1">
              脚本错误：{{ item.testError }}
            </div>
          </div>

          <!-- 响应预览 -->
          <div v-if="item.responsePreview">
            <div class="text-[11px] text-app-muted mb-1">响应（前 2000 字符）</div>
            <pre class="text-[11px] font-mono text-app-text/80 whitespace-pre-wrap break-all bg-app-bg border border-app-border rounded-md p-2 max-h-48 overflow-auto">{{ item.responsePreview }}</pre>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>
