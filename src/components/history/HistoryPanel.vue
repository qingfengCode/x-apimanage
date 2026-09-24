<script setup lang="ts">
import { computed, ref } from "vue";
import { Trash2, Clock, Search } from "lucide-vue-next";
import { useHistoryStore } from "@/stores/history";
import { useTabStore } from "@/stores/tab";
import { formatDateTime, statusColor, methodColor } from "@/utils/format";
import EmptyState from "@/components/common/EmptyState.vue";
import type { RequestSnapshot } from "@/types";

const historyStore = useHistoryStore();
const tabStore = useTabStore();

const query = ref("");
const methodFilter = ref<string>(""); // 空=全部
const statusFilter = ref<string>(""); // 空=全部; 2xx/3xx/4xx/5xx/error

const STATUS_GROUPS = [
  { value: "", label: "状态" },
  { value: "2xx", label: "2xx" },
  { value: "3xx", label: "3xx" },
  { value: "4xx", label: "4xx" },
  { value: "5xx", label: "5xx" },
  { value: "error", label: "错误" },
] as const;

function statusGroupOf(status: number | null): string {
  if (!status || status <= 0) return "error";
  if (status >= 200 && status < 300) return "2xx";
  if (status >= 300 && status < 400) return "3xx";
  if (status >= 400 && status < 500) return "4xx";
  if (status >= 500) return "5xx";
  return "error";
}

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  return historyStore.items.filter((it) => {
    if (methodFilter.value && (it.method || "").toUpperCase() !== methodFilter.value) return false;
    if (statusFilter.value && statusGroupOf(it.status) !== statusFilter.value) return false;
    if (!q) return true;
    return (
      (it.url || "").toLowerCase().includes(q) ||
      (it.method || "").toLowerCase().includes(q)
    );
  });
});

// ---- 按天分组（今天 / 昨天 / 具体日期） ----
function dayKey(ts: number): string {
  const d = new Date(ts);
  return `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}`;
}
function todayKey(): string {
  return dayKey(Date.now());
}
function yesterdayKey(): string {
  return dayKey(Date.now() - 86_400_000);
}
function groupLabel(key: string): string {
  if (key === todayKey()) return "今天";
  if (key === yesterdayKey()) return "昨天";
  const [y, m, d] = key.split("-").map(Number);
  return `${y}年${m}月${d}日`;
}

interface HistoryGroup {
  label: string;
  items: typeof filtered.value;
}
const grouped = computed<HistoryGroup[]>(() => {
  const sorted = [...filtered.value].sort((a, b) => b.createdAt - a.createdAt);
  const out: HistoryGroup[] = [];
  for (const item of sorted) {
    const key = dayKey(item.createdAt);
    const last = out[out.length - 1];
    if (last && dayKey(last.items[0].createdAt) === key) {
      last.items.push(item);
    } else {
      out.push({ label: groupLabel(key), items: [item] });
    }
  }
  return out;
});

/** 从历史恢复为新请求 Tab（快照逻辑统一在 history store，命令面板共用） */
function reopen(item: (typeof historyStore.items)[number]) {
  historyStore.restoreFromHistory(item);
}

/** 清空全部历史前二次确认（不可恢复） */
async function onClear() {
  const ok = await askDiscard("确定清空全部历史记录吗？此操作不可恢复。");
  if (ok) await historyStore.clear();
}

/** 原生/插件确认框（与 tab store 同款退化策略） */
async function askDiscard(message: string): Promise<boolean> {
  try {
    const { ask } = await import("@tauri-apps/plugin-dialog");
    return await ask(message, {
      title: "清空历史",
      kind: "warning",
      okLabel: "清空",
      cancelLabel: "取消",
    });
  } catch {
    return window.confirm(message);
  }
}
</script>

