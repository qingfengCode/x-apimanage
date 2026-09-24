<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { marked } from "marked";
import { Eye, Pencil, Save, Copy, Check, Trash2, Download } from "lucide-vue-next";
import { useDocsStore } from "@/stores/docs";
import { useTabStore, docTabDirty } from "@/stores/tab";
import { copyText } from "@/utils/curl";
import { useToast } from "@/composables/useToast";
import { makeCodeCopyHandler } from "@/composables/markdownClick";
import MonacoEditor from "@/components/editor/MonacoEditor.vue";

/**
 * 文档 Tab 的内容视图：查看/编辑一篇 Markdown 文档。
 * 编辑状态（标题/类型/正文/是否编辑中）存在 DocTab 对象上，
 * 切换 Tab / 切换工作区都不丢，只有关闭脏 Tab 时才确认丢弃。
 */
const docsStore = useDocsStore();
const tabStore = useTabStore();
const toast = useToast();
const previewRef = ref<HTMLElement | null>(null);
const onPreviewClick = makeCodeCopyHandler(previewRef);

const tab = computed(() => tabStore.activeDocTab);
const doc = computed(() =>
  tab.value?.docId ? docsStore.documents.find((d) => d.id === tab.value!.docId) ?? null : null
);
const creating = computed(() => tab.value != null && tab.value.docId == null);
const dirty = computed(() => (tab.value ? docTabDirty(tab.value) : false));

const previewHtml = computed(() =>
  tab.value ? (marked.parse(tab.value.content, { async: false }) as string) : ""
);

async function save() {
  const t = tab.value;
  if (!t) return;
  if (!t.title.trim()) {
    toast.push("标题不能为空", "error");
    return;
  }
  const wasCreating = t.docId == null;
  const saved = await docsStore.save({
    id: t.docId ?? undefined,
    docType: t.docType,
    title: t.title.trim(),
    content: t.content,
  });
  t.docId = saved.id;
  t.title = saved.title;
  t.editing = false;
  refreshBaseline();
  // 新建 Tab（doc:new:xxx）落库后同步 tab 身份，左侧列表再点进来能命中本 Tab
  tabStore.syncDocTabIdentity(t.id, saved.id);
  if (!wasCreating) toast.push("文档已保存", "success");
}

function refreshBaseline() {
  const t = tab.value;
  if (!t) return;
  t.baseTitle = t.title;
  t.baseType = t.docType;
  t.baseContent = t.content;
}

/** 取消编辑：已落库文档恢复为库中内容；新建未落库则直接关闭 Tab */
function cancelEdit() {
  const t = tab.value;
  if (!t) return;
  if (!t.docId) {
    tabStore.closeTab(t.id);
    return;
  }
  const d = docsStore.documents.find((x) => x.id === t.docId);
  if (d) {
    t.title = d.title;
    t.docType = d.docType === "product" ? "product" : "api";
    t.content = d.content;
  }
  t.editing = false;
  refreshBaseline();
}

const copied = ref(false);
async function copyAll() {
  if (tab.value && (await copyText(tab.value.content))) {
    copied.value = true;
    setTimeout(() => (copied.value = false), 1500);
  }
}

