<script setup lang="ts">
import { computed, ref } from "vue";
import { FileText, Package, Plus, Trash2 } from "lucide-vue-next";
import { useDocsStore } from "@/stores/docs";
import { useTabStore } from "@/stores/tab";
import { formatDateTime } from "@/utils/format";
import EmptyState from "@/components/common/EmptyState.vue";

const docsStore = useDocsStore();
const tabStore = useTabStore();
const typeFilter = ref<string>("");

const filteredDocs = computed(() =>
  docsStore.documents.filter((d) => !typeFilter.value || d.docType === typeFilter.value)
);

/** 当前激活 Tab 打开的文档 id（用于列表高亮） */
const activeDocId = computed(() => tabStore.activeDocTab?.docId ?? null);

function typeBadge(t: string) {
  return t === "product"
    ? "bg-accent-purple/15 text-accent-purple"
    : "bg-accent-blue/15 text-accent-blue";
}
function typeLabel(t: string) {
  return t === "product" ? "产品" : "API";
}

/** 点击文档 → 以标签页打开（已打开则激活）；编辑状态在各自 Tab 上，无需切换确认 */
function openDoc(doc: { id: string; title: string; docType: string; content: string }) {
  tabStore.openDocTab(doc);
}

/** 新建文档 Tab（编辑态，保存时才落库） */
function newDoc() {
  tabStore.newDocTab();
}

/** 列表内删除文档：先确认（不可恢复），同时静默关闭其 Tab */
async function removeDoc(docId: string, docName: string) {
  if (!window.confirm(`删除文档「${docName}」？此操作不可恢复。`)) return;
  await docsStore.remove(docId);
  tabStore.closeDocTabByDocId(docId);
}
</script>

<template>
  <div class="flex flex-col h-full">
    <div class="panel-header">
      <span class="panel-title">Docs</span>
      <button class="ml-auto icon-btn" title="新建文档" @click="newDoc">
        <Plus :size="13" />
      </button>
    </div>

    <!-- 类型过滤 -->
    <div class="flex gap-0.5 px-2 py-1 border-b border-app-border">
      <button class="seg" :class="!typeFilter ? 'seg-active' : ''" @click="typeFilter = ''">
        全部
      </button>
      <button class="seg" :class="typeFilter === 'api' ? 'seg-active' : ''" @click="typeFilter = 'api'">
        API
      </button>
      <button
        class="seg"
        :class="typeFilter === 'product' ? 'seg-active' : ''"
        @click="typeFilter = 'product'"
      >
        产品
      </button>
    </div>

    <div class="flex-1 overflow-auto py-1">
      <div
        v-for="doc in filteredDocs"
        :key="doc.id"
        class="group flex items-center gap-2 mx-1 px-2 h-7 rounded-md cursor-pointer hover:bg-app-hover"
        :class="{ 'bg-app-hover': activeDocId === doc.id }"
        @click="openDoc(doc)"
      >
        <component
          :is="doc.docType === 'product' ? Package : FileText"
          :size="13"
          class="flex-shrink-0"
          :class="doc.docType === 'product' ? 'text-accent-purple' : 'text-accent-blue'"
        />
        <div class="flex-1 min-w-0 flex items-center gap-1.5">
          <span
            class="text-[10px] px-1 py-px rounded font-medium flex-shrink-0"
            :class="typeBadge(doc.docType)"
          >
            {{ typeLabel(doc.docType) }}
          </span>
          <span class="text-xs truncate">{{ doc.title }}</span>
          <span class="ml-auto text-[10px] text-app-muted/70 flex-shrink-0">
            {{ formatDateTime(doc.updatedAt) }}
          </span>
        </div>
        <button
          class="icon-btn opacity-0 group-hover:opacity-100 flex-shrink-0"
          title="删除文档"
          @click.stop="removeDoc(doc.id, doc.title)"
        >
          <Trash2 :size="12" />
        </button>
      </div>

      <EmptyState
        v-if="!filteredDocs.length"
        :icon="FileText"
        title="暂无文档"
        hint="让 AI 助手生成，或点击右上角 + 手动新建"
      />
    </div>
  </div>
</template>
