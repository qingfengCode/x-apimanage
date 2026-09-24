<script setup lang="ts">
import { ref } from "vue";
import {
  ChevronRight,
  Folder,
  FolderOpen,
  MoreVertical,
  FilePlus2,
  FolderPlus,
  Pencil,
  Trash2,
  PlayCircle,
  Copy,
} from "lucide-vue-next";
import type { Collection, RequestItem } from "@/types";
import { useCollectionStore } from "@/stores/collection";
import { treeDragNodeId, clearTreeDrag } from "@/composables/treeDnd";
import { useTabStore } from "@/stores/tab";
import { useWorkspaceStore } from "@/stores/workspace";
import { methodColor } from "@/utils/format";
import ContextMenu, { type MenuItem } from "@/components/common/ContextMenu.vue";
import InputModal from "@/components/common/InputModal.vue";

const props = defineProps<{ node: Collection; depth: number }>();

const collectionStore = useCollectionStore();
const tabStore = useTabStore();
const workspaceStore = useWorkspaceStore();

const menuRef = ref<InstanceType<typeof ContextMenu>>();
const requestMenuRef = ref<InstanceType<typeof ContextMenu>>();

// 单例 InputModal 状态（一个组件实例承载新建/重命名）
const modal = ref({
  show: false,
  title: "",
  label: "",
  placeholder: "",
  initial: "",
  confirmText: "确定",
  onConfirm: null as null | ((v: string) => void | Promise<void>),
});

function openModal(opts: {
  title: string;
  label?: string;
  placeholder?: string;
  initial?: string;
  confirmText?: string;
  validate?: (v: string) => string;
  onConfirm: (v: string) => void | Promise<void>;
}) {
  modal.value = {
    show: true,
    title: opts.title,
    label: opts.label || "",
    placeholder: opts.placeholder || "",
    initial: opts.initial || "",
    confirmText: opts.confirmText || "确定",
    onConfirm: opts.onConfirm,
  };
}
function onModalConfirm(v: string) {
  if (modal.value.onConfirm) modal.value.onConfirm(v);
}
const required = (v: string) => (v.trim() ? "" : "名称不能为空");

// 展开状态以 collection store 为唯一来源：组件因侧栏切换重建后能恢复
// 用户之前的展开/收起（未记录过的节点默认展开）
const expanded = ref(collectionStore.isExpanded(props.node.id));

function setExpanded(v: boolean) {
  expanded.value = v;
  collectionStore.setExpanded(props.node.id, v);
}

function toggle() {
  setExpanded(!expanded.value);
}

const children = () => collectionStore.childrenOf(props.node.id);
const requests = () => collectionStore.requestsOf(props.node.id);

function openRequest(req: RequestItem) {
  tabStore.openRequest(req);
}

function addSubFolder() {
  openModal({
    title: "新建文件夹",
    label: "文件夹名称",
    placeholder: "New Folder",
    initial: "New Folder",
    validate: required,
    onConfirm: async (name) => {
      await collectionStore.createCollection({ name, parentId: props.node.id, kind: "folder" });
      setExpanded(true);
    },
  });
}

function addRequest() {
  openModal({
    title: "新建请求",
    label: "请求名称",
    placeholder: "New Request",
    initial: "New Request",
    validate: required,
    onConfirm: async (name) => {
      const saved = await collectionStore.saveRequest({
        collectionId: props.node.id,
        name,
        method: "GET",
        url: "",
        params: "[]",
        headers: "[]",
        body: "null",
      });
      setExpanded(true);
      openRequest(saved);
    },
  });
}

function rename() {
  openModal({
    title: "重命名",
    label: "名称",
    initial: props.node.name,
    validate: required,
    onConfirm: async (name) => {
      if (name !== props.node.name) await collectionStore.renameCollection(props.node.id, name);
    },
  });
}

async function remove() {
  if (!window.confirm(`删除 "${props.node.name}" 及其所有子项？`)) return;
  await collectionStore.deleteCollection(props.node.id);
}

