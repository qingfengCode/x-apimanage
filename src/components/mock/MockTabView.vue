<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { Trash2, Power, Copy, Check, Loader2 } from "lucide-vue-next";
import { useMockStore } from "@/stores/mock";
import { useTabStore } from "@/stores/tab";
import { copyText } from "@/utils/curl";
import { useToast } from "@/composables/useToast";
import type { KeyValue } from "@/types";
import MonacoEditor from "@/components/editor/MonacoEditor.vue";
import KeyValueEditor from "@/components/common/KeyValueEditor.vue";

/**
 * Mock 路由 Tab 的内容视图：编辑一条路由的响应配置。
 * 路由数据以 mockStore 为唯一来源（本组件不复制状态），
 * 修改经 400ms 防抖自动落库；路由在列表中被删除时自动关闭本 Tab。
 */
const mockStore = useMockStore();
const tabStore = useTabStore();
const toast = useToast();

const tab = computed(() => tabStore.activeMockTab);
const route = computed(() =>
  tab.value ? mockStore.routes.find((r) => r.id === tab.value!.routeId) ?? null : null
);
const view = ref<"body" | "headers">("body");

// ---- 编辑与自动保存：改动合并进 pending，空闲 400ms 后整行落库 ----
// pendingId 在 onField 时捕获：切换 Tab/路由触发的 flush 发生在路由变更之后，
// 若按当前 route 落库会把上一个路由的编辑写到新路由上
let timer: ReturnType<typeof setTimeout> | undefined;
let pending: Record<string, unknown> = {};
let pendingId: string | null = null;
const saveState = ref<"idle" | "saving" | "saved">("idle");

function onField(patch: Record<string, unknown>) {
  pending = { ...pending, ...patch };
  pendingId = route.value?.id ?? pendingId;
  saveState.value = "idle";
  if (timer) clearTimeout(timer);
  timer = setTimeout(flush, 400);
}

async function flush() {
  if (timer) {
    clearTimeout(timer);
    timer = undefined;
  }
  const patch = pending;
  const id = pendingId;
  pending = {};
  pendingId = null;
  if (!Object.keys(patch).length || !id) return;
  saveState.value = "saving";
  try {
    const cur = mockStore.routes.find((r) => r.id === id);
    if (!cur) return; // 路由已被删除则不写（避免复活）
    await mockStore.save({
      id: cur.id,
      method: (patch.method as string) ?? cur.method,
      path: (patch.path as string) ?? cur.path,
      status: (patch.status as number) ?? cur.status,
      responseHeaders: (patch.responseHeaders as string) ?? cur.responseHeaders,
      responseBody: (patch.responseBody as string) ?? cur.responseBody,
      delayMs: (patch.delayMs as number) ?? cur.delayMs,
      enabled: (patch.enabled as boolean) ?? cur.enabled,
    });
    saveState.value = "saved";
    setTimeout(() => {
      if (saveState.value === "saved") saveState.value = "idle";
    }, 1500);
  } catch (e) {
    saveState.value = "idle";
    toast.push(`Mock 路由保存失败：${String(e)}`, "error", 5000);
  }
}

// 切换到其它 Tab（含 mock→mock）时把未落库的编辑刷出去；
// 路由被删除时静默关闭本 Tab
watch(
  () => [tabStore.activeTabId, route.value?.id] as const,
  () => {
    void flush();
    if (tab.value && !route.value && mockStore.loaded) {
      tabStore.closeTab(tab.value.id);
    }
  }
);
onBeforeUnmount(() => {
  if (timer) clearTimeout(timer);
  void flush();
});

