<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  TransitionRoot,
  TransitionChild,
  Dialog,
  DialogPanel,
} from "@headlessui/vue";
import {
  Search,
  Send,
  FileText,
  Clock,
  Terminal,
  PlayCircle,
  Power,
  Layers,
  Plus,
  Upload,
  ChevronRight,
} from "lucide-vue-next";
import { useCollectionStore } from "@/stores/collection";
import { useDocsStore } from "@/stores/docs";
import { useHistoryStore } from "@/stores/history";
import { useTabStore } from "@/stores/tab";
import { useWorkspaceStore } from "@/stores/workspace";
import { useEnvironmentStore } from "@/stores/environment";
import { useMockStore } from "@/stores/mock";
import type { RequestItem } from "@/types";

/**
 * 命令面板（Ctrl+P）：跨请求/文档/历史/命令的模糊搜索与跳转。
 * 数据在打开时快照（打开期间列表变化不影响本次结果），执行后关闭。
 */

interface PaletteItem {
  key: string;
  group: string;
  label: string;
  hint?: string;
  icon: any;
  iconClass?: string;
  keywords?: string;
  action: () => void;
}

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const collectionStore = useCollectionStore();
const docsStore = useDocsStore();
const historyStore = useHistoryStore();
const tabStore = useTabStore();
const workspaceStore = useWorkspaceStore();
const envStore = useEnvironmentStore();
const mockStore = useMockStore();

const query = ref("");
const activeIdx = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLElement | null>(null);

/** 打开时构建候选（含打开集合中的全部请求） */
const items = computed<PaletteItem[]>(() => {
  if (!props.show) return [];
  const out: PaletteItem[] = [];

  // 命令
  out.push(
    {
      key: "cmd:new",
      group: "命令",
      label: "新建请求",
      hint: "Ctrl+T",
      icon: Plus,
      action: () => tabStore.newTab(),
    },
    {
      key: "cmd:curl",
      group: "命令",
      label: "导入 cURL 命令",
      icon: Terminal,
      action: () => window.dispatchEvent(new CustomEvent("open:curl-import")),
    },
    {
      key: "cmd:io",
      group: "命令",
      label: "导入 Postman / OpenAPI",
      icon: Upload,
      action: () => window.dispatchEvent(new CustomEvent("open:import-export")),
    },
    {
      key: "cmd:runner",
      group: "命令",
      label: "集合运行 Runner",
      icon: PlayCircle,
      action: () => workspaceStore.openRunner(),
    },
    {
      key: "cmd:mock",
      group: "命令",
      label: "Mock 服务",
      icon: Power,
      action: () => window.dispatchEvent(new CustomEvent("open:mock-panel")),
    },
    {
      key: "cmd:mock-toggle",
      group: "命令",
      label: mockStore.serverUrl ? "停止 Mock 服务" : "启动 Mock 服务",
      icon: Power,
      keywords: "mock start stop 启动 停止",
      action: () => {
        if (mockStore.serverUrl) mockStore.stop().catch(() => {});
        else mockStore.start().catch(() => {});
      },
    }
  );

  // 环境切换
  for (const env of envStore.environments) {
    out.push({
      key: `env:${env.id}`,
      group: "切换环境",
      label: env.name,
      hint: env.isActive ? "当前" : undefined,
      icon: Layers,
      iconClass: env.isActive ? "text-accent-green" : undefined,
      keywords: `environment ${env.name}`,
      action: () => {
        envStore.setActive(env.id).catch(() => {});
      },
    });
  }

  // 集合请求
  for (const colId of Object.keys(collectionStore.requests)) {
    const col = collectionStore.collections.find((c) => c.id === colId);
    for (const r of collectionStore.requests[colId] as RequestItem[]) {
      out.push({
        key: `req:${r.id}`,
        group: "请求",
        label: r.name,
        hint: `${r.method} ${r.url || ""}`.slice(0, 80),
        icon: Send,
        iconClass: "text-accent-blue",
        keywords: `${r.method} ${r.name} ${r.url ?? ""} ${col?.name ?? ""}`,
        action: () => tabStore.openRequest(r),
      });
    }
  }

  // 文档
  for (const d of docsStore.documents) {
    out.push({
      key: `doc:${d.id}`,
      group: "文档",
      label: d.title,
      hint: d.docType === "product" ? "产品文档" : "API 文档",
      icon: FileText,
      iconClass: d.docType === "product" ? "text-accent-purple" : "text-accent-blue",
      keywords: d.title,
      action: () => tabStore.openDocTab(d),
    });
  }

  // 最近历史
  for (const h of historyStore.items.slice(0, 20)) {
    out.push({
      key: `his:${h.id}`,
      group: "历史",
      label: `${h.method || "GET"} ${h.url || ""}`.slice(0, 90),
      hint: h.requestSnapshot ? "含完整快照" : undefined,
      icon: Clock,
      keywords: `${h.method ?? ""} ${h.url ?? ""}`,
      action: () => historyStore.restoreFromHistory(h),
    });
  }

  return out;
});

/** 轻量模糊匹配：子序列命中计分，连续命中与词首命中加权 */
function fuzzyScore(query: string, target: string): number | null {
  if (!query) return 0;
  const q = query.toLowerCase();
  const t = target.toLowerCase();
  const direct = t.indexOf(q);
  if (direct >= 0) return 1000 - direct; // 子串直接命中优先
  let ti = 0;
  let score = 0;
  let streak = 0;
  for (const ch of q) {
    const found = t.indexOf(ch, ti);
    if (found === -1) return null;
    streak = found === ti ? streak + 1 : 0;
    score += 10 + streak * 5 - Math.min(found - ti, 5);
    ti = found + 1;
  }
  return score;
}

