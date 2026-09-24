<script setup lang="ts">
import { computed, ref } from "vue";
import { X, XCircle, ArrowRight, Ban, Save, Plus, Bot, ClipboardPaste, Plug, FileText, Package, Copy, Power } from "lucide-vue-next";
import { useTabStore, docTabDirty } from "@/stores/tab";
import { useAiStore } from "@/stores/ai";
import { useCollectionStore } from "@/stores/collection";
import { useMockStore } from "@/stores/mock";
import { methodColor } from "@/utils/format";
import ContextMenu, { type MenuItem } from "@/components/common/ContextMenu.vue";

const emit = defineEmits<{ (e: "importCurl"): void }>();

const tabStore = useTabStore();
const aiStore = useAiStore();
const collectionStore = useCollectionStore();
const mockStore = useMockStore();
const tabs = computed(() => tabStore.tabs);
const contextMenu = ref<InstanceType<typeof ContextMenu> | null>(null);

/** mock 标签的显示名：METHOD path */
function mockTabLabel(routeId: string): string {
  const r = mockStore.routes.find((x) => x.id === routeId);
  if (!r) return "Mock 路由";
  return `${r.method} ${r.path}`;
}

/** 复制请求：已保存的复制为同集合副本并打开；临时草稿复制为新草稿 Tab */
async function duplicateTab(id: string) {
  const tab = tabs.value.find((t) => t.id === id);
  if (!tab || tab.kind !== "request") return;
  if (tab.draft.collectionId) {
    const saved = await collectionStore.duplicateRequestById(tab.draft.id);
    if (saved) tabStore.openRequest(saved);
  } else {
    tabStore.openDraft({
      ...JSON.parse(JSON.stringify(tab.draft)),
      name: `${tab.draft.name} (副本)`,
    });
  }
}

/** Tab 是否有未保存改动（请求草稿 / 文档编辑；mock 自动保存恒为否） */
function isDirty(tab: (typeof tabs.value)[number]): boolean {
  return tab.kind === "request" ? tab.draft.dirty : tab.kind === "doc" ? docTabDirty(tab) : false;
}

function close(id: string, e: MouseEvent) {
  e.stopPropagation();
  tabStore.closeTabWithConfirm(id);
}

/** 右键菜单：关闭 / 关闭其他 / 关闭右侧 / 关闭全部 */
function openMenu(id: string, e: MouseEvent) {
  const items: MenuItem[] = [
    { label: "关闭", icon: X, onClick: () => tabStore.closeTabWithConfirm(id) },
  ];
  if (tabs.value.length > 1) {
    items.push(
      { label: "关闭其他", icon: XCircle, onClick: () => tabStore.closeOthers(id) },
      { divider: true },
      { label: "关闭右侧", icon: ArrowRight, onClick: () => tabStore.closeRight(id) },
      { divider: true },
      { label: "关闭全部", icon: Ban, onClick: () => tabStore.closeAll() }
    );
  }
  // 当前 Tab 是集合请求（非临时草稿）时提供保存入口
  const tab = tabs.value.find((t) => t.id === id);
  if (tab && tab.kind === "request" && tab.draft.collectionId && tab.draft.dirty) {
    items.push({ divider: true });
    items.push({
      label: "保存到集合",
      icon: Save,
      onClick: () => {
        tabStore.setActive(id);
        window.dispatchEvent(new CustomEvent("shortcut:save"));
      },
    });
  }
  // 请求 Tab 提供「复制请求」
  if (tab && tab.kind === "request") {
    items.push({ divider: true });
    items.push({ label: "复制请求", icon: Copy, onClick: () => void duplicateTab(id) });
  }
  contextMenu.value?.open(e, items);
}

// ---- 拖拽排序 ----
const dragIndex = ref<number | null>(null);
const dropIndex = ref<number | null>(null);

function onDragStart(idx: number, e: DragEvent) {
  dragIndex.value = idx;
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "move";
    // Firefox 需要 setData 才能触发拖拽
    e.dataTransfer.setData("text/plain", String(idx));
  }
}

function onDragOver(idx: number, e: DragEvent) {
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  if (dragIndex.value !== null && dragIndex.value !== idx) {
    dropIndex.value = idx;
  }
}

function onDrop(idx: number, e: DragEvent) {
  e.preventDefault();
  const from = dragIndex.value;
  if (from === null || from === idx) {
    resetDrag();
    return;
  }
  // 重排 tabs 数组
  const arr = tabStore.tabs.slice();
  const [moved] = arr.splice(from, 1);
  arr.splice(idx, 0, moved);
  tabStore.tabs = arr;
  resetDrag();
}

function onDragEnd() {
  resetDrag();
}

function resetDrag() {
  dragIndex.value = null;
  dropIndex.value = null;
}
</script>