function openMenu(e: MouseEvent) {
  const items: MenuItem[] = [
    { label: "新建请求", icon: FilePlus2, onClick: addRequest },
    { label: "新建文件夹", icon: FolderPlus, onClick: addSubFolder },
    { divider: true },
    {
      label: "运行集合（Runner）",
      icon: PlayCircle,
      onClick: () => workspaceStore.openRunner(props.node.id),
    },
    { divider: true },
    { label: "重命名", icon: Pencil, onClick: rename },
    { label: "删除", icon: Trash2, danger: true, onClick: remove },
  ];
  menuRef.value?.open(e, items);
}

// ---- 请求项操作 ----
function renameRequest(req: RequestItem) {
  openModal({
    title: "重命名请求",
    label: "名称",
    initial: req.name,
    validate: required,
    onConfirm: async (name) => {
      if (name !== req.name) await collectionStore.renameRequest(req.id, name);
    },
  });
}

async function deleteRequest(req: RequestItem) {
  if (!window.confirm(`删除请求 "${req.name}"？`)) return;
  await collectionStore.deleteRequest(req.id, req.collectionId);
}

function openRequestMenu(e: MouseEvent, req: RequestItem) {
  const items: MenuItem[] = [
    {
      label: "复制请求",
      icon: Copy,
      onClick: async () => {
        const saved = await collectionStore.duplicateRequestById(req.id);
        if (saved) openRequest(saved);
      },
    },
    { label: "重命名", icon: Pencil, onClick: () => renameRequest(req) },
    { label: "删除", icon: Trash2, danger: true, onClick: () => deleteRequest(req) },
  ];
  requestMenuRef.value?.open(e, items);
}

// ---- 拖拽：请求 → 文件夹 / 文件夹 → 文件夹 / 顶层放置条 ----
const dropHover = ref(false);
const REQ_PREFIX = "xapimanage-req:";
const NODE_PREFIX = "xapimanage-col:";

/** 请求行：开始拖拽 */
function onRequestDragStart(e: DragEvent, req: RequestItem) {
  clearTreeDrag();
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", REQ_PREFIX + req.id);
  }
}

/** 节点行：开始拖拽（文件夹/集合整体移动） */
function onNodeDragStart(e: DragEvent) {
  treeDragNodeId.value = props.node.id; // 供 CollectionTree 顶层放置条感知
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", NODE_PREFIX + props.node.id);
  }
}

function onDragEnd() {
  clearTreeDrag(); // dragend 浏览器保证触发（drop 或取消都会）
}

/** 目标节点能否容纳源节点（不能拖到自己/自己的子树里，否则成环） */
function nodeDropAllowed(sourceId: string): boolean {
  if (sourceId === props.node.id) return false;
  return !collectionStore.isNodeDescendant(sourceId, props.node.id);
}

/** 节点行：悬停判定是否可放置 */
function onNodeDragOver(e: DragEvent) {
  if (!e.dataTransfer) return;
  // 注意：dragover 处于 protected mode，getData() 恒返回空串，只能看 types；
  // 源身份用共享状态区分——节点拖拽时 treeDragNodeId 非空，请求拖拽时为 null。
  if (!e.dataTransfer.types.includes("text/plain")) return;
  const sourceNode = treeDragNodeId.value;
  if (sourceNode && !nodeDropAllowed(sourceNode)) return; // 拖到自己/子孙：不放行，显示禁止符
  e.preventDefault();
  e.dataTransfer.dropEffect = "move";
  if (!dropHover.value) dropHover.value = true;
}

function onNodeDragLeave() {
  dropHover.value = false;
}

async function onNodeDrop(e: DragEvent) {
  dropHover.value = false;
  if (!e.dataTransfer) return;
  const data = e.dataTransfer.getData("text/plain");
  if (!data) return;
  e.preventDefault();
  e.stopPropagation();
  try {
    if (data.startsWith(REQ_PREFIX)) {
      await collectionStore.moveRequest(data.slice(REQ_PREFIX.length), props.node.id);
    } else if (data.startsWith(NODE_PREFIX)) {
      const sourceId = data.slice(NODE_PREFIX.length);
      if (nodeDropAllowed(sourceId)) {
        await collectionStore.moveCollection(sourceId, props.node.id);
      }
    }
  } catch (err) {
    console.error("tree drop failed:", err);
  }
}
</script>