const filtered = computed(() => {
  const q = query.value.trim();
  if (!q) {
    // 空查询：命令 → 环境 → 请求（前 8）→ 文档（前 5）→ 历史（前 5）
    const byGroup = (g: string, n: number) =>
      items.value.filter((i) => i.group === g).slice(0, n);
    return [...byGroup("命令", 99), ...byGroup("切换环境", 99), ...byGroup("请求", 8), ...byGroup("文档", 5), ...byGroup("历史", 5)];
  }
  return items.value
    .map((it) => ({ it, s: fuzzyScore(q, it.keywords || it.label) }))
    .filter((x): x is { it: PaletteItem; s: number } => x.s != null)
    .sort((a, b) => b.s - a.s)
    .slice(0, 30)
    .map((x) => x.it);
});

watch(filtered, () => (activeIdx.value = 0));

watch(
  () => props.show,
  async (v) => {
    if (v) {
      query.value = "";
      activeIdx.value = 0;
      await nextTick();
      inputRef.value?.focus();
    }
  }
);

function execute(item: PaletteItem) {
  emit("update:show", false);
  item.action();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    activeIdx.value = Math.min(activeIdx.value + 1, filtered.value.length - 1);
    scrollActiveIntoView();
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    activeIdx.value = Math.max(activeIdx.value - 1, 0);
    scrollActiveIntoView();
  } else if (e.key === "Enter") {
    e.preventDefault();
    const it = filtered.value[activeIdx.value];
    if (it) execute(it);
  }
}

function scrollActiveIntoView() {
  nextTick(() => {
    const el = listRef.value?.querySelector<HTMLElement>(`[data-idx="${activeIdx.value}"]`);
    el?.scrollIntoView({ block: "nearest" });
  });
}

function onWindowKeydown(e: KeyboardEvent) {
  if (props.show && e.key === "Escape") emit("update:show", false);
}
onMounted(() => window.addEventListener("keydown", onWindowKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onWindowKeydown));
</script>

<template>
  <TransitionRoot appear :show="show" as="template">
    <Dialog as="div" class="relative z-50" @close="emit('update:show', false)">
      <TransitionChild
        as="template"
        enter="duration-150 ease-out"
        enter-from="opacity-0"
        enter-to="opacity-100"
        leave="duration-100 ease-in"
        leave-from="opacity-100"
        leave-to="opacity-0"
      >
        <div class="fixed inset-0 bg-black/40" />
      </TransitionChild>

      <div class="fixed inset-x-0 top-[12vh] flex items-start justify-center px-4 pointer-events-none">
        <TransitionChild
          as="template"
          enter="duration-150 ease-out"
          enter-from="opacity-0 scale-95 -translate-y-2"
          enter-to="opacity-100 scale-100 translate-y-0"
          leave="duration-100 ease-in"
          leave-from="opacity-100 scale-100"
          leave-to="opacity-0 scale-95"
        >
          <DialogPanel class="w-[560px] max-w-full pop pointer-events-auto overflow-hidden">
            <!-- 搜索框 -->
            <div class="flex items-center gap-2 px-3 h-11 border-b border-app-border">
              <Search :size="15" class="text-app-muted flex-shrink-0" />
              <input
                ref="inputRef"
                v-model="query"
                placeholder="搜索请求 / 文档 / 历史，或执行命令…"
                spellcheck="false"
                class="flex-1 bg-transparent text-sm outline-none placeholder:text-app-muted/60"
                @keydown="onKeydown"
              />
              <span class="text-[10px] text-app-muted/60 flex-shrink-0 border border-app-border rounded px-1.5 py-0.5">Esc</span>
            </div>

            <!-- 结果列表 -->
            <div ref="listRef" class="max-h-[46vh] overflow-auto py-1">
              <div
                v-for="(it, idx) in filtered"
                :key="it.key"
                :data-idx="idx"
                class="flex items-center gap-2.5 mx-1.5 px-2.5 h-8 rounded-md cursor-pointer text-xs"
                :class="idx === activeIdx ? 'bg-app-hover' : 'hover:bg-app-hover/60'"
                @click="execute(it)"
                @mousemove="activeIdx = idx"
              >
                <component
                  :is="it.icon"
                  :size="13"
                  class="flex-shrink-0"
                  :class="it.iconClass ?? 'text-app-muted'"
                />
                <span class="truncate flex-shrink-0 max-w-[45%]">{{ it.label }}</span>
                <span v-if="it.hint" class="text-[11px] text-app-muted truncate flex-1 min-w-0 font-mono">
                  {{ it.hint }}
                </span>
                <span v-else class="flex-1" />
                <span class="text-[10px] text-app-muted/70 flex-shrink-0">{{ it.group }}</span>
                <ChevronRight :size="11" class="text-app-muted/50 flex-shrink-0" :class="idx === activeIdx ? 'opacity-100' : 'opacity-0'" />
              </div>

              <div v-if="!filtered.length" class="py-8 text-center text-xs text-app-muted">
                没有匹配的结果
              </div>
            </div>

            <!-- 底部提示 -->
            <div class="flex items-center gap-3 px-3 h-7 border-t border-app-border text-[10px] text-app-muted/70 flex-shrink-0">
              <span>↑↓ 选择</span>
              <span>Enter 执行</span>
              <span class="ml-auto">Ctrl+P 打开/关闭</span>
            </div>
          </DialogPanel>
        </TransitionChild>
      </div>
    </Dialog>
  </TransitionRoot>
</template>
