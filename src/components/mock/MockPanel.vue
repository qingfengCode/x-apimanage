<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Play, Square, Plus, Trash2, Power, Copy, RefreshCw, ScrollText } from "lucide-vue-next";
import { useMockStore } from "@/stores/mock";
import { useCollectionStore } from "@/stores/collection";
import { useTabStore } from "@/stores/tab";
import type { MockRoute } from "@/types";
import EmptyState from "@/components/common/EmptyState.vue";
import { statusColor } from "@/utils/format";

const mockStore = useMockStore();
const collectionStore = useCollectionStore();
const tabStore = useTabStore();

const portInput = ref<number | null>(null);
const starting = ref(false);

/** 当前激活 Tab 打开的路由 id（列表高亮） */
const activeRouteId = computed(() => tabStore.activeMockTab?.routeId ?? null);

// ---- 请求日志：服务运行时轮询刷新 ----
const showLogs = ref(false);
let logsTimer: ReturnType<typeof setInterval> | undefined;
onMounted(() => {
  logsTimer = setInterval(() => {
    // 仅服务运行中拉取；面板在所有侧栏模式下都常驻（v-show），开销可控
    if (mockStore.serverUrl) {
      mockStore.loadLogs();
    }
  }, 3000);
});
onBeforeUnmount(() => {
  if (logsTimer) clearInterval(logsTimer);
});