<template>
  <div>
    <!-- 节点行 -->
    <div
      draggable="true"
      class="group flex items-center gap-1.5 mx-1 px-1.5 h-6 cursor-pointer rounded text-xs transition-colors"
      :class="[
        dropHover
          ? 'bg-accent-blue/15 shadow-[inset_0_0_0_1px_rgb(var(--accent-blue)/0.6)]'
          : 'hover:bg-app-hover',
      ]"
      :style="{ marginLeft: depth * 12 + 4 + 'px' }"
      @click="toggle"
      @contextmenu="openMenu"
      @dragstart="onNodeDragStart"
      @dragend="onDragEnd"
      @dragover="onNodeDragOver"
      @dragleave="onNodeDragLeave"
      @drop="onNodeDrop"
    >
      <ChevronRight
        :size="12"
        class="text-app-muted transition-transform flex-shrink-0"
        :class="{ 'rotate-90': expanded }"
      />
      <component
        :is="expanded ? FolderOpen : Folder"
        :size="13"
        class="text-accent-yellow flex-shrink-0"
      />
      <span class="flex-1 truncate">{{ node.name }}</span>
      <!-- 拖拽悬停提示 -->
      <span
        v-if="dropHover"
        class="flex-shrink-0 text-[10px] text-accent-blue mr-1 animate-pulse"
      >
        松开移到这里
      </span>
      <button
        v-if="!dropHover"
        class="opacity-0 group-hover:opacity-100 w-5 h-5 flex items-center justify-center rounded hover:bg-app-surface2 text-app-muted hover:text-app-text transition-all"
        title="新建请求"
        @click.stop="addRequest"
      >
        <FilePlus2 :size="12" />
      </button>
      <button
        v-if="!dropHover"
        class="opacity-0 group-hover:opacity-100 w-5 h-5 flex items-center justify-center rounded hover:bg-app-surface2 text-app-muted hover:text-app-text transition-all"
        title="更多"
        @click.stop="openMenu"
      >
        <MoreVertical :size="12" />
      </button>
    </div>

    <!-- 子项 -->
    <div v-show="expanded">
      <TreeNode
        v-for="child in children()"
        :key="child.id"
        :node="child"
        :depth="depth + 1"
      />
      <!-- 请求列表 -->
      <div
        v-for="req in requests()"
        :key="req.id"
        draggable="true"
        class="group flex items-center gap-1.5 mx-1 px-1.5 h-6 cursor-pointer rounded text-xs hover:bg-app-hover"
        :class="{ 'tree-item-active': tabStore.activeTabId === req.id }"
        :style="{ marginLeft: depth * 12 + 22 + 'px' }"
        :title="tabStore.activeTabId === req.id ? '' : '可拖拽到其它文件夹'"
        @click="openRequest(req)"
        @contextmenu="(e) => openRequestMenu(e, req)"
        @dragstart="(e) => onRequestDragStart(e, req)"
        @dragend="onDragEnd"
      >
        <span
          class="font-mono text-[10px] font-bold w-[34px] flex-shrink-0"
          :class="methodColor(req.method)"
        >
          {{ (req.method || "GET").slice(0, 4) }}
        </span>
        <span class="flex-1 truncate text-app-text/90">{{ req.name }}</span>
      </div>
    </div>

    <ContextMenu ref="menuRef" />
    <ContextMenu ref="requestMenuRef" />
    <InputModal
      v-model:show="modal.show"
      :title="modal.title"
      :label="modal.label"
      :placeholder="modal.placeholder"
      :initial-value="modal.initial"
      :confirm-text="modal.confirmText"
      :validate="required"
      @confirm="onModalConfirm"
    />
  </div>
</template>