/** 导出为 .md 文件（原生保存对话框） */
async function exportMd() {
  const t = tab.value;
  if (!t) return;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({
      title: "导出文档",
      defaultPath: `${t.title.replace(/[\\/:*?"<>|]/g, "_")}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!path) return;
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("write_text_file", { path, content: t.content });
    toast.push("已导出 Markdown 文件", "success");
  } catch (e) {
    toast.push(String(e), "error", 5000);
  }
}

// Ctrl+S 保存（仅本 Tab 激活时）
function onKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && (e.key === "s" || e.key === "S")) {
    if (tab.value?.editing) {
      e.preventDefault();
      save();
    }
  }
}
onMounted(() => window.addEventListener("keydown", onKeydown, true));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown, true));

async function removeDoc() {
  const t = tab.value;
  if (!t?.docId) return;
  if (!window.confirm(`删除文档「${t.title}」？`)) return;
  await docsStore.remove(t.docId);
  tabStore.closeTab(t.id);
}
</script>

<template>
  <div v-if="tab" class="h-full flex flex-col bg-app-bg min-w-0">
    <!-- 工具栏 -->
    <div class="flex items-center gap-2 px-3 h-10 flex-shrink-0 border-b border-app-border bg-app-surface">
      <template v-if="tab.editing">
        <input
          v-model="tab.title"
          class="input flex-1 max-w-xs text-[13px] font-medium"
          placeholder="文档标题"
          spellcheck="false"
        />
        <select v-model="tab.docType" class="input w-28">
          <option value="api">API 文档</option>
          <option value="product">产品文档</option>
        </select>
      </template>
      <template v-else>
        <span
          class="text-[10px] px-1.5 py-0.5 rounded font-medium flex-shrink-0"
          :class="tab.docType === 'product'
            ? 'bg-accent-purple/15 text-accent-purple'
            : 'bg-accent-blue/15 text-accent-blue'"
        >
          {{ tab.docType === "product" ? "产品文档" : "API 文档" }}
        </span>
        <span class="text-[13px] font-medium truncate">{{ tab.title }}</span>
        <span v-if="doc" class="text-[10px] text-app-muted/70 flex-shrink-0">
          更新于 {{ new Date(doc.updatedAt).toLocaleString() }}
        </span>
      </template>

      <div class="ml-auto flex items-center gap-1">
        <span v-if="dirty" class="text-[11px] text-accent-orange mr-1 flex-shrink-0">未保存</span>
        <template v-if="tab.editing">
          <button class="btn-ghost btn-sm" title="取消编辑" @click="cancelEdit">
            取消
          </button>
          <button class="btn-primary btn-sm" title="保存 (Ctrl+S)" @click="save">
            <Save :size="13" /> 保存
          </button>
        </template>
        <button v-else class="btn-ghost btn-sm" title="编辑" @click="tab.editing = true">
          <Pencil :size="13" /> 编辑
        </button>
        <button class="icon-btn" :title="copied ? '已复制' : '复制 Markdown'" @click="copyAll">
          <Check v-if="copied" :size="14" class="text-accent-green" />
          <Copy v-else :size="14" />
        </button>
        <button class="icon-btn" title="导出为 .md 文件" @click="exportMd">
          <Download :size="14" />
        </button>
        <button
          v-if="tab.docId"
          class="icon-btn hover:text-accent-red hover:bg-accent-red/10"
          title="删除"
          @click="removeDoc"
        >
          <Trash2 :size="14" />
        </button>
      </div>
    </div>

    <!-- 内容：编辑（Monaco）或预览 -->
    <div class="flex-1 min-h-0 flex flex-col">
      <div v-if="tab.editing" class="flex-1 min-h-0">
        <MonacoEditor
          :model-value="tab.content"
          language="markdown"
          @update:model-value="(v: string) => (tab!.content = v)"
        />
      </div>
      <div v-else class="flex-1 min-h-0 flex flex-col">
        <div class="flex items-center gap-1 px-3 h-7 border-b border-app-border/50 text-[11px] text-app-muted flex-shrink-0 bg-app-surface">
          <Eye :size="12" /> 预览
        </div>
        <div
          ref="previewRef"
          class="ai-md flex-1 overflow-auto px-8 py-5 text-[13px] leading-relaxed max-w-[860px]"
          v-html="previewHtml"
          @click="onPreviewClick"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.ai-md :deep(h1) { font-size: 1.5em; font-weight: 700; margin: 0.8em 0 0.5em; }
.ai-md :deep(h2) { font-size: 1.25em; font-weight: 600; margin: 1em 0 0.4em; border-bottom: 1px solid rgb(var(--app-border)); padding-bottom: 0.25em; }
.ai-md :deep(h3),
.ai-md :deep(h4) { font-size: 1.05em; font-weight: 600; margin: 0.9em 0 0.35em; }
.ai-md :deep(p) { margin: 0.5em 0; }
.ai-md :deep(ul),
.ai-md :deep(ol) { padding-left: 1.6em; margin: 0.5em 0; }
.ai-md :deep(li) { margin: 0.2em 0; }
.ai-md :deep(code) {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
  font-size: 0.9em;
  background: rgb(var(--app-bg));
  padding: 0.15em 0.4em;
  border-radius: 4px;
}
.ai-md :deep(pre) {
  background: rgb(var(--app-bg));
  border: 1px solid rgb(var(--app-border));
  border-radius: 8px;
  padding: 0.8em 1em;
  overflow-x: auto;
  margin: 0.7em 0;
}
.ai-md :deep(pre code) { background: transparent; padding: 0; }
.ai-md :deep(table) { border-collapse: collapse; margin: 0.7em 0; width: 100%; font-size: 0.95em; }
.ai-md :deep(th),
.ai-md :deep(td) { border: 1px solid rgb(var(--app-border)); padding: 0.4em 0.7em; text-align: left; }
.ai-md :deep(th) { background: rgb(var(--app-bg)); font-weight: 600; }
.ai-md :deep(blockquote) {
  border-left: 3px solid rgb(var(--accent-blue) / 0.5);
  padding-left: 1em;
  color: rgb(var(--app-muted));
  margin: 0.6em 0;
}
.ai-md :deep(a) { color: rgb(var(--accent-blue)); }
.ai-md :deep(hr) { border: none; border-top: 1px solid rgb(var(--app-border)); margin: 1em 0; }
</style>