// ---- 响应头（JSON 字符串 <-> KeyValue[]） ----
const headerItems = computed<KeyValue[]>(() => {
  if (!route.value?.responseHeaders) return [];
  try {
    const parsed = JSON.parse(route.value.responseHeaders);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
});

function onHeadersChange(items: KeyValue[]) {
  onField({
    responseHeaders: JSON.stringify(items.filter((h) => h.key)),
  });
}

// ---- 测试地址（服务运行中时给出完整 URL） ----
const testUrl = computed(() => {
  if (!mockStore.serverUrl || !route.value) return "";
  return mockStore.serverUrl + route.value.path;
});
const copiedUrl = ref(false);
async function copyTestUrl() {
  if (testUrl.value && (await copyText(testUrl.value))) {
    copiedUrl.value = true;
    setTimeout(() => (copiedUrl.value = false), 1500);
  }
}

async function removeRoute() {
  if (!route.value || !window.confirm(`删除路由 ${route.value.method} ${route.value.path}？`)) return;
  const id = route.value.id;
  await mockStore.remove(id);
  tabStore.closeMockTabByRouteId(id);
}
</script>

<template>
  <div class="h-full flex flex-col bg-app-bg min-w-0">
    <!-- 路由已被删除（列表侧删除但本 Tab 未及自动关闭的兜底） -->
    <div
      v-if="!route"
      class="h-full flex flex-col items-center justify-center text-app-muted gap-2"
    >
      <Power :size="22" :stroke-width="1.3" class="opacity-50" />
      <span class="text-xs">该 Mock 路由已不存在</span>
    </div>

    <template v-else>
      <!-- 基本信息工具栏 -->
      <div class="px-3 py-2 border-b border-app-border bg-app-surface flex flex-col gap-1.5 flex-shrink-0">
        <div class="flex items-center gap-2">
          <select
            :value="route.method"
            class="input w-[76px] font-mono font-bold px-1.5 flex-shrink-0"
            @change="(e) => onField({ method: (e.target as HTMLSelectElement).value })"
          >
            <option v-for="m in ['GET','POST','PUT','DELETE','PATCH','ANY']" :key="m" :value="m">{{ m }}</option>
          </select>
          <input
            :value="route.path"
            placeholder="/api/path/:id"
            spellcheck="false"
            class="input flex-1 min-w-0 font-mono"
            @change="(e) => onField({ path: (e.target as HTMLInputElement).value })"
          />
          <label class="flex items-center gap-1.5 text-xs text-app-muted select-none flex-shrink-0" title="启用后参与 Mock 匹配">
            <input
              type="checkbox"
              :checked="route.enabled"
              @change="onField({ enabled: !route.enabled })"
            />
            启用
          </label>
          <button
            class="icon-btn hover:text-accent-red hover:bg-accent-red/10 flex-shrink-0"
            title="删除路由"
            @click="removeRoute"
          >
            <Trash2 :size="14" />
          </button>
        </div>

        <div class="flex items-center gap-2">
          <label class="flex items-center gap-1.5 text-xs text-app-muted select-none">
            状态码
            <input
              :value="route.status"
              type="number"
              class="input w-16 text-[11px] px-1.5 font-mono"
              @change="(e) => onField({ status: Number((e.target as HTMLInputElement).value) })"
            />
          </label>
          <label class="flex items-center gap-1.5 text-xs text-app-muted select-none" title="响应前等待的毫秒数，用于模拟慢接口">
            延迟
            <input
              :value="route.delayMs"
              type="number"
              class="input w-20 text-[11px] px-1.5 font-mono"
              @change="(e) => onField({ delayMs: Number((e.target as HTMLInputElement).value) })"
            />
            ms
          </label>

          <!-- 自动保存指示 -->
          <span class="text-[11px] text-app-muted/70 flex items-center gap-1">
            <Loader2 v-if="saveState === 'saving'" :size="11" class="animate-spin" />
            <Check v-else-if="saveState === 'saved'" :size="11" class="text-accent-green" />
            {{
              saveState === "saving" ? "保存中" : saveState === "saved" ? "已自动保存" : "修改后自动保存"
            }}
          </span>

          <!-- 测试地址 -->
          <button
            v-if="testUrl"
            class="ml-auto flex items-center gap-1.5 max-w-[46%] px-2 h-6 rounded-md bg-app-surface2 border border-app-border text-[11px] font-mono text-accent-green/90 hover:border-accent-green/50 transition-colors"
            :title="copiedUrl ? '已复制' : `复制测试地址：${testUrl}`"
            @click="copyTestUrl"
          >
            <Check v-if="copiedUrl" :size="11" class="flex-shrink-0" />
            <Copy v-else :size="11" class="flex-shrink-0" />
            <span class="truncate">{{ testUrl }}</span>
          </button>
        </div>
      </div>

      <!-- Body / Headers 切换 -->
      <div class="flex items-center gap-1 px-2.5 h-8 border-b border-app-border/60 flex-shrink-0">
        <button class="seg" :class="view === 'body' ? 'seg-active' : ''" @click="view = 'body'">
          响应 Body
        </button>
        <button class="seg" :class="view === 'headers' ? 'seg-active' : ''" @click="view = 'headers'">
          响应 Headers
          <span v-if="headerItems.filter((h) => h.key).length" class="text-[10px] bg-app-hover px-1.5 rounded-full">
            {{ headerItems.filter((h) => h.key).length }}
          </span>
        </button>
      </div>

      <!-- 编辑区 -->
      <div class="flex-1 min-h-0">
        <MonacoEditor
          v-if="view === 'body'"
          :model-value="route.responseBody"
          language="json"
          @update:model-value="(v: string) => onField({ responseBody: v })"
        />
        <div v-else class="h-full flex flex-col">
          <div class="px-3 h-7 flex items-center text-[11px] text-app-muted border-b border-app-border/40 flex-shrink-0">
            响应头（原样返回给客户端）
          </div>
          <div class="flex-1 min-h-0">
            <KeyValueEditor
              :model-value="headerItems"
              key-placeholder="Header 名（如 Content-Type）"
              value-placeholder="值"
              @update:model-value="onHeadersChange"
            />
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