<template>
  <div class="flex flex-col h-full">
    <div class="panel-header">
      <span class="panel-title">History</span>
      <button
        v-if="historyStore.items.length"
        class="ml-auto icon-btn hover:text-accent-red hover:bg-accent-red/10"
        @click="onClear"
        title="清空（需确认）"
      >
        <Trash2 :size="13" />
      </button>
    </div>

    <!-- 搜索 + 过滤 -->
    <div class="px-2 py-1 border-b border-app-border flex items-center gap-1">
      <div class="relative flex-1">
        <Search :size="12" class="absolute left-2 top-1/2 -translate-y-1/2 text-app-muted" />
        <input
          v-model="query"
          placeholder="搜索 URL / 方法"
          spellcheck="false"
          class="w-full bg-app-surface2 border border-app-border rounded-md pl-7 pr-2 h-6 text-xs outline-none focus:border-accent-blue/70 placeholder:text-app-muted/60 transition-colors"
        />
      </div>
      <select
        v-model="methodFilter"
        class="bg-app-surface2 border border-app-border rounded-md px-1 h-6 text-[11px] outline-none focus:border-accent-blue/70"
      >
        <option value="">全部</option>
        <option v-for="m in ['GET','POST','PUT','DELETE','PATCH']" :key="m" :value="m">{{ m }}</option>
      </select>
      <select
        v-model="statusFilter"
        class="bg-app-surface2 border border-app-border rounded-md px-1 h-6 text-[11px] outline-none focus:border-accent-blue/70"
      >
        <option v-for="g in STATUS_GROUPS" :key="g.value" :value="g.value">{{ g.label }}</option>
      </select>
    </div>

    <!-- 列表（按天分组） -->
    <div class="flex-1 overflow-auto">
      <template v-for="group in grouped" :key="group.label">
        <!-- 组头 -->
        <div class="sticky top-0 z-10 flex items-center gap-2 px-3 py-1 bg-app-bg/95 backdrop-blur text-[10px] uppercase tracking-wider text-app-muted font-semibold border-b border-app-border/30">
          {{ group.label }}
          <span class="font-normal text-app-muted/60 normal-case tracking-normal">{{ group.items.length }} 条</span>
        </div>
        <div
          v-for="item in group.items"
          :key="item.id"
          class="px-2.5 py-1 hover:bg-app-hover cursor-pointer border-b border-app-border/30"
          :title="item.hasResponse
            ? '点击恢复请求与响应（可直接查看当时的响应内容）'
            : item.requestSnapshot
              ? '点击恢复完整请求（无响应快照）'
              : '点击恢复 URL（无快照）'"
          @click="reopen(item)"
        >
          <div class="flex items-center gap-2">
          <span
            class="font-mono text-[10px] font-bold w-[32px] flex-shrink-0"
            :class="methodColor(item.method || '')"
          >
            {{ (item.method || "GET").slice(0, 4) }}
          </span>
          <span
            v-if="item.status"
            class="font-mono text-[10px] w-[24px] flex-shrink-0"
            :class="statusColor(item.status)"
          >
            {{ item.status }}
          </span>
          <span
            v-if="item.hasResponse"
            class="text-[9px] px-1 rounded bg-accent-green/15 text-accent-green font-medium flex-shrink-0"
            title="含请求与响应快照：点击恢复后可直接查看当时的响应"
          >
            响应
          </span>
          <span
            v-else-if="item.requestSnapshot"
            class="w-1.5 h-1.5 rounded-full bg-accent-green flex-shrink-0"
            title="含请求快照"
          />
        </div>
        <div class="text-[11px] text-app-muted truncate mt-0.5 pl-[40px]">{{ item.url }}</div>
        <div class="text-[10px] text-app-muted/60 pl-[40px] mt-0.5">
          {{ formatDateTime(item.createdAt) }}
        </div>
        </div>
      </template>

      <div
        v-if="!filtered.length && historyStore.items.length"
        class="flex flex-col items-center justify-center py-8 text-app-muted text-xs gap-1 p-4"
      >
        <Search :size="22" :stroke-width="1" class="opacity-40" />
        <span>无匹配历史</span>
      </div>
      <EmptyState
        v-else-if="!historyStore.items.length"
        :icon="Clock"
        title="暂无历史"
        hint="发送请求后会自动记录"
      />
    </div>
  </div>
</template>
