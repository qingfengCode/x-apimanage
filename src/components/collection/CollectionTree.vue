<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { FolderPlus, Download, Search, CornerUpLeft } from "lucide-vue-next";
import { useCollectionStore } from "@/stores/collection";
import TreeNode from "./TreeNode.vue";
import SearchResultItem from "./SearchResultItem.vue";
import EmptyState from "@/components/common/EmptyState.vue";
import ImportExportDialog from "./ImportExportDialog.vue";
import InputModal from "@/components/common/InputModal.vue";
import { Inbox } from "lucide-vue-next";
import type { Collection, RequestItem } from "@/types";
import { treeDragNodeId } from "@/composables/treeDnd";

const collectionStore = useCollectionStore();
const showImportExport = ref(false);

// ---- 顶层放置条：把节点拖到此处提升为顶层（根级） ----
const rootDropHover = ref(false);
async function onRootDrop(e: DragEvent) {
  rootDropHover.value = false;
  const id = treeDragNodeId.value;
  if (!id) return;
  e.preventDefault();
  e.stopPropagation();
  try {
    await collectionStore.moveCollection(id, null);
  } catch (err) {
    console.error("move to root failed:", err);
  }
}

// 顶部新建集合用 InputModal
const showNewColModal = ref(false);
function createCollection(name: string) {
  collectionStore.createCollection({ name, parentId: null, kind: "collection" });
}

// ---- 搜索 ----
const query = ref("");
const searching = computed(() => query.value.trim().length > 0);

/** 扁平化的搜索结果：匹配的集合/文件夹 + 其下的请求 */
const flatResults = computed(() => {
  if (!searching.value) return null;
  const q = query.value.trim().toLowerCase();
  const out: { collection: Collection | null; request: RequestItem }[] = [];
  for (const col of collectionStore.collections) {
    const reqs = collectionStore.requests[col.id] || [];
    for (const r of reqs) {
      if (
        r.name.toLowerCase().includes(q) ||
        (r.url || "").toLowerCase().includes(q) ||
        r.method.toLowerCase().includes(q)
      ) {
        out.push({ collection: col, request: r });
      }
    }
  }
  return out;
});

const required = (v: string) => (v.trim() ? "" : "名称不能为空");

// 命令面板等处的「导入 Postman / OpenAPI」入口（window 事件解耦）
function onGlobalImport() {
  showImportExport.value = true;
}
onMounted(() => window.addEventListener("open:import-export", onGlobalImport));
onBeforeUnmount(() => window.removeEventListener("open:import-export", onGlobalImport));
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- 标题栏 -->
    <div class="panel-header">
      <span class="panel-title">Collections</span>
      <div class="ml-auto flex items-center gap-0.5">
        <button
          class="icon-btn"
          title="导入/导出 Postman"
          @click="showImportExport = true"
        >
          <Download :size="13" />
        </button>
        <button class="icon-btn" title="新建集合" @click="showNewColModal = true">
          <FolderPlus :size="13" />
        </button>
      </div>
    </div>

    <!-- 搜索框 -->
    <div class="px-2 py-1 border-b border-app-border">
      <div class="relative">
        <Search :size="12" class="absolute left-2 top-1/2 -translate-y-1/2 text-app-muted" />
        <input
          v-model="query"
          placeholder="搜索请求（名称/URL/方法）"
          spellcheck="false"
          class="w-full bg-app-surface2 border border-app-border rounded-md pl-7 pr-2 h-6 text-xs outline-none focus:border-accent-blue/70 placeholder:text-app-muted/60 transition-colors"
        />
      </div>
    </div>

    <!-- 顶层放置条：拖拽集合/文件夹到此处 → 提升为顶层 -->
    <Transition
      enter-active-class="transition duration-100 ease-out"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition duration-100 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="treeDragNodeId"
        class="flex-shrink-0 mx-2 my-1 rounded-md border border-dashed flex items-center justify-center gap-1.5 py-1.5 text-[11px] transition-colors cursor-pointer"
        :class="rootDropHover
          ? 'border-accent-green bg-accent-green/10 text-accent-green'
          : 'border-accent-blue/50 bg-accent-blue/5 text-accent-blue'"
        @dragover.prevent="rootDropHover = true"
        @dragleave="rootDropHover = false"
        @drop="onRootDrop"
      >
        <CornerUpLeft :size="12" />
        {{ rootDropHover ? "松开 → 提升为顶层" : "拖到此处提升为顶层（根级）" }}
      </div>
    </Transition>

    <!-- 列表 -->
    <div class="flex-1 overflow-auto py-1">
      <!-- 正常树 -->
      <template v-if="!searching">
        <TreeNode
          v-for="node in collectionStore.roots"
          :key="node.id"
          :node="node"
          :depth="0"
        />
        <EmptyState
          v-if="!collectionStore.roots.length"
          :icon="Inbox"
          title="暂无集合"
          hint="点击右上角图标创建一个集合"
        />
      </template>

      <!-- 搜索结果扁平化 -->
      <template v-else>
        <SearchResultItem
          v-for="(item, i) in flatResults"
          :key="'r' + i"
          :collection="item.collection!"
          :request="item.request"
        />
        <div
          v-if="!flatResults?.length"
          class="flex flex-col items-center justify-center py-8 text-app-muted text-xs gap-1 p-4"
        >
          <Search :size="22" :stroke-width="1" class="opacity-40" />
          <span>无匹配请求</span>
        </div>
      </template>
    </div>

    <ImportExportDialog v-model:show="showImportExport" />
    <InputModal
      v-model:show="showNewColModal"
      title="新建集合"
      label="集合名称"
      placeholder="New Collection"
      initial-value="New Collection"
      :validate="required"
      @confirm="createCollection"
    />
  </div>
</template>