<template>
  <div class="flex items-stretch h-8 border-b border-app-border bg-app-surface flex-shrink-0">
    <!-- 左：tabs 滚动区 -->
    <div class="flex items-stretch gap-0.5 px-1.5 overflow-x-auto flex-1 min-w-0">
    <div
      v-for="(tab, idx) in tabs"
      :key="tab.id"
      draggable="true"
      class="group flex items-center gap-1.5 pl-2 pr-1 my-1 rounded-md cursor-pointer min-w-[120px] max-w-[220px] text-xs whitespace-nowrap select-none transition-colors"
      :class="[
        tab.id === tabStore.activeTabId
          ? 'bg-app-bg text-app-text shadow-sm'
          : 'text-app-muted hover:bg-app-hover/60 hover:text-app-text',
        dropIndex === idx && dragIndex !== idx ? 'tab-drop-indicator' : '',
        dragIndex === idx ? 'opacity-50' : '',
      ]"
      @click="tabStore.setActive(tab.id)"
      @contextmenu="(e) => openMenu(tab.id, e)"
      @dragstart="(e) => onDragStart(idx, e)"
      @dragover="(e) => onDragOver(idx, e)"
      @drop="(e) => onDrop(idx, e)"
      @dragend="onDragEnd"
    >
      <!-- 请求 Tab：方法徽标；文档 Tab：类型图标；Mock Tab：电源图标 -->
      <span
        v-if="tab.kind === 'request'"
        class="font-mono font-semibold text-[10px] w-7 flex-shrink-0"
        :class="methodColor(tab.draft.method)"
      >
        {{ tab.draft.method.slice(0, 4) }}
      </span>
      <component
        :is="tab.kind === 'doc' && tab.docType === 'product' ? Package : FileText"
        v-else-if="tab.kind === 'doc'"
        :size="12"
        class="flex-shrink-0"
        :class="tab.docType === 'product' ? 'text-accent-purple' : 'text-accent-blue'"
      />
      <Power v-else :size="12" class="flex-shrink-0 text-accent-orange" />
      <span class="truncate flex-1">
        {{
          tab.kind === "request"
            ? tab.draft.name
            : tab.kind === "doc"
              ? tab.title
              : mockTabLabel(tab.routeId)
        }}
      </span>
      <span
        v-if="isDirty(tab)"
        class="w-1.5 h-1.5 rounded-full bg-accent-orange flex-shrink-0"
        title="未保存"
      />
      <button
        class="w-5 h-5 -mr-0.5 flex items-center justify-center rounded text-app-muted hover:text-app-text hover:bg-app-surface2 transition-colors flex-shrink-0 opacity-0 group-hover:opacity-100"
        :class="{ 'opacity-100': tab.id === tabStore.activeTabId }"
        @click="close(tab.id, $event)"
        title="关闭 (Ctrl+W)"
      >
        <X :size="12" />
      </button>
    </div>

    <!-- 新建 Tab -->
    <button
      class="icon-btn self-center flex-shrink-0 ml-0.5"
      title="新建请求 (Ctrl+T)"
      @click="tabStore.newTab()"
    >
      <Plus :size="14" />
    </button>

    <!-- 导入 cURL -->
    <button
      class="icon-btn self-center flex-shrink-0"
      title="导入 cURL 命令"
      @click="emit('importCurl')"
    >
      <ClipboardPaste :size="14" />
    </button>
    </div>

    <!-- 右：固定按钮组（不随 tabs 滚动） -->
    <div class="flex items-center gap-0.5 px-1.5 flex-shrink-0 border-l border-app-border/60">
      <!-- AI 助手开关 -->
      <button
        class="self-center flex-shrink-0 flex items-center gap-1.5 px-2 h-6 rounded-md text-[11px] font-medium transition-colors"
        :class="
          aiStore.panelVisible
            ? 'bg-accent-purple/15 text-accent-purple'
            : 'text-app-muted hover:text-app-text hover:bg-app-hover/60'
        "
        title="AI 助手 (Ctrl+I)"
        @click="aiStore.togglePanel()"
      >
        <Bot :size="13" />
        AI
      </button>

      <!-- MCP 服务开关 -->
      <button
        class="self-center flex-shrink-0 flex items-center gap-1.5 px-2 h-6 rounded-md text-[11px] font-medium transition-colors"
        :class="
          aiStore.mcpPanelVisible
            ? 'bg-accent-blue/15 text-accent-blue'
            : 'text-app-muted hover:text-app-text hover:bg-app-hover/60'
        "
        :title="`MCP 服务${aiStore.mcpUrl ? '（运行中）' : ''}`"
        @click="aiStore.toggleMcpPanel()"
      >
        <Plug :size="13" />
        MCP
      </button>
    </div>
  </div>
  <ContextMenu ref="contextMenu" />
</template>