function formatLogTime(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

async function toggleServer() {
  if (mockStore.serverUrl) {
    await mockStore.stop();
  } else {
    starting.value = true;
    try {
      await mockStore.start(portInput.value ?? undefined);
    } catch (e) {
      alert(`启动失败：${String(e)}`);
    } finally {
      starting.value = false;
    }
  }
}

/** 点击路由 → 以标签页打开编辑（已打开则激活），与请求/文档 Tab 共存 */
function selectRoute(r: MockRoute) {
  tabStore.openMockTab(r.id);
}

async function addRoute() {
  const saved = await mockStore.save({
    method: "GET",
    path: "/hello",
    status: 200,
    responseHeaders: JSON.stringify([{ key: "Content-Type", value: "application/json" }]),
    responseBody: JSON.stringify({ message: "Hello from mock!" }),
    delayMs: 0,
    enabled: true,
  });
  tabStore.openMockTab(saved.id);
}

async function toggleRoute(r: MockRoute) {
  await mockStore.save({
    id: r.id,
    method: r.method,
    path: r.path,
    status: r.status,
    responseHeaders: r.responseHeaders,
    responseBody: r.responseBody,
    delayMs: r.delayMs,
    enabled: !r.enabled,
  });
}

async function removeRoute(id: string, path: string) {
  if (!window.confirm(`删除路由 ${path}？`)) return;
  await mockStore.remove(id);
  tabStore.closeMockTabByRouteId(id);
}

/** 从现有集合请求一键生成 mock 路由 */
async function generateFromCollection() {
  const cols = collectionStore.collections.filter((c) => c.kind === "collection");
  if (!cols.length) {
    alert("暂无集合可导入");
    return;
  }
  const colName = window.prompt("从集合生成 mock 路由。输入集合名称：", cols[0].name);
  if (!colName) return;
  const col = cols.find((c) => c.name === colName);
  if (!col) {
    alert("未找到该集合");
    return;
  }
  const reqs = await invoke<any[]>("list_requests_recursive", { collectionId: col.id });
  let count = 0;
  for (const r of reqs) {
    let path = "/";
    try {
      path = new URL(r.url || "/").pathname || "/";
    } catch {
      const m = (r.url || "").match(/\/[^?\s]*/);
      if (m) path = m[0];
    }
    await mockStore.save({
      method: (r.method || "GET").toUpperCase(),
      path,
      status: 200,
      responseHeaders: JSON.stringify([{ key: "Content-Type", value: "application/json" }]),
      responseBody: JSON.stringify({ mocked: true, from: r.name }),
      delayMs: 0,
      enabled: true,
    });
    count++;
  }
  alert(`已生成 ${count} 个 mock 路由`);
}
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- 标题 + 服务控制 -->
    <div class="px-2 py-1.5 border-b border-app-border">
      <div class="flex items-center h-6">
        <Power :size="13" class="mr-1.5" :class="mockStore.serverUrl ? 'text-accent-green' : 'text-app-muted'" />
        <span class="panel-title">Mock Server</span>
      </div>

      <div class="flex items-center gap-1.5 mt-1">
        <input
          v-model.number="portInput"
          type="number"
          placeholder="端口（留空自动）"
          class="input w-28"
          min="0"
          max="65535"
          :disabled="!!mockStore.serverUrl"
        />
        <button
          class="btn-sm flex items-center gap-1 rounded-md text-white px-3"
          :class="mockStore.serverUrl ? 'bg-accent-red hover:brightness-110' : 'bg-accent-green hover:brightness-110'"
          :disabled="starting"
          @click="toggleServer"
        >
          <component :is="mockStore.serverUrl ? Square : Play" :size="12" />
          {{ mockStore.serverUrl ? "停止" : starting ? "启动中" : "启动" }}
        </button>
      </div>

      <div v-if="mockStore.serverUrl" class="mt-1.5 text-[11px] flex items-center gap-1">
        <span class="w-1.5 h-1.5 rounded-full bg-accent-green animate-pulse flex-shrink-0" />
        <code class="text-accent-green font-mono truncate">{{ mockStore.serverUrl }}</code>
      </div>
    </div>

    <!-- 工具条 -->
    <div class="flex items-center gap-0.5 px-2 h-7 border-b border-app-border">
      <button class="btn-ghost btn-sm" @click="addRoute"><Plus :size="12" /> 新建</button>
      <button class="btn-ghost btn-sm" @click="generateFromCollection" title="从集合的请求生成">
        <Copy :size="12" /> 从集合生成
      </button>
      <button class="btn-ghost btn-sm ml-auto" @click="mockStore.refreshRoutes()" title="刷新">
        <RefreshCw :size="12" />
      </button>
    </div>

    <!-- 路由列表（点击选中，编辑在主区） -->
    <div class="flex-1 overflow-auto">
      <div
        v-for="r in mockStore.routes"
        :key="r.id"
        class="group flex items-center gap-2 mx-1 px-2 h-7 rounded-md cursor-pointer hover:bg-app-hover"
        :class="{ 'bg-app-hover': activeRouteId === r.id }"
        @click="selectRoute(r)"
      >
        <input
          type="checkbox"
          :checked="r.enabled"
          title="启用/停用"
          @click.stop
          @change.stop="toggleRoute(r)"
        />
        <span
          class="font-mono text-[10px] font-bold w-[46px] flex-shrink-0 px-1 rounded bg-app-surface2 text-center"
          :class="r.enabled ? 'text-accent-orange' : 'text-app-muted'"
        >
          {{ r.method }}
        </span>
        <code class="text-[11px] font-mono flex-1 truncate" :class="r.enabled ? 'text-app-text' : 'text-app-muted'">
          {{ r.path }}
        </code>
        <span class="text-[11px] text-app-muted tabular-nums flex-shrink-0">{{ r.status }}</span>
        <button
          class="w-5 h-5 flex items-center justify-center rounded text-app-muted hover:text-accent-red hover:bg-accent-red/10 opacity-0 group-hover:opacity-100 transition-all"
          title="删除路由"
          @click.stop="removeRoute(r.id, r.path)"
        >
          <Trash2 :size="12" />
        </button>
      </div>

      <EmptyState
        v-if="!mockStore.routes.length"
        :icon="Power"
        title="暂无 mock 路由"
        hint="点击「新建」或「从集合生成」"
      />
    </div>

    <!-- 请求日志（服务运行时显示） -->
    <div v-if="mockStore.serverUrl" class="flex-shrink-0 border-t border-app-border flex flex-col max-h-64">
      <button
        class="flex items-center gap-1.5 px-2.5 h-7 text-[11px] text-app-muted hover:text-app-text hover:bg-app-hover/60 transition-colors"
        @click="showLogs = !showLogs; mockStore.loadLogs()"
      >
        <ScrollText :size="12" />
        请求日志
        <span class="text-[10px] bg-app-hover px-1.5 rounded-full">{{ mockStore.logs.length }}</span>
        <span
          v-if="showLogs"
          class="ml-auto icon-btn !w-5 !h-5"
          title="清空日志"
          @click.stop="mockStore.clearLogs()"
        >
          <Trash2 :size="11" />
        </span>
      </button>
      <div v-show="showLogs" class="overflow-auto flex-1 min-h-0 pb-1">
        <div
          v-for="log in mockStore.logs"
          :key="log.id"
          class="flex items-center gap-2 px-2.5 h-6 text-[11px] hover:bg-app-hover/50"
          :title="log.matchedPattern
            ? `命中路由 ${log.method} ${log.matchedPattern}（延迟 ${log.delayMs}ms）`
            : '未命中任何路由（404）'"
        >
          <span class="text-app-muted/70 font-mono w-14 flex-shrink-0 tabular-nums">{{ formatLogTime(log.ts) }}</span>
          <span class="font-mono font-bold w-[38px] flex-shrink-0" :class="log.matchedPattern ? 'text-accent-orange' : 'text-app-muted'">
            {{ log.method.slice(0, 5) }}
          </span>
          <span class="font-mono truncate flex-1 min-w-0" :class="log.matchedPattern ? 'text-app-text/90' : 'text-app-muted'">
            {{ log.path }}
          </span>
          <span class="font-mono flex-shrink-0" :class="log.status === 404 ? 'text-accent-red' : statusColor(log.status)">
            {{ log.status }}
          </span>
        </div>
        <div v-if="!mockStore.logs.length" class="py-3 text-center text-[11px] text-app-muted/70">
          暂无请求，服务运行中被访问后在此显示
        </div>
      </div>
    </div>
  </div>
</template>
